//! Ballista executor binary.
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use log::info;

use ballista_aws_tools::fargate;
use ballista_aws_tools::{start_executor, wait_executors};

#[macro_use]
extern crate configure_me;

include_config!("executor");

const TASK_RECONNECT_SEC: u64 = 10;

pub async fn executor() -> Result<()> {
    let (opt, _remaining_args) =
        config::Config::including_optional_config_files(&["/etc/ballista/executor.toml"])
            .unwrap_or_exit();

    // if no host is specified in conf, assume we are runnin in Fargate
    let scheduler_host = match &opt.scheduler_host {
        Some(host) => host.clone(),
        None => {
            let client = fargate::FargateCreationClient::try_new(opt.cluster_name)?;
            let task_def_arn_ref = Arc::new(opt.scheduler_task_def_arn);
            let task_arn = client
                .get_existing_tasks(String::clone(&task_def_arn_ref))
                .await?;
            if task_arn.len() == 0 {
                bail!("Scheduler task not found");
            }
            let host = client.wait_for_provisioning(task_arn).await?;
            // poll fargate to check that the scheduler is still there, otherwise shutdown
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(TASK_RECONNECT_SEC));
                loop {
                    interval.tick().await;
                    // TODO ping scheduler instead of using Fargate API
                    let scheduler_tasks = client
                        .get_existing_tasks(String::clone(&task_def_arn_ref))
                        .await
                        .expect("Could not get existing tasks in keepalive ticker");
                    if scheduler_tasks.len() == 0 {
                        info!("Shutting down after scheduler lost");
                        exit(0);
                    }
                }
            });
            String::from(&host[0])
        }
    };

    let bind_host = opt.bind_host;
    let bind_port = opt.bind_port;
    let scheduler_port = opt.scheduler_port;
    let concurrent_tasks = opt.concurrent_tasks as usize;

    // should wait for the scheduler to be ready (up with 0 executor) before starting.
    wait_executors(&scheduler_host, scheduler_port, 0).await?;

    start_executor(bind_host, bind_port, scheduler_host, scheduler_port, None, concurrent_tasks).await
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    executor().await
}
