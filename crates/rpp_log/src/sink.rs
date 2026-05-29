//! The persistence boundary. `rpp_log` knows how to *build* the
//! [`AppRunLogRow`](crate::AppRunLogRow); a `RunLogSink` knows how to *store* it.
//! `rpp_db` implements this against Postgres; tests can implement an in-memory one.

use crate::run_log::AppRunLogRow;

/// Stores an `app_run_logs` row. Implementors own the transport (PG, etc.).
#[allow(async_fn_in_trait)]
pub trait RunLogSink {
    type Error: std::error::Error + Send + Sync + 'static;

    /// INSERT one row into `util.app_run_logs`.
    async fn insert_app_run_log(&self, row: &AppRunLogRow) -> Result<(), Self::Error>;
}
