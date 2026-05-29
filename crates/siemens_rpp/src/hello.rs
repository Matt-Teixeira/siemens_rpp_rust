//! Phase 0 smoke path: open a run, log a couple of events, and persist them to
//! `util.app_run_logs` + the run-log file — proving the rpp_log + rpp_db wiring
//! (and the TLS PG connection) end to end. Replaced by the real runner in Phase 1.

use rpp_db::{build_pool, PgRunLogSink};
use rpp_log::{RunLog, Tag, Type};
use serde_json::json;

use crate::config::Config;
use crate::error::AppError;

/// Write one "hello" run for the given job label (`ct` / `mri` / `parse`).
pub async fn hello_run(cfg: &Config, job: &str) -> Result<(), AppError> {
    let mut run = RunLog::new(&cfg.app_name);
    tracing::info!(run_id = run.run_id(), job, "starting hello run");

    run.add(
        Type::Info,
        "hello_run",
        Tag::Call,
        json!({ "job": job, "message": "Phase 0 hello run", "run_env": cfg.run_env }),
    );

    // Build the TLS pool and persist via the PG sink.
    let pool = build_pool(&cfg.db)?;
    let sink = PgRunLogSink::new(&pool);

    // Honest wording: the app_run_logs INSERT *is* what serializes this buffer, so
    // a "Successful Insert" event can't truthfully sit inside its own payload here.
    // (The real runner can log data-insert success because that INSERT — into
    // log.siemens_* — is a separate, earlier step from the final run-log INSERT.)
    run.add(
        Type::Info,
        "hello_run",
        Tag::Details,
        json!({ "job": job, "message": "persisting hello run to util.app_run_logs" }),
    );

    let path = cfg.run_log_path(run.run_id());
    let row = run.finish(&sink, &path).await?;

    tracing::info!(
        run_id = %row.run_id,
        app_name = %row.app_name,
        file = %path,
        "hello run persisted to util.app_run_logs and disk"
    );
    Ok(())
}
