//! `siemens_rpp` entrypoint — a thin shim over the library `run()`.
//!
//! `ct`/`mri` are the cron-driven boot+parse jobs; `parse` is single-system mode
//! for testing / one-off retry. The orchestration lives in `lib.rs`.

#[tokio::main]
async fn main() {
    // Load .env before anything reads the environment.
    let _ = dotenvy::dotenv();
    rpp_log::tracing_init::init("info");

    // TODO(M7 hardening): graceful shutdown on SIGINT/SIGTERM that flushes the
    // recorder/exporter before exit (see siemens_rpp_plan.md operational reqs).
    // Not needed for the current one-shot batch path.

    if let Err(err) = siemens_rpp::run().await {
        // Top-level: log and exit non-zero so cron sees the failure.
        tracing::error!("{err:#}");
        std::process::exit(1);
    }
}
