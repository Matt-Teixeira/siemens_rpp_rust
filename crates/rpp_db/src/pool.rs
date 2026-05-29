//! Pool construction with a TLS connector that mirrors the Node `pg` SSL config.

use deadpool_postgres::{Manager, ManagerConfig, RecyclingMethod};
use postgres_native_tls::MakeTlsConnector;
use tokio_postgres::Config as PgConfig;

use crate::Result;

pub type Pool = deadpool_postgres::Pool;

/// SSL posture, mirroring the Node `buildSsl()` three modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SslMode {
    /// No TLS. Local Docker only — will fail against the SSL-enforcing server.
    Disable,
    /// Encrypt but do **not** verify: no CA loaded, accept any cert/hostname.
    /// Matches Node `require` → `{ rejectUnauthorized: false }`. The live `.env`
    /// uses this.
    Require,
    /// Load the CA from `ssl_ca_path` and verify the chain. Matches Node
    /// `verify-ca`/`verify-full` → `{ ca, rejectUnauthorized: true }`. Falls back
    /// to `Require` behavior if the CA file is missing (Node warns + falls back).
    Verify,
}

/// Connection configuration. Field names map to the `PG*` env vars used by the
/// existing fleet `.env` (`PGHOST`, `PGPORT`, `PGUSER`, `PGPASSWORD`,
/// `PGDATABASE`, `PG_SSLMODE`, `PG_SSL_PATH`).
#[derive(Debug, Clone)]
pub struct DbConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub dbname: String,
    pub ssl_mode: SslMode,
    /// Path to the CA certificate (PEM). Used **only** for `verify-ca`/`verify-full`
    /// (`SslMode::Verify`), where it is loaded as the trusted root. Ignored for
    /// `disable` and `require` — matching Node `buildSsl()`, which reads the CA only
    /// on the `verify-*` path.
    pub ssl_ca_path: Option<String>,
    /// Max pool size.
    pub max_size: usize,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            user: "postgres".to_string(),
            password: String::new(),
            dbname: "postgres".to_string(),
            ssl_mode: SslMode::Require,
            ssl_ca_path: None,
            max_size: 16,
        }
    }
}

/// The resolved TLS plan for a `(mode, has-CA-path)` combination. Pure and
/// testable; the actual cert read / connector build happens in `build_tls`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TlsDecision {
    /// `verify-ca`/`verify-full` with a CA path configured: load CA and verify.
    VerifyWithCa,
    /// `require`: encrypt, no CA, no verification (`rejectUnauthorized: false`).
    EncryptNoVerify,
    /// `verify-*` requested but **no** CA path configured. Node warns and falls
    /// back to `require` here; we do the same (the warning is emitted in
    /// `build_tls`). Kept distinct so the warn-worthy case is testable.
    VerifyFallbackNoCa,
}

/// Decide the TLS plan from the mode and whether a non-empty CA path is set.
/// `Disable` is handled in `build_pool` and never reaches here.
fn decide_tls(mode: SslMode, has_ca_path: bool) -> TlsDecision {
    match mode {
        SslMode::Verify if has_ca_path => TlsDecision::VerifyWithCa,
        SslMode::Verify => TlsDecision::VerifyFallbackNoCa,
        SslMode::Require | SslMode::Disable => TlsDecision::EncryptNoVerify,
    }
}

/// Build a TLS connector matching the Node `buildSsl()` behavior for the given
/// mode. `Disable` never reaches here (handled in `build_pool`).
fn build_tls(cfg: &DbConfig) -> Result<MakeTlsConnector> {
    let mut builder = native_tls::TlsConnector::builder();

    let has_ca = cfg.ssl_ca_path.as_deref().is_some_and(|p| !p.is_empty());

    // `accept_invalid_*` together implement `rejectUnauthorized: false` (encrypt
    // without verifying chain or hostname).
    let accept_unverified = |b: &mut native_tls::TlsConnectorBuilder| {
        b.danger_accept_invalid_hostnames(true);
        b.danger_accept_invalid_certs(true);
    };

    match decide_tls(cfg.ssl_mode, has_ca) {
        TlsDecision::VerifyWithCa => {
            let path = cfg.ssl_ca_path.as_deref().unwrap();
            match std::fs::read(path) {
                Ok(pem) => {
                    let cert = native_tls::Certificate::from_pem(&pem)?;
                    builder.add_root_certificate(cert);
                    // rejectUnauthorized: true — verify chain and hostname.
                }
                Err(source) => {
                    // Node warns and falls back to `require` when the CA file is
                    // unreadable. Surface the path; don't hard-fail.
                    tracing::warn!(
                        path,
                        error = %source,
                        "PG_SSL_PATH unreadable; falling back to encrypted-but-unverified (require)"
                    );
                    accept_unverified(&mut builder);
                }
            }
        }
        TlsDecision::VerifyFallbackNoCa => {
            // Finding-1 fix: an operator asked for verify-* but set no CA path. Node
            // warns before downgrading; previously this path was silent.
            tracing::warn!(
                "PG_SSLMODE=verify-* but PG_SSL_PATH is unset/empty; \
                 falling back to encrypted-but-unverified (require)"
            );
            accept_unverified(&mut builder);
        }
        TlsDecision::EncryptNoVerify => {
            // `require`: encrypt, no CA, no verification (rejectUnauthorized: false).
            accept_unverified(&mut builder);
        }
    }

    Ok(MakeTlsConnector::new(builder.build()?))
}

/// Build a pooled, TLS-enabled Postgres connection pool.
pub fn build_pool(cfg: &DbConfig) -> Result<Pool> {
    let mut pg = PgConfig::new();
    pg.host(&cfg.host)
        .port(cfg.port)
        .user(&cfg.user)
        .password(&cfg.password)
        .dbname(&cfg.dbname);

    let mgr_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    };

    // `require` and `verify-*` both use TLS; build_tls() handles the verify-vs-not
    // distinction internally. Only `disable` uses NoTls.
    let manager = match cfg.ssl_mode {
        SslMode::Require | SslMode::Verify => {
            let tls = build_tls(cfg)?;
            Manager::from_config(pg, tls, mgr_config)
        }
        SslMode::Disable => Manager::from_config(pg, tokio_postgres::NoTls, mgr_config),
    };

    Ok(Pool::builder(manager).max_size(cfg.max_size).build()?)
}

/// Acquire a client and apply a per-session `statement_timeout` (ms). Pass 0 to
/// skip setting a timeout.
pub async fn get_client(
    pool: &Pool,
    statement_timeout_ms: u64,
) -> Result<deadpool_postgres::Object> {
    let client = pool.get().await?;
    if statement_timeout_ms > 0 {
        client
            .batch_execute(&format!("SET statement_timeout = {statement_timeout_ms}"))
            .await?;
    }
    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::{decide_tls, SslMode, TlsDecision};

    #[test]
    fn require_always_encrypts_without_verifying() {
        assert_eq!(
            decide_tls(SslMode::Require, true),
            TlsDecision::EncryptNoVerify
        );
        assert_eq!(
            decide_tls(SslMode::Require, false),
            TlsDecision::EncryptNoVerify
        );
    }

    #[test]
    fn verify_with_ca_path_verifies() {
        assert_eq!(decide_tls(SslMode::Verify, true), TlsDecision::VerifyWithCa);
    }

    #[test]
    fn verify_without_ca_path_falls_back_with_warning() {
        // The warn-worthy downgrade (Codex finding 1) is a distinct, testable value.
        assert_eq!(
            decide_tls(SslMode::Verify, false),
            TlsDecision::VerifyFallbackNoCa
        );
    }
}
