//! Run-logging that mirrors the Node `util.app_run_logs` contract bit-for-bit.
//!
//! Source of truth: `hhm_rpp_siemens/utils/logger/{log,enums}.js`. A run owns an
//! in-memory `Vec<LogEvent>`; at the end of the run the whole buffer is serialized
//! to `verbose_log`, the WARN/ERROR subset to `warn_error_logs`, and both are
//! written as one row into `util.app_run_logs` — plus a JSON file on disk.
//!
//! This crate is intentionally storage-agnostic: it produces the
//! [`AppRunLogRow`] to persist and the bytes to write, but the actual PG INSERT is
//! performed by a [`RunLogSink`] implementor (see `rpp_db`). That keeps `rpp_log`
//! free of any database dependency so it can be lifted into a standalone fleet
//! workspace later.

mod enums;
mod event;
mod run_log;
mod sink;
pub mod tracing_init;

pub use enums::{Tag, Type};
pub use event::LogEvent;
pub use run_log::{AppRunLogRow, RunLog};
pub use sink::RunLogSink;

/// Crate error type. The persist path returns the sink's error boxed behind this.
#[derive(Debug, thiserror::Error)]
pub enum LogError {
    #[error("failed to serialize log events: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("failed to write run-log file at {path}: {source}")]
    FileWrite {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("run-log sink failed: {0}")]
    Sink(#[source] Box<dyn std::error::Error + Send + Sync>),
}

pub type Result<T> = std::result::Result<T, LogError>;
