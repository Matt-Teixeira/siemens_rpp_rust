//! The Postgres implementation of `rpp_log::RunLogSink`: one parameterized INSERT
//! into `util.app_run_logs` with the Node column set
//! `(app_name, run_id, verbose_log, warn_error_logs)`.

use rpp_log::{AppRunLogRow, RunLogSink};

use crate::pool::Pool;
use crate::DbError;

/// Writes `util.app_run_logs` rows through a [`Pool`].
pub struct PgRunLogSink<'a> {
    pool: &'a Pool,
    statement_timeout_ms: u64,
}

impl<'a> PgRunLogSink<'a> {
    pub fn new(pool: &'a Pool) -> Self {
        Self {
            pool,
            statement_timeout_ms: 0,
        }
    }

    /// Set a per-acquire `statement_timeout` (ms) applied in `get_client` when this
    /// sink runs its INSERT. Intentional API surface for the real runner; unused on
    /// the Phase 0 hello path (which leaves it at 0 = no timeout).
    pub fn with_statement_timeout(mut self, ms: u64) -> Self {
        self.statement_timeout_ms = ms;
        self
    }
}

impl RunLogSink for PgRunLogSink<'_> {
    type Error = DbError;

    async fn insert_app_run_log(&self, row: &AppRunLogRow) -> Result<(), Self::Error> {
        let client = crate::pool::get_client(self.pool, self.statement_timeout_ms).await?;
        // Live schema (verified against pg_db util.app_run_logs):
        //   app_name text, run_id uuid, verbose_log json, warn_error_logs json.
        //
        // We send every param as a Rust String. The casts are written `$N::text::T`
        // (NOT `$N::T`): a bare `$2::uuid` makes Postgres infer the parameter's type
        // as `uuid`, and tokio-postgres cannot serialize a String to the uuid wire
        // type (it only maps to `text`) — that produced "error serializing parameter".
        // `::text::uuid` pins the param's inferred type to `text` (which String does
        // serialize to), then casts text->uuid / text->json inside SQL. This keeps
        // the Node "send strings, let PG coerce" behavior. Fully parameterized.
        client
            .execute(
                "INSERT INTO util.app_run_logs \
                   (app_name, run_id, verbose_log, warn_error_logs) \
                 VALUES ($1, $2::text::uuid, $3::text::json, $4::text::json)",
                &[
                    &row.app_name,
                    &row.run_id,
                    &row.verbose_log,
                    &row.warn_error_logs,
                ],
            )
            .await?;
        Ok(())
    }
}
