use std::error::Error;
use std::time::Instant;

use anyhow::Result;
use log::{debug, info};

use ballista_aws_tools::tpch::{register_tpch_tables, QUERY_1};
use ballista_aws_tools::{fargate, wait_executors};

use ballista::context::BallistaContext;
use datafusion::arrow::util::pretty;
use futures::StreamExt;
use lambda_runtime::{error::HandlerError, lambda, Context};
use serde_json::Value;

#[macro_use]
extern crate configure_me;

include_config!("trigger");

async fn query_ballista(host: &str, port: u16) -> Result<()> {
    let mut ctx = BallistaContext::remote(host, port);
    register_tpch_tables(&mut ctx)?;
    // run benchmark
    let sql = QUERY_1;
    debug!("Running benchmark with query: {}", sql);
    let start = Instant::now();
    let df = ctx.sql(&sql)?;
    let mut batches = vec![];
    debug!("plan: {:?}", &df.to_logical_plan());
    let mut stream = ctx.collect(&df.to_logical_plan()).await?;
    while let Some(result) = stream.next().await {
        let batch = result?;
        batches.push(batch);
    }
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    info!("Query took {:.1} ms", elapsed);
    pretty::print_batches(&batches)?;

    Ok(())
}

pub async fn start_trigger() -> Result<()> {
    let executor_count = 3;
    // parse options
    let (opt, _remaining_args) =
        config::Config::including_optional_config_files(&["/etc/ballista/standalone.toml"])
            .unwrap_or_exit();

    // start standalone and extra executor
    let subnets: Vec<String> = opt.subnets.split(",").map(|s| s.to_owned()).collect();
    let client = fargate::FargateCreationClient::try_new(opt.cluster_name)?;
    let sched_future = client.get_or_provision(
        opt.standalone_task_def_arn,
        opt.standalone_task_sg_id,
        subnets.clone(),
        1,
    );
    let exec_future = client.get_or_provision(
        opt.executor_task_def_arn,
        opt.executor_task_sg_id,
        subnets,
        executor_count - 1,
    );
    let (scheduler_ip_res, executor_ip_res) = tokio::join!(sched_future, exec_future);
    let scheduler_ip = &scheduler_ip_res?[0];
    let executor_ips = executor_ip_res?;

    info!("scheduler: {}, executors: {:?}", scheduler_ip, executor_ips);

    wait_executors(&scheduler_ip, opt.scheduler_port, executor_count).await?;

    query_ballista(&scheduler_ip, opt.scheduler_port).await?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    lambda!(my_handler);
    Ok(())
}

fn my_handler(event: Value, _: Context) -> Result<Value, HandlerError> {
    info!("event: {:?}", event);
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(start_trigger())
        .unwrap();
    Ok(Value::String("Ok!".to_owned()))
}
