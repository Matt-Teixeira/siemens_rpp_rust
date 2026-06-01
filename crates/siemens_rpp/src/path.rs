//! File-path construction for a system's log file.
//!
//! Resolved shape (TD-015, source-verified against `acquisition/Siemens_10.js:15`
//! and the live boot data): `{ACQU_FILES_ROOT}/{system_id}/{file_name}`.
//! `dir_name` is **not** used for siemens. A future `PathBuilder` keyed on the
//! manufacturer/modality pair can host other vendors; siemens v1 is the one rule.

use std::path::PathBuf;

/// Build the absolute path to a system's log file.
pub fn log_file_path(acqu_files_root: &str, system_id: &str, file_name: &str) -> PathBuf {
    PathBuf::from(acqu_files_root)
        .join(system_id)
        .join(file_name)
}

#[cfg(test)]
mod tests {
    use super::log_file_path;

    #[test]
    fn builds_root_id_file() {
        let p = log_file_path("/opt/resources/acqu_files", "SME21862", "Application.log");
        assert_eq!(
            p.to_str().unwrap(),
            "/opt/resources/acqu_files/SME21862/Application.log"
        );
    }
}
