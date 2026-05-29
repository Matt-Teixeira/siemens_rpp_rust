//! `siemens_rpp` entrypoint.
//!
//! Phase 0 scope: load config, build the CLI, and on every subcommand emit a
//! "hello" run into `util.app_run_logs` (the Phase 0 exit criterion). The real
//! per-system parse/persist loop arrives in later phases; `ct`/`mri`/`parse`
//! currently just prove the logging + DB + (optionally) Redis wiring end to end.

mod cli;
mod config;
mod error;
mod hello;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::error::AppError;

#[tokio::main]
async fn main() {
    // Load .env before anything reads the environment.
    let _ = dotenvy::dotenv();
    rpp_log::tracing_init::init("info");

    // TODO(M7 hardening): graceful shutdown on SIGINT/SIGTERM that flushes the
    // recorder/exporter before exit (see siemens_rpp_plan.md operational reqs).
    // Not needed for the Phase 0 one-shot batch path.

    if let Err(err) = run().await {
        // Top-level: log and exit non-zero so cron sees the failure.
        tracing::error!("{err:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), AppError> {
    let cli = Cli::parse();
    let cfg = config::Config::from_env()?;

    match cli.command {
        Command::Ct => hello::hello_run(&cfg, "ct").await,
        Command::Mri => hello::hello_run(&cfg, "mri").await,
        Command::Parse(args) => {
            tracing::info!(?args, "parse subcommand is a Phase 1 stub");
            hello::hello_run(&cfg, "parse").await
        }
    }
}
