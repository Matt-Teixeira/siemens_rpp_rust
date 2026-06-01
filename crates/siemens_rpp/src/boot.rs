//! Boot queries: materialize the Siemens worklist for a modality.
//!
//! The SQL is copied verbatim from the Node `acquisition/on_boot_queries.js`
//! (`SIEMENS_CT` / `SIEMENS_MRI`). Each row maps to a [`SystemRow`]; `log_config` is
//! a `json` column deserialized into [`LogConfig`](crate::types::LogConfig).

use rpp_db::{get_client, Pool};

use crate::error::AppError;
use crate::types::{LogConfig, SystemRow};

/// Which boot worklist to materialize.
#[derive(Debug, Clone, Copy)]
pub enum Modality {
    Ct,
    Mri,
}

impl Modality {
    /// Parse a CLI/string modality (case-insensitive: `ct` / `mri`).
    pub fn parse(s: &str) -> Result<Self, AppError> {
        match s.to_lowercase().as_str() {
            "ct" => Ok(Modality::Ct),
            "mri" => Ok(Modality::Mri),
            other => Err(AppError::Config(format!(
                "unknown modality: {other:?} (use ct|mri)"
            ))),
        }
    }

    fn boot_sql(self) -> &'static str {
        match self {
            Modality::Ct => SIEMENS_CT,
            Modality::Mri => SIEMENS_MRI,
        }
    }
}

/// Run the boot query for `modality` and return the worklist.
pub async fn boot_systems(pool: &Pool, modality: Modality) -> Result<Vec<SystemRow>, AppError> {
    let client = get_client(pool, 0).await?;
    let rows = client
        .query(modality.boot_sql(), &[])
        .await
        .map_err(rpp_db::DbError::from)?;
    rows.iter().map(row_to_system).collect()
}

/// Run the boot query and return only the system matching `system_id` (for
/// `parse --system-id`), or an error if not found.
pub async fn boot_one(
    pool: &Pool,
    modality: Modality,
    system_id: &str,
) -> Result<SystemRow, AppError> {
    let systems = boot_systems(pool, modality).await?;
    systems
        .into_iter()
        .find(|s| s.id == system_id)
        .ok_or_else(|| {
            AppError::Config(format!(
                "system {system_id} not found in the {modality:?} boot worklist"
            ))
        })
}

fn row_to_system(row: &tokio_postgres::Row) -> Result<SystemRow, AppError> {
    // log_config is a `json` column → comes back as serde_json::Value, then into LogConfig.
    let log_config_json: serde_json::Value =
        row.try_get("log_config").map_err(rpp_db::DbError::from)?;
    let log_config: LogConfig = serde_json::from_value(log_config_json)
        .map_err(|e| AppError::Config(format!("invalid log_config json: {e}")))?;

    Ok(SystemRow {
        id: row.try_get("id").map_err(rpp_db::DbError::from)?,
        manufacturer: row.try_get("manufacturer").map_err(rpp_db::DbError::from)?,
        modality: row.try_get("modality").map_err(rpp_db::DbError::from)?,
        time_zone_id: row.try_get("time_zone_id").map_err(rpp_db::DbError::from)?,
        debian_server_path: row
            .try_get("debian_server_path")
            .map_err(rpp_db::DbError::from)?,
        log_config,
    })
}

// --- Boot SQL, verbatim from acquisition/on_boot_queries.js ---

const SIEMENS_CT: &str = r#"
  SELECT
	sys.id,
    sys.manufacturer,
    sys.modality,
    sites.time_zone_id,
    ac.debian_server_path,
		json_build_object(
			'file_name',
			log.file_name,
			'dir_name',
			log.dir_name,
			'parsers',
			log.regex_models,
			'pg_tables',
			log.pg_tables,
			'file_version',
			ac.file_version
		) AS log_config
FROM
	systems sys
	JOIN config.acquisition ac ON ac.system_id = sys.id
	JOIN config.log log ON log.system_id = sys.id
	JOIN sites ON sites.id = sys.site_id
WHERE
	sys.manufacturer = 'Siemens'
	AND sys.modality LIKE '%CT'
	AND ac.run_group = 1
  AND sys.process_log IS TRUE
GROUP BY
  sys.id,
  sys.manufacturer,
  sys.modality,
  ac.system_id,
  log.file_name,
  log.dir_name,
  log.regex_models,
  log.pg_tables,
  ac.file_version,
  ac.debian_server_path,
  sites.time_zone_id;
"#;

const SIEMENS_MRI: &str = r#"
    SELECT
    sys.id,
    sys.manufacturer,
    sys.modality,
	sites.time_zone_id,
    ac.debian_server_path,
        json_build_object(
            'file_name',
            log.file_name,
            'dir_name',
            log.dir_name,
            'parsers',
            log.regex_models,
            'pg_tables',
            log.pg_tables,
            'file_version',
            ac.file_version
        ) AS log_config
  FROM
    systems sys
    JOIN config.acquisition ac ON ac.system_id = sys.id
    JOIN config.log log ON log.system_id = sys.id
	JOIN sites ON sites.id = sys.site_id
  WHERE
    sys.manufacturer = 'Siemens'
    AND sys.modality = 'MRI'
    AND ac.run_group = 1
    AND sys.process_log IS TRUE
  GROUP BY
    sys.id,
    ac.system_id,
    log.file_name,
    log.dir_name,
    log.regex_models,
    log.pg_tables,
    ac.file_version,
	sites.time_zone_id;
"#;
