//! The `parse` runner: single-system parse for testing / one-off retry.
//!
//! Phase 1 scope is `--dry-run`: boot one system → build path → read+scan the log
//! file → print the mapped rows as a JSON array to stdout (diffable against Node's
//! `mappedData`). No PG/Redis/gzip writes. `host_datetime`/`capture_datetime` are
//! left null until Phase 2.

use std::io::{BufRead, BufReader};

use rpp_db::build_pool;
use rpp_redis::RedisClient;

use crate::boot::{boot_one, Modality};
use crate::cli::ParseArgs;
use crate::config::Config;
use crate::error::AppError;
use crate::parse::scan_lines;
use crate::path::log_file_path;
use crate::types::Row;

/// Run `siemens_rpp parse`.
pub async fn run_parse(cfg: &Config, args: &ParseArgs) -> Result<(), AppError> {
    let system_id = args
        .system_id
        .as_deref()
        .ok_or_else(|| AppError::Config("parse requires --system-id".to_string()))?;
    let modality = Modality::parse(
        args.modality
            .as_deref()
            .ok_or_else(|| AppError::Config("parse requires --modality ct|mri".to_string()))?,
    )?;

    // Boot the one system from the live worklist.
    let pool = build_pool(&cfg.db)?;
    let system = boot_one(&pool, modality, system_id).await?;

    let parser_name =
        system.log_config.parsers.first().ok_or_else(|| {
            AppError::Config(format!("system {system_id} has no parsers configured"))
        })?;

    // File path: explicit --file override, else ACQU_FILES_ROOT/<id>/<file_name>.
    let path = match &args.file {
        Some(f) => std::path::PathBuf::from(f),
        None => log_file_path(
            &cfg.acqu_files_root,
            &system.id,
            &system.log_config.file_name,
        ),
    };

    // Missing file → warn + empty result (Node `is_file_present` → return).
    if !path.exists() {
        tracing::warn!(system_id, path = %path.display(), "log file not found; nothing to parse");
        print_rows(&[])?;
        return Ok(());
    }

    // Cursor: read-only here. Skipped entirely with --no-cursor-update; otherwise
    // read (never written in dry-run) so the scan stops at the previous head.
    let redis_line = if args.no_cursor_update {
        None
    } else {
        read_cursor(cfg, &system.id, &system.log_config.file_name).await
    };

    // Read + strip CRLF (TD-016): BufRead::lines() drops '\n'; strip a trailing '\r'.
    let file = std::fs::File::open(&path)
        .map_err(|e| AppError::Config(format!("failed to open {}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    let mut lines: Vec<String> = Vec::new();
    for line in reader.lines() {
        let line =
            line.map_err(|e| AppError::Config(format!("read error in {}: {e}", path.display())))?;
        lines.push(line.strip_suffix('\r').unwrap_or(&line).to_string());
    }

    let scan = scan_lines(
        lines.iter().map(String::as_str),
        redis_line.as_deref(),
        parser_name,
        &system.id,
    )
    .map_err(AppError::Config)?;

    if scan.bad_match_count > 0 {
        tracing::warn!(
            system_id,
            count = scan.bad_match_count,
            "non-blank lines failed the regex (Bad Match) — warn-and-skip (TD-017)"
        );
    }
    tracing::info!(
        system_id,
        rows = scan.rows.len(),
        cursor_hit = scan.cursor_hit,
        dry_run = args.dry_run,
        "parse complete"
    );

    // Phase 1 is dry-run only; persistence arrives in later phases.
    if !args.dry_run {
        return Err(AppError::Config(
            "parse currently supports --dry-run only (persistence is a later phase)".to_string(),
        ));
    }

    print_rows(&scan.rows)?;
    Ok(())
}

/// Best-effort cursor read; a Redis failure is logged and treated as "no cursor"
/// (dry-run never writes Redis, so this only affects where the scan would stop).
async fn read_cursor(cfg: &Config, sme: &str, file_name: &str) -> Option<String> {
    match RedisClient::connect(&cfg.redis_host, cfg.redis_port).await {
        Ok(mut client) => match client.get_cursor(sme, file_name).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "failed to read redis cursor; scanning whole file");
                None
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "failed to connect to redis; scanning whole file");
            None
        }
    }
}

/// Print rows as a JSON array to stdout (diffable against Node `mappedData`).
fn print_rows(rows: &[Row]) -> Result<(), AppError> {
    let json = serde_json::to_string_pretty(rows)
        .map_err(|e| AppError::Config(format!("failed to serialize rows: {e}")))?;
    println!("{json}");
    Ok(())
}
