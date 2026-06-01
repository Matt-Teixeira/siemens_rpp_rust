//! `siemens_rpp` library root.
//!
//! Exists so the pure logic (parsing, types, path building) is reachable from
//! integration tests and so `main.rs` stays a thin shim. The Siemens CT/MRI log
//! parser (Rust port of `hhm_rpp_siemens`).

pub mod boot;
pub mod cli;
pub mod config;
pub mod error;
pub mod hello;
pub mod parse;
pub mod path;
pub mod runner;
pub mod types;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::error::AppError;

/// Parse the CLI, load config, and dispatch the subcommand. Returns the typed
/// error so `main` can log + set the exit code.
pub async fn run() -> Result<(), AppError> {
    let cli = Cli::parse();
    let cfg = config::Config::from_env()?;

    match cli.command {
        Command::Ct => hello::hello_run(&cfg, "ct").await,
        Command::Mri => hello::hello_run(&cfg, "mri").await,
        Command::Parse(args) => runner::run_parse(&cfg, &args).await,
    }
}
