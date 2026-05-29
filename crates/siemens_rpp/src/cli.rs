//! Command-line surface (clap derive). Subcommands align with the Node `npm run`
//! scripts: `ct` / `mri` are cron-driven; `parse` is the single-system mode for
//! testing and one-off retries (Phase 1+).

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "siemens_rpp",
    version,
    about = "Siemens CT/MRI log parser (Rust port)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the SIEMENS_CT boot query, then the per-system loop.
    Ct,
    /// Run the SIEMENS_MRI boot query, then the per-system loop.
    Mri,
    /// Single-system mode for testing / one-off retry (Phase 1+).
    Parse(ParseArgs),
}

/// Arguments for `siemens_rpp parse`. Wired up in Phase 1; defined now so the CLI
/// shape is stable.
#[derive(Debug, Args)]
pub struct ParseArgs {
    /// System id, e.g. SME00817.
    #[arg(long)]
    pub system_id: Option<String>,
    /// Modality: ct | mri.
    #[arg(long)]
    pub modality: Option<String>,
    /// Explicit file path override (skips path construction).
    #[arg(long)]
    pub file: Option<String>,
    /// IANA timezone override.
    #[arg(long)]
    pub tz: Option<String>,
    /// Parse and report, but perform no PG/Redis writes.
    #[arg(long)]
    pub dry_run: bool,
    /// Skip the Redis cursor SET (useful for backfills).
    #[arg(long)]
    pub no_cursor_update: bool,
}
