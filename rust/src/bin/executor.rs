//! Ballista executor binary.

use anyhow::Result;

use ballista_aws_tools::start_executor;

use ballista_core::print_version;

#[macro_use]
extern crate configure_me;

#[cfg(feature = "snmalloc")]
#[global_allocator]
static ALLOC: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;

#[allow(clippy::all, warnings)]
mod config {
    // Ideally we would use the include_config macro from configure_me, but then we cannot use
    // #[allow(clippy::all)] to silence clippy warnings from the generated code
    include!(concat!(env!("OUT_DIR"), "/executor_configure_me_config.rs"));
}

use config::prelude::*;

pub async fn executor() -> Result<()> {
    let (opt, _remaining_args) =
        config::Config::including_optional_config_files(&["/etc/ballista/executor.toml"])
            .unwrap_or_exit();

    if opt.version {
        print_version();
        std::process::exit(0);
    }

    let bind_host = opt.bind_host;
    let bind_port = opt.bind_port;

    let scheduler_host = opt.scheduler_host;
    let scheduler_port = opt.scheduler_port;

    start_executor(bind_host, bind_port, scheduler_host, scheduler_port, None).await
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    executor().await
}
