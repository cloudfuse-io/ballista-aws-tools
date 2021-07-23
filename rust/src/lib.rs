use std::process::exit;
use std::str;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use arrow_flight::flight_service_server::FlightServiceServer;
use hyper::{body::to_bytes, Body, Client, Method, Request, Uri};

use log::{debug, info};
use serde::Deserialize;
use tempfile::TempDir;
use tonic::transport::Server as TonicServer;
use uuid::Uuid;

use ballista_core::serde::protobuf::{
    executor_registration, scheduler_grpc_client::SchedulerGrpcClient, ExecutorRegistration,
};
use ballista_core::BALLISTA_VERSION;
use ballista_executor::execution_loop;
use ballista_executor::executor::Executor;
use ballista_executor::flight_service::BallistaFlightService;

////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Deserialize)]
pub struct RegisteredExecutors {
    pub id: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchedulerState {
    pub executors: Vec<RegisteredExecutors>,
}

/// connects to the scheduler and waits until there are sufficient executors connected
pub async fn wait_executors(
    scheduler_host: &str,
    scheduler_port: u16,
    min_executor_count: usize,
) -> Result<()> {
    let uri: Uri = format!("http://{}:{}/state", scheduler_host, scheduler_port).parse()?;
    let client = Client::new();
    loop {
        let req = Request::builder()
            .method(Method::GET)
            .uri(uri.clone())
            .header(hyper::header::ACCEPT, "application/json")
            .header("x-lifetime", "extend")
            .body(Body::empty())?;

        let resp = match client.request(req).await {
            Ok(resp) => resp,
            Err(e) => {
                info!("Could not connect to scheduler, retrying...");
                debug!("{:?}", e);
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
        };
        let body: Body = resp.into_body();
        let body_bytes = to_bytes(body).await?;
        let state: SchedulerState = serde_json::from_slice(&body_bytes).with_context(|| {
            format!(
                "Impossible to parse scheduler state: {}",
                str::from_utf8(&body_bytes).unwrap()
            )
        })?;
        if state.executors.len() >= min_executor_count {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

///////////////////////////////////////////////

/// We limit the number of concurrent tasks to 1 for now
static CONCURENT_TASKS: usize = 1;

use futures::future::{BoxFuture, FutureExt};
use tonic::transport::Channel;

fn connect(
    scheduler_url: String,
    retry: u8,
) -> BoxFuture<'static, Result<SchedulerGrpcClient<Channel>>> {
    async move {
        match SchedulerGrpcClient::connect(scheduler_url.clone()).await {
            Ok(sched) => Ok(sched),
            Err(e) if retry == 2 => Err(e)
                .with_context(|| format!("Connection failed to scheduler at {}", scheduler_url)),
            Err(_) => connect(scheduler_url, retry + 1).await,
        }
    }
    .boxed()
}

pub async fn start_executor(
    bind_host: String,
    bind_port: u16,
    scheduler_host: String,
    scheduler_port: u16,
    optional_host: Option<String>,
) -> Result<()> {
    let addr = format!("{}:{}", bind_host, bind_port);
    let addr = addr
        .parse()
        .with_context(|| format!("Could not parse address: {}", addr))?;

    let scheduler_url = format!("http://{}:{}", scheduler_host, scheduler_port);

    let work_dir = TempDir::new()?
        .into_path()
        .into_os_string()
        .into_string()
        .unwrap();
    info!("Running with config:");
    info!("work_dir: {}", work_dir);
    info!("concurrent_tasks: {}", CONCURENT_TASKS);

    let executor_meta = ExecutorRegistration {
        id: Uuid::new_v4().to_string(), // assign this executor a unique ID
        optional_host: optional_host.map(executor_registration::OptionalHost::Host),
        port: bind_port as u32,
    };

    let scheduler = connect(scheduler_url.clone(), 0).await?;

    let executor = Arc::new(Executor::new(&work_dir));

    let service = BallistaFlightService::new(executor.clone());

    let server = FlightServiceServer::new(service);
    info!(
        "Ballista v{} Rust Executor listening on {:?}",
        BALLISTA_VERSION, addr
    );
    let server_future = tokio::spawn(TonicServer::builder().add_service(server).serve(addr));
    tokio::spawn(execution_loop::poll_loop(
        scheduler,
        executor,
        executor_meta,
        CONCURENT_TASKS,
    ));

    server_future
        .await
        .context("Tokio error")?
        .context("Could not start executor server")?;
    Ok(())
}

//////////////////////////////////////////////////////

const TASK_EXPIRATION_SEC: i64 = 300;

pub fn shutdown_ticker() -> Arc<AtomicI64> {
    let last_query = Arc::new(AtomicI64::new(chrono::Utc::now().timestamp()));
    let last_query_ref = Arc::clone(&last_query);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let elapsed = chrono::Utc::now().timestamp() - last_query_ref.load(Ordering::Relaxed);
            if elapsed >= TASK_EXPIRATION_SEC {
                info!(
                    "task expired after {}s of inactivity, shutting down...",
                    elapsed
                );
                exit(0);
            }
        }
    });
    last_query
}

///////////////////////////////////////////////////////

pub mod fargate;
pub mod tpch;
