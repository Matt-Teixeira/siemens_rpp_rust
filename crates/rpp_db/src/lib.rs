//! Postgres access for the fleet: a TLS-enabled, pooled `tokio-postgres` client
//! plus the `util.app_run_logs` writer.
//!
//! **TLS is required** (TD-020): the server enforces SSL. We mirror the Node
//! client's three-mode `buildSsl()` (`utils/db/pg-pool.js`) exactly:
//!
//! - `disable`               → no TLS (`NoTls`). Local Docker only.
//! - `require`               → encrypt, **no CA, no verification**
//!   (`{ rejectUnauthorized: false }`). This is what the live `.env` uses.
//! - `verify-ca`/`verify-full` → load the CA from `PG_SSL_PATH` and verify the
//!   chain (`{ ca, rejectUnauthorized: true }`); if the CA file is missing, Node
//!   warns and falls back to `require` — we do the same.

mod app_run_log;
mod pool;

pub use app_run_log::PgRunLogSink;
pub use pool::{build_pool, get_client, DbConfig, Pool, SslMode};

/// Errors from building the pool / TLS connector or acquiring a client.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("failed to read CA certificate at {path}: {source}")]
    CaRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("TLS connector build failed: {0}")]
    Tls(#[from] native_tls::Error),
    #[error("pool build failed: {0}")]
    PoolBuild(#[from] deadpool_postgres::BuildError),
    #[error("failed to get a client from the pool: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),
    #[error("postgres error: {0}")]
    Postgres(#[from] tokio_postgres::Error),
}

pub type Result<T> = std::result::Result<T, DbError>;
