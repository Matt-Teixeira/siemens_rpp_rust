//! The per-run event buffer and the end-of-run persistence shape.

use chrono::Utc;
use uuid::Uuid;

use crate::enums::{Tag, Type};
use crate::event::LogEvent;
use crate::sink::RunLogSink;
use crate::{LogError, Result};

/// The row written to `util.app_run_logs` — columns match the Node ColumnSet
/// `["app_name", "run_id", "verbose_log", "warn_error_logs"]`.
#[derive(Debug, Clone)]
pub struct AppRunLogRow {
    pub app_name: String,
    pub run_id: String,
    /// `JSON.stringify(log_events)`.
    pub verbose_log: String,
    /// `JSON.stringify(log_events filtered to WARN+ERROR)`.
    pub warn_error_logs: String,
}

/// An in-flight run: a `run_id` plus the accumulating event buffer.
///
/// Mirrors Node's `{ run_id, log_events }`. Build with [`RunLog::new`], append with
/// [`RunLog::add`]/[`RunLog::add_error`], then [`RunLog::finish`] to persist.
#[derive(Debug)]
pub struct RunLog {
    run_id: String,
    app_name: String,
    events: Vec<LogEvent>,
}

impl RunLog {
    /// Start a new run with a v4 `run_id`.
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            run_id: Uuid::new_v4().to_string(),
            app_name: app_name.into(),
            events: Vec::new(),
        }
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn app_name(&self) -> &str {
        &self.app_name
    }

    pub fn events(&self) -> &[LogEvent] {
        &self.events
    }

    /// Current UTC timestamp in `toISOString()` shape: millisecond precision with a
    /// `Z` suffix (e.g. `2026-05-29T19:00:00.000Z`).
    fn now_iso() -> String {
        Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    }

    /// Append an INFO/WARN event (no `err_msg`).
    pub fn add(
        &mut self,
        event_type: Type,
        func: impl Into<String>,
        tag: Tag,
        note: serde_json::Value,
    ) {
        self.events.push(LogEvent::new(
            self.run_id.clone(),
            Self::now_iso(),
            event_type,
            func,
            tag,
            note,
        ));
    }

    /// Append an ERROR event with an `err_msg` (mirrors Node attaching the stack).
    pub fn add_error(
        &mut self,
        func: impl Into<String>,
        tag: Tag,
        note: serde_json::Value,
        err_msg: impl Into<String>,
    ) {
        self.events.push(
            LogEvent::new(
                self.run_id.clone(),
                Self::now_iso(),
                Type::Error,
                func,
                tag,
                note,
            )
            .with_err(err_msg),
        );
    }

    /// Serialize the buffer into the `util.app_run_logs` row shape.
    pub fn to_row(&self) -> Result<AppRunLogRow> {
        let verbose_log = serde_json::to_string(&self.events)?;
        let warn_error: Vec<&LogEvent> = self
            .events
            .iter()
            .filter(|e| e.event_type.is_warn_or_error())
            .collect();
        let warn_error_logs = serde_json::to_string(&warn_error)?;
        Ok(AppRunLogRow {
            app_name: self.app_name.clone(),
            run_id: self.run_id.clone(),
            verbose_log,
            warn_error_logs,
        })
    }

    /// Bytes for the on-disk run-log file: `JSON.stringify(log_events)`.
    pub fn file_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&self.events)?)
    }

    /// Write the run-log file to `path` (parent dirs must exist or be creatable).
    pub async fn write_file(&self, path: &str) -> Result<()> {
        let bytes = self.file_bytes()?;
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|source| LogError::FileWrite {
                        path: path.to_string(),
                        source,
                    })?;
            }
        }
        tokio::fs::write(path, &bytes)
            .await
            .map_err(|source| LogError::FileWrite {
                path: path.to_string(),
                source,
            })
    }

    /// Persist the run: INSERT the row via the sink, then write the file. Returns
    /// the persisted row for inspection/logging.
    pub async fn finish<S: RunLogSink>(&self, sink: &S, file_path: &str) -> Result<AppRunLogRow> {
        let row = self.to_row()?;
        sink.insert_app_run_log(&row)
            .await
            .map_err(|e| LogError::Sink(Box::new(e)))?;
        self.write_file(file_path).await?;
        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn now_iso_has_millis_and_z() {
        let s = RunLog::now_iso();
        // e.g. 2026-05-29T19:00:00.000Z  -> 24 chars, ends with Z, has a dot
        assert!(s.ends_with('Z'), "expected Z suffix: {s}");
        assert_eq!(s.len(), 24, "expected toISOString length: {s}");
        assert_eq!(s.as_bytes()[19], b'.', "expected millis separator: {s}");
    }

    #[test]
    fn warn_error_filter_only_keeps_warn_and_error() {
        let mut rl = RunLog::new("siemens_rpp");
        rl.add(Type::Info, "f", Tag::Call, json!({"a": 1}));
        rl.add(Type::Warn, "f", Tag::Details, json!({"b": 2}));
        rl.add_error("f", Tag::Catch, json!({"c": 3}), "boom");

        let row = rl.to_row().unwrap();
        let verbose: serde_json::Value = serde_json::from_str(&row.verbose_log).unwrap();
        let we: serde_json::Value = serde_json::from_str(&row.warn_error_logs).unwrap();
        assert_eq!(verbose.as_array().unwrap().len(), 3);
        assert_eq!(we.as_array().unwrap().len(), 2);
    }

    #[test]
    fn err_msg_only_present_on_error() {
        let mut rl = RunLog::new("siemens_rpp");
        rl.add(Type::Info, "f", Tag::Call, json!({}));
        rl.add_error("f", Tag::Catch, json!({}), "stack-trace");
        let verbose = serde_json::to_string(rl.events()).unwrap();
        // exactly one err_msg key across the buffer
        assert_eq!(verbose.matches("err_msg").count(), 1, "{verbose}");
    }

    #[test]
    fn row_columns_match_node_shape() {
        let rl = RunLog::new("siemens_rpp");
        let row = rl.to_row().unwrap();
        assert_eq!(row.app_name, "siemens_rpp");
        assert_eq!(row.verbose_log, "[]");
        assert_eq!(row.warn_error_logs, "[]");
        assert_eq!(row.run_id.len(), 36); // uuid v4
    }
}
