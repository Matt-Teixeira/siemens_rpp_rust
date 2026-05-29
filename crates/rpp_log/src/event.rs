//! A single log event. Field order and naming mirror the Node `addLogEvent`
//! object: `{ run_id, dt, type, func, tag, note, err_msg? }`.

use serde::{Deserialize, Serialize};

use crate::enums::{Tag, Type};

/// One run-log event.
///
/// JSON shape (matches Node):
/// ```json
/// { "run_id": "...", "dt": "2026-05-29T19:00:00.000Z", "type": "INFO",
///   "func": "win10_siemens_ct", "tag": "CALL", "note": { ... } }
/// ```
/// `err_msg` is present **only** on ERROR events (Node adds it conditionally), so
/// it is skipped when `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub run_id: String,
    /// UTC timestamp, `new Date().toISOString()` shape (millis + `Z`). This is
    /// deliberately UTC — distinct from the NY-zoned `host_datetime`.
    pub dt: String,
    #[serde(rename = "type")]
    pub event_type: Type,
    pub func: String,
    pub tag: Tag,
    /// Arbitrary structured payload; `serde_json::Value` keeps Node
    /// `JSON.stringify(note)` semantics.
    pub note: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub err_msg: Option<String>,
}

impl LogEvent {
    /// Build an event with the given UTC `dt` string (caller supplies the clock so
    /// this stays testable and the crate avoids hidden time deps).
    pub fn new(
        run_id: impl Into<String>,
        dt: impl Into<String>,
        event_type: Type,
        func: impl Into<String>,
        tag: Tag,
        note: serde_json::Value,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            dt: dt.into(),
            event_type,
            func: func.into(),
            tag,
            note,
            err_msg: None,
        }
    }

    /// Attach an error message (sets `err_msg`); typically used with `Type::Error`.
    pub fn with_err(mut self, err_msg: impl Into<String>) -> Self {
        self.err_msg = Some(err_msg.into());
        self
    }
}
