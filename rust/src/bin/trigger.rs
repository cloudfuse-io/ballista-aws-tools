use std::time::Instant;

use anyhow::Result;
use ballista::prelude::BallistaConfig;
use log::{debug, info};

use ballista_aws_tools::tpch::{register_memsql_tpch_tables, get_query};
use ballista_aws_tools::{fargate, wait_executors};

use ballista::context::BallistaContext;
use datafusion::arrow::util::pretty;
use lambda_runtime::{handler_fn, Context, Error};
use serde_json::Value;

#[macro_use]
extern crate configure_me;

include_config!("trigger");

async fn query_ballista(host: &str, port: u16, tpch_query: u8) -> Result<()> {
    let mut ctx = BallistaContext::remote(host, port, &BallistaConfig::new()?);
    register_memsql_tpch_tables(&mut ctx)?;
    let sql = get_query(tpch_query)?;
    // run benchmark
    debug!("Running benchmark with query: {}", sql);
    
    let df = ctx.sql(&sql)?;
    debug!("plan: {:?}", &df.to_logical_plan());
    let batches = df.collect().await?;
    pretty::print_batches(&batches)?;

    Ok(())
}

pub async fn start_trigger(executor_count: usize, tpch_query: u8) -> Result<(u64, u64)> {
    // parse options
    let (opt, _remaining_args) =
        config::Config::including_optional_config_files(&["/etc/ballista/standalone.toml"])
            .unwrap_or_exit();

    let start = Instant::now();

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

    let provisioning_duration = start.elapsed().as_millis() as u64;

    let start = Instant::now();
    query_ballista(&scheduler_ip, opt.scheduler_port, tpch_query).await?;
    let execution_duration = start.elapsed().as_millis() as u64;

    Ok((provisioning_duration, execution_duration))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    let func = handler_fn(my_handler);
    lambda_runtime::run(func).await?;
    Ok(())
}

#[derive(Deserialize)]
struct TriggerQuery {
    #[serde(default)]
    pub executor_count: u16,
    #[serde(default)]
    pub tpch_query: u8,
}

impl Default for TriggerQuery{
    fn default() -> Self {
        Self { executor_count: 2, tpch_query: 1 }
    }
}

#[derive(Serialize)]
struct TriggerResponse {
    pub provisioning_duration_ms: u64,
    pub execution_duration_ms: u64,
}

async fn my_handler(event: Value, _: Context) -> Result<Value, Error> {
    let query: TriggerQuery = serde_json::from_value(event)?;
    let (provisioning_duration_ms, execution_duration_ms) = start_trigger(query.executor_count as usize, query.tpch_query).await?;
    Ok(serde_json::to_value(TriggerResponse{provisioning_duration_ms, execution_duration_ms})?)
}
