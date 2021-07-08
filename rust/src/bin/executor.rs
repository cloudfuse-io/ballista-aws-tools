//! Ballista executor binary.
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use log::info;

use ballista_aws_tools::fargate;
use ballista_aws_tools::start_executor;

#[macro_use]
extern crate configure_me;

include_config!("executor");

const TASK_RECONNECT_SEC: u64 = 10;

pub async fn executor() -> Result<()> {
    let (opt, _remaining_args) =
        config::Config::including_optional_config_files(&["/etc/ballista/executor.toml"])
            .unwrap_or_exit();

    let bind_host = opt.bind_host;
    let bind_port = opt.bind_port;

    // if no host is specified in conf, assume we are runnin in Fargate
    let scheduler_host = match &opt.scheduler_host {
        Some(host) => host.clone(),
        None => {
            let client = fargate::FargateCreationClient::try_new(opt.cluster_name)?;
            let task_def_arn_ref = Arc::new(opt.scheduler_task_def_arn);
            let task_arn = client
                .get_existing_task(String::clone(&task_def_arn_ref))
                .await
                .ok_or(anyhow!("Scheduler task not found"))?;
            let host = client.wait_for_provisioning(task_arn).await?;
            // poll fargate to check that the scheduler is still there, otherwise shutdown
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(TASK_RECONNECT_SEC));
                loop {
                    interval.tick().await;
                    client
                        .get_existing_task(String::clone(&task_def_arn_ref))
                        .await
                        .or_else(|| {
                            info!("Shutting down after scheduler lost");
                            exit(0);
                        });
                }
            });
            host
        }
    };

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
        }
    });

    let scheduler_port = opt.scheduler_port;
    start_executor(bind_host, bind_port, scheduler_host, scheduler_port, None).await
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    executor().await
}
