use std::env;
use std::process::exit;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use arrow_flight::flight_service_server::FlightServiceServer;
use hyper::{body::to_bytes, Body, Client, Uri};
use log::info;
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
pub struct FargateNetwork {
    #[serde(rename(deserialize = "IPv4Addresses"))]
    pub ipv4_addresses: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FargateContainer {
    #[serde(rename(deserialize = "Networks"))]
    pub networks: Vec<FargateNetwork>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FargateMetadata {
    #[serde(rename(deserialize = "Containers"))]
    pub containers: Vec<FargateContainer>,
}

pub async fn get_fargate_task_external_host() -> Result<String> {
    let matadata_endpoint = env::var("ECS_CONTAINER_METADATA_URI_V4")?;
    let uri: Uri = (matadata_endpoint + "/task").parse()?;
    let client = Client::new();
    loop {
        let resp = client.get(uri.clone()).await?;
        let body: Body = resp.into_body();
        let body_bytes = to_bytes(body).await?;
        let metadata: FargateMetadata = serde_json::from_slice(&body_bytes)?;
        if metadata.containers.len() > 0
            && metadata.containers[0].networks.len() > 0
            && metadata.containers[0].networks[0].ipv4_addresses.len() > 0
        {
            return Ok(metadata.containers[0].networks[0].ipv4_addresses[0].clone());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

///////////////////////////////////////////////

/// We limit the number of concurrent tasks to 1 for now
static CONCURENT_TASKS: usize = 1;

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

    let scheduler = SchedulerGrpcClient::connect(scheduler_url)
        .await
        .context("Could not connect to scheduler")?;

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
