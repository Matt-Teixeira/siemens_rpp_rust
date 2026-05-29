//! Redis access mirroring the Node contract: a plain host+port connection (no
//! auth, no TLS, no db selector) and the cursor key convention `${sme}.${file}`.
//!
//! The cursor value is the file's most-recent first line, stored **raw**. CRLF
//! handling (stripping a trailing `\r` symmetrically) is the caller's job at the
//! scan boundary — see TD-016 — so this crate stores/returns bytes-as-given.

use redis::AsyncCommands;

/// Errors from connecting to or talking with Redis.
#[derive(Debug, thiserror::Error)]
pub enum RedisError {
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
}

pub type Result<T> = std::result::Result<T, RedisError>;

/// A thin Redis client holding the multiplexed connection.
#[derive(Clone)]
pub struct RedisClient {
    conn: redis::aio::MultiplexedConnection,
}

impl RedisClient {
    /// Connect using the Node convention: `redis://{host}:{port}` — no auth/TLS/db.
    pub async fn connect(host: &str, port: u16) -> Result<Self> {
        let client = redis::Client::open(format!("redis://{host}:{port}"))?;
        let conn = client.get_multiplexed_async_connection().await?;
        Ok(Self { conn })
    }

    /// The cursor key for a system+file: `${sme}.${file}` (e.g.
    /// `SME00817.EvtApplication_Today.txt`).
    pub fn cursor_key(sme: &str, file: &str) -> String {
        format!("{sme}.{file}")
    }

    /// GET the stored cursor line. Returns `None` if the key is absent (Node
    /// treats a missing key as "no previous run").
    pub async fn get_cursor(&mut self, sme: &str, file: &str) -> Result<Option<String>> {
        let key = Self::cursor_key(sme, file);
        let val: Option<String> = self.conn.get(&key).await?;
        Ok(val)
    }

    /// SET the cursor line. Stored raw (no trim/normalize beyond what the caller
    /// already applied).
    pub async fn set_cursor(&mut self, sme: &str, file: &str, line: &str) -> Result<()> {
        let key = Self::cursor_key(sme, file);
        let _: () = self.conn.set(&key, line).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_key_matches_node_shape() {
        assert_eq!(
            RedisClient::cursor_key("SME00817", "EvtApplication_Today.txt"),
            "SME00817.EvtApplication_Today.txt"
        );
    }
}
