// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Ballista Rust executor binary.

use std::convert::Infallible;
use std::{net::SocketAddr, sync::Arc};

use anyhow::{Context, Result};
use arrow_flight::flight_service_server::FlightServiceServer;

use futures::future::{self, Either, TryFutureExt};
use hyper::{server::conn::AddrStream, service::make_service_fn, Server};
use log::info;
use tempfile::TempDir;
use tonic::transport::Server as TonicServer;
use tower::Service;
use uuid::Uuid;

use ballista_core::serde::protobuf::{
    scheduler_grpc_client::SchedulerGrpcClient, ExecutorRegistration,
};
use ballista_core::BALLISTA_VERSION;
use ballista_core::{print_version, serde::protobuf::scheduler_grpc_server::SchedulerGrpcServer};
use ballista_executor::execution_loop;
use ballista_executor::executor::Executor;
use ballista_executor::flight_service::BallistaFlightService;
use ballista_scheduler::api::{get_routes, EitherBody, Error};
use ballista_scheduler::state::StandaloneClient;
use ballista_scheduler::{state::ConfigBackendClient, SchedulerServer};

#[macro_use]
extern crate configure_me;

#[allow(clippy::all, warnings)]
mod config {
    // Ideally we would use the include_config macro from configure_me, but then we cannot use
    // #[allow(clippy::all)] to silence clippy warnings from the generated code
    include!(concat!(
        env!("OUT_DIR"),
        "/standalone_configure_me_config.rs"
    ));
}

use config::prelude::*;

#[cfg(feature = "snmalloc")]
#[global_allocator]
static ALLOC: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;

/// We limit the number of concurrent tasks to 1 for now
static CONCURENT_TASKS: usize = 1;

async fn start_server(
    config_backend: Arc<dyn ConfigBackendClient>,
    namespace: String,
    addr: SocketAddr,
) -> Result<()> {
    info!(
        "Ballista v{} Scheduler listening on {:?}",
        BALLISTA_VERSION, addr
    );

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

            future::ok::<_, Infallible>(tower::service_fn(
                move |req: hyper::Request<hyper::Body>| {
                    let header = req.headers().get(hyper::header::ACCEPT);
                    if header.is_some() && header.unwrap().eq("application/json") {
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
    start_server(client, namespace, addr).await?;
    Ok(())
}

pub async fn executor(opt: &Config) -> Result<()> {
    let bind_host = &opt.bind_host;
    let port = opt.executor_bind_port;

    let addr = format!("{}:{}", bind_host, port);
    let addr = addr
        .parse()
        .with_context(|| format!("Could not parse address: {}", addr))?;

    let scheduler_host = "localhost";
    let scheduler_port = opt.scheduler_bind_port;
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
        optional_host: None,
        port: port as u32,
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

#[tokio::main]
async fn main() -> Result<()> {
    // parse options
    let (opt, _remaining_args) =
        config::Config::including_optional_config_files(&["/etc/ballista/standalone.toml"])
            .unwrap_or_exit();

    if opt.version {
        print_version();
        std::process::exit(0);
    }

    env_logger::init();
    tokio::select! {
        res = scheduler(&opt) => {
            println!("scheduler stopped: {:?}", res);
        }
        res = executor(&opt) => {
            println!("executor stopped: {:?}", res);
        }
    }
    Ok(())
}
