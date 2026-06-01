//! Configuration loaded from the environment (`.env` via dotenvy in `main`).
//!
//! Env var names match the existing fleet `.env` contract (libpq-style `PG*` for
//! Postgres, `REDIS_*` for Redis, `ACQU_FILES_ROOT` for the file store — renamed
//! from Node's `DATA_STORE_DEV`, see TD-015). No secrets are hardcoded.

use rpp_db::{DbConfig, SslMode};

use crate::error::AppError;

/// Fully-resolved application configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub app_name: String,
    pub run_env: String,
    pub logger: String,
    pub db: DbConfig,
    // Loaded and validated now so the env contract is exercised from Phase 0, but
    // not consumed until the Redis cursor (Phase 1/2) and file scan (Phase 1) land.
    #[allow(dead_code)]
    pub redis_host: String,
    #[allow(dead_code)]
    pub redis_port: u16,
    /// File-store root (`ACQU_FILES_ROOT`), e.g. `/opt/resources/acqu_files`.
    #[allow(dead_code)]
    pub acqu_files_root: String,
}

impl Config {
    /// Build config from environment variables, erroring on anything required but
    /// missing or unparseable.
    pub fn from_env() -> Result<Self, AppError> {
        let app_name = req("APP_NAME")?;
        let run_env = opt("RUN_ENV", "dev");
        let logger = opt("LOGGER", "dev");

        // Containerized runs must NOT use RUN_ENV=dev: dev resolves the run-log path
        // to a relative `./<file>` under the root-owned /workspace, while the process
        // runs as `svc` (gosu) — so the file write fails *after* the app has already
        // touched shared DB state. The compose service sets APP_CONTAINER=1; reject
        // the known-bad combination here, before any DB work. (Codex P0.5 finding 1.)
        let in_container = matches!(
            std::env::var("APP_CONTAINER").ok().as_deref(),
            Some("1") | Some("true") | Some("yes")
        );
        if is_disallowed_container_run_env(in_container, &run_env) {
            return Err(AppError::Config(
                "RUN_ENV=dev is not allowed in a container (run-log path would be a \
                 non-writable relative ./ path); use staging or prod"
                    .to_string(),
            ));
        }

        // Mirror Node buildSsl() mode mapping: disable | require | verify-ca/verify-full.
        //
        // INTENTIONAL DIVERGENCE (Codex finding 2): Node defaults a *missing*
        // PG_SSLMODE to `disable`; we default to `require`. The server enforces SSL,
        // so a missing var defaulting to `disable` would just produce a confusing
        // connection failure. Defaulting to `require` still connects (encrypted,
        // unverified). In practice the var is always set in the fleet .env, so this
        // default is a safety net, not a behavioral difference on real deployments.
        let ssl_mode = match opt("PG_SSLMODE", "require").to_lowercase().as_str() {
            "disable" => SslMode::Disable,
            "verify-ca" | "verify-full" => SslMode::Verify,
            _ => SslMode::Require,
        };
        let ssl_ca_path = std::env::var("PG_SSL_PATH").ok().filter(|s| !s.is_empty());

        let db = DbConfig {
            host: req("PGHOST")?,
            port: parse_port("PGPORT", 5432)?,
            user: req("PGUSER")?,
            password: req("PGPASSWORD")?,
            dbname: req("PGDATABASE")?,
            ssl_mode,
            ssl_ca_path,
            max_size: parse_usize("PG_POOL_MAX", 16)?,
        };

        Ok(Self {
            app_name,
            run_env,
            logger,
            db,
            redis_host: req("REDIS_HOST")?,
            redis_port: parse_port("REDIS_PORT", 6379)?,
            acqu_files_root: opt("ACQU_FILES_ROOT", "/opt/resources/acqu_files"),
        })
    }

    /// Run-log file path for this run, matching the Node convention:
    /// dev → `./<app>-log.<logger>.<run_id>.json`;
    /// staging/prod → `/opt/run-logs/<app>/<app>-log.<logger>.<run_id>.json`.
    pub fn run_log_path(&self, run_id: &str) -> String {
        let file = format!("{}-log.{}.{}.json", self.app_name, self.logger, run_id);
        if self.run_env == "dev" {
            format!("./{file}")
        } else {
            format!("/opt/run-logs/{}/{}", self.app_name, file)
        }
    }
}

/// True when a containerized run requested the known-bad `RUN_ENV=dev` (whose
/// relative run-log path isn't writable as `svc`). Pure so it's unit-testable
/// without touching the process environment.
fn is_disallowed_container_run_env(in_container: bool, run_env: &str) -> bool {
    in_container && run_env == "dev"
}

fn req(key: &str) -> Result<String, AppError> {
    std::env::var(key)
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Config(format!("missing required env var {key}")))
}

fn opt(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn parse_port(key: &str, default: u16) -> Result<u16, AppError> {
    match std::env::var(key).ok().filter(|s| !s.is_empty()) {
        None => Ok(default),
        Some(v) => v
            .parse::<u16>()
            .map_err(|_| AppError::Config(format!("{key} is not a valid port: {v}"))),
    }
}

fn parse_usize(key: &str, default: usize) -> Result<usize, AppError> {
    match std::env::var(key).ok().filter(|s| !s.is_empty()) {
        None => Ok(default),
        Some(v) => v
            .parse::<usize>()
            .map_err(|_| AppError::Config(format!("{key} is not a valid number: {v}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_rejects_dev_only() {
        // The one disallowed combination: in a container with RUN_ENV=dev.
        assert!(is_disallowed_container_run_env(true, "dev"));
        // Allowed: staging/prod in a container; dev only on the native host.
        assert!(!is_disallowed_container_run_env(true, "staging"));
        assert!(!is_disallowed_container_run_env(true, "prod"));
        assert!(!is_disallowed_container_run_env(false, "dev"));
        assert!(!is_disallowed_container_run_env(false, "staging"));
    }

    fn cfg(run_env: &str) -> Config {
        Config {
            app_name: "siemens_rpp".to_string(),
            run_env: run_env.to_string(),
            logger: "dev".to_string(),
            db: DbConfig::default(),
            redis_host: "h".to_string(),
            redis_port: 6379,
            acqu_files_root: "/opt/resources/acqu_files".to_string(),
        }
    }

    #[test]
    fn run_log_path_dev_is_relative() {
        assert_eq!(
            cfg("dev").run_log_path("abc"),
            "./siemens_rpp-log.dev.abc.json"
        );
    }

    #[test]
    fn run_log_path_staging_prod_use_run_logs_dir() {
        assert_eq!(
            cfg("staging").run_log_path("abc"),
            "/opt/run-logs/siemens_rpp/siemens_rpp-log.dev.abc.json"
        );
        assert_eq!(
            cfg("prod").run_log_path("abc"),
            "/opt/run-logs/siemens_rpp/siemens_rpp-log.dev.abc.json"
        );
    }
}
