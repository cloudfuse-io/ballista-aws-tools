//! Ballista Rust scheduler + executor binary.

use std::convert::Infallible;
use std::sync::atomic::Ordering;
use std::{net::SocketAddr, sync::Arc};

use ballista_aws_tools::fargate::get_fargate_task_external_host;
use ballista_aws_tools::{shutdown_ticker, start_executor};

use anyhow::{Context, Result};
use futures::future::{self, Either, TryFutureExt};
use hyper::{server::conn::AddrStream, service::make_service_fn, Server};
use log::info;
use tonic::transport::Server as TonicServer;
use tower::Service;

use ballista_core::serde::protobuf::scheduler_grpc_server::SchedulerGrpcServer;
use ballista_core::BALLISTA_VERSION;
use ballista_scheduler::api::{get_routes, EitherBody, Error};
use ballista_scheduler::state::StandaloneClient;
use ballista_scheduler::{state::ConfigBackendClient, SchedulerServer};

#[macro_use]
extern crate configure_me;

include_config!("standalone");

async fn start_scheduler_server(
    config_backend: Arc<dyn ConfigBackendClient>,
    namespace: String,
    addr: SocketAddr,
) -> Result<()> {
    info!(
        "Ballista v{} Scheduler listening on {:?}",
        BALLISTA_VERSION, addr
    );

    let last_query_time = shutdown_ticker();

    Ok(Server::bind(&addr)
        .serve(make_service_fn(move |request: &AddrStream| {
            let scheduler_server = SchedulerServer::new(
                config_backend.clone(),
                namespace.clone(),
                request.remote_addr().ip(),
            );
            let scheduler_grpc_server = SchedulerGrpcServer::new(scheduler_server.clone());

            let mut tonic = TonicServer::builder()
                .add_service(scheduler_grpc_server)
                .into_service();
            let mut warp = warp::service(get_routes(scheduler_server));

            let last_query_time = Arc::clone(&last_query_time);

            future::ok::<_, Infallible>(tower::service_fn(
                move |req: hyper::Request<hyper::Body>| {
                    let lifetime_header = req.headers().get("x-lifetime");
                    if lifetime_header.is_some() && lifetime_header.unwrap().eq("extend") {
                        last_query_time.store(chrono::Utc::now().timestamp(), Ordering::Relaxed);
                    }
                    let accept_header = req.headers().get(hyper::header::ACCEPT);
                    if accept_header.is_some() && accept_header.unwrap().eq("application/json") {
                        return Either::Left(
                            warp.call(req)
                                .map_ok(|res| res.map(EitherBody::Left))
                                .map_err(Error::from),
                        );
                    }
                    Either::Right(
                        tonic
                            .call(req)
                            .map_ok(|res| res.map(EitherBody::Right))
                            .map_err(Error::from),
                    )
                },
            ))
        }))
        .await
        .context("Could not start grpc server")?)
}

async fn scheduler(opt: &Config) -> Result<()> {
    let namespace = opt.namespace.clone();
    let bind_host = &opt.bind_host;
    let port = opt.scheduler_bind_port;

    let addr = format!("{}:{}", bind_host, port);
    let addr = addr.parse()?;

    let client = Arc::new(
        StandaloneClient::try_new_temporary()
            .context("Could not create standalone config backend")?,
    );
    start_scheduler_server(client, namespace, addr).await?;
    Ok(())
}

pub async fn executor(opt: &Config) -> Result<()> {
    let bind_host = opt.bind_host.clone();
    // if no host is specified in conf, assume we are runnin in Fargate
    let external_host = match &opt.executor_external_host {
        Some(host) => host.clone(),
        None => get_fargate_task_external_host().await?,
    };
    let bind_port = opt.executor_bind_port;
    let scheduler_host = "localhost".to_owned();
    let scheduler_port = opt.scheduler_bind_port;

    start_executor(
        bind_host,
        bind_port,
        scheduler_host,
        scheduler_port,
        Some(external_host),
    )
    .await
}

#[tokio::main]
async fn main() -> Result<()> {
    // parse options
    let (opt, _remaining_args) =
        config::Config::including_optional_config_files(&["/etc/ballista/standalone.toml"])
            .unwrap_or_exit();

    env_logger::init();
    tokio::select! {
        res = scheduler(&opt) => {
            info!("scheduler stopped: {:?}", res);
        }
        res = executor(&opt) => {
            info!("executor stopped: {:?}", res);
        }
    }
    Ok(())
}
