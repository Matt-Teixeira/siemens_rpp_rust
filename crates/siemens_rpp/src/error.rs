//! The binary's error type. Library crates expose typed errors; here we collect
//! them behind one `AppError` and add `anyhow`-style context only at `main`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error(transparent)]
    Db(#[from] rpp_db::DbError),

    #[error(transparent)]
    Log(#[from] rpp_log::LogError),

    #[error(transparent)]
    Redis(#[from] rpp_redis::RedisError),
}

/// Convenience alias used by the per-stage modules added in Phase 1+.
#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, AppError>;
