//! Optional dev-facing `tracing` setup. The buffered [`RunLog`](crate::RunLog)
//! events remain the source of truth for `util.app_run_logs`; this is only for
//! human-readable console output during development.

use tracing_subscriber::{EnvFilter, FmtSubscriber};

/// Initialize a `tracing` subscriber from `RUST_LOG` (default `info`).
///
/// Logs are written to **stderr** so stdout stays reserved for machine-readable
/// program output (e.g. `siemens_rpp parse --dry-run` emits a pure JSON array that
/// must be byte-diffable against the Node output).
///
/// Idempotent-ish: uses `try_init` so a second call (e.g. in tests) is a no-op
/// rather than a panic.
pub fn init(default_directive: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_directive));
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}
