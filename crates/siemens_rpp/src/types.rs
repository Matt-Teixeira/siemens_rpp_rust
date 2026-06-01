//! Domain types for the parse pipeline: the boot-query row, its `log_config`, and
//! the mapped output row.

use serde::{Deserialize, Serialize};

/// One row from the `SIEMENS_CT` / `SIEMENS_MRI` boot query
/// (`acquisition/on_boot_queries.js`). Column order/names mirror the SELECT.
#[derive(Debug, Clone, Deserialize)]
pub struct SystemRow {
    /// `sys.id`, e.g. `SME21862`.
    pub id: String,
    // Read from the boot query for completeness/parity; consumed in later phases.
    #[allow(dead_code)]
    pub manufacturer: String,
    #[allow(dead_code)]
    pub modality: String,
    /// `sites.time_zone_id`, IANA name; may be NULL (→ default `America/New_York`).
    /// Used for `host_datetime` construction in Phase 2.
    #[allow(dead_code)]
    pub time_zone_id: Option<String>,
    /// `ac.debian_server_path` — the source host path (not used for the Rust file
    /// read; the Rust app builds its own path from `ACQU_FILES_ROOT`).
    #[allow(dead_code)]
    pub debian_server_path: Option<String>,
    pub log_config: LogConfig,
}

/// The `log_config` JSON object built by `json_build_object(...)` in the boot query.
#[derive(Debug, Clone, Deserialize)]
pub struct LogConfig {
    pub file_name: String,
    /// Not used for siemens paths (TD-015); kept for parity with the boot JSON.
    #[allow(dead_code)]
    pub dir_name: Option<String>,
    /// `log.regex_models`, e.g. `["re_v1"]`. `parsers[0]` selects the regex variant.
    pub parsers: Vec<String>,
    /// `log.pg_tables`, e.g. `["siemens_ct"]` — the persist target (Phase ≥ later).
    #[allow(dead_code)]
    pub pg_tables: Vec<String>,
    /// `ac.file_version` — `win_10` (default) vs `win_7` (deferred) dispatch.
    #[allow(dead_code)]
    pub file_version: Option<String>,
}

/// A mapped output row, matching the Node `siemens_ct_mri` schema
/// (`persist/pg-schemas.js`) — **field order is the serialization order** and must
/// stay identical to Node's `mapDataToSchema` output.
///
/// Fields the win_10 regex never captures (`domain_group`, `id_group`, `month`,
/// `day`, `year`) stay `None` → serialize as `null`, exactly as Node leaves them.
/// `host_datetime` and `capture_datetime` are datetime work (Phase 2); they are
/// `None` in Phase 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    pub system_id: String,
    pub host_state: Option<String>,
    pub host_date: Option<String>,
    pub host_time: Option<String>,
    pub source_group: Option<String>,
    pub type_group: Option<String>,
    pub text_group: Option<String>,
    pub domain_group: Option<String>,
    pub id_group: Option<String>,
    pub month: Option<String>,
    pub day: Option<String>,
    pub year: Option<String>,
    pub host_datetime: Option<String>,
    pub capture_datetime: Option<String>,
}
