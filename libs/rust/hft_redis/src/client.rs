//! [`RedisStore`] — `ConnectionManager` wrapper with typed read/write helpers.
//!
//! `ConnectionManager` automatically reconnects on network failures, so callers
//! never need to handle reconnection logic explicitly.
//!
//! `RedisStore` is `Clone`: it shares the same underlying connection pool.
//! Create one instance per service and clone into helpers:
//! ```rust,ignore
//! let store = RedisStore::new(url).await?;
//! let kill  = KillSwitch::new(store.clone());
//! let rl    = RateLimiter::new(store.clone());
//! ```

use redis::{aio::ConnectionManager, AsyncCommands, Client};
use serde::{de::DeserializeOwned, Serialize};
use tracing::error;

use hft_common::config::{env_or, load_dotenv};

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum RedisError {
    /// Redis command or protocol error (includes connection errors).
    #[error("redis error: {0}")]
    Command(#[from] redis::RedisError),

    /// JSON serialisation / deserialisation failure.
    #[error("codec error: {0}")]
    Codec(String),

    /// Invalid configuration (e.g. bad URL).
    #[error("config error: {0}")]
    Config(String),
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, RedisError>;

// ── RedisStore ────────────────────────────────────────────────────────────────

/// Auto-reconnecting Redis client with typed helpers.
#[derive(Clone)]
pub struct RedisStore {
    pub(crate) conn: ConnectionManager,
}

impl RedisStore {
    /// Connect to Redis and return a live store.
    ///
    /// `url` examples: `"redis://redis:6379/0"`, `"redis://:password@host:6379"`
    ///
    /// # Errors
    /// Returns [`RedisError::Config`] for invalid URLs and
    /// [`RedisError::Command`] if the initial connection attempt fails.
    pub async fn new(url: &str) -> Result<Self> {
        let client =
            Client::open(url).map_err(|e| RedisError::Config(format!("invalid Redis URL: {e}")))?;

        let conn = ConnectionManager::new(client)
            .await
            .map_err(|e| RedisError::Config(format!("Redis connection failed: {e}")))?;

        Ok(Self { conn })
    }

    /// Build from the `REDIS_URL` environment variable (default: `redis://redis:6379/0`).
    pub async fn from_env() -> Result<Self> {
        load_dotenv();
        let url = env_or("REDIS_URL", "redis://redis:6379/0");
        Self::new(&url).await
    }

    // ── Raw string ops ────────────────────────────────────────────────────────

    /// `GET key` → `Option<String>`.
    pub async fn get_str(&mut self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.get(key).await?)
    }

    /// `SET key value`.
    pub async fn set_str(&mut self, key: &str, val: &str) -> Result<()> {
        self.conn.set::<_, _, ()>(key, val).await?;
        Ok(())
    }

    /// `SETEX key seconds value`.
    pub async fn set_str_ex(&mut self, key: &str, val: &str, ttl_secs: u64) -> Result<()> {
        self.conn.set_ex::<_, _, ()>(key, val, ttl_secs).await?;
        Ok(())
    }

    /// `DEL key`.
    pub async fn del(&mut self, key: &str) -> Result<()> {
        self.conn.del::<_, ()>(key).await?;
        Ok(())
    }

    /// `EXISTS key` → bool.
    pub async fn exists(&mut self, key: &str) -> Result<bool> {
        Ok(self.conn.exists(key).await?)
    }

    /// `EXPIRE key seconds` — set / refresh TTL on an existing key.
    pub async fn expire(&mut self, key: &str, ttl_secs: u64) -> Result<bool> {
        Ok(self.conn.expire(key, ttl_secs as i64).await?)
    }

    // ── Boolean helpers ───────────────────────────────────────────────────────
    //
    // Convention: booleans are stored as the string "1" (true) or "0" (false).
    // A *missing* key is treated as false (consistent with kill-switch semantics:
    // delete the key to deactivate rather than writing "0").

    /// `GET key` → true if the stored value is `"1"`, false otherwise (including missing).
    pub async fn get_bool(&mut self, key: &str) -> Result<bool> {
        let val: Option<String> = self.conn.get(key).await?;
        Ok(matches!(val.as_deref(), Some("1")))
    }

    /// `SET key ("1"|"0")` — no expiry.
    pub async fn set_bool(&mut self, key: &str, val: bool) -> Result<()> {
        self.conn.set::<_, _, ()>(key, if val { "1" } else { "0" }).await?;
        Ok(())
    }

    /// `SETEX key ttl_secs ("1"|"0")` — matching the public API name.
    pub async fn set_bool_ttl(&mut self, key: &str, val: bool, ttl_secs: u64) -> Result<()> {
        self.conn
            .set_ex::<_, _, ()>(key, if val { "1" } else { "0" }, ttl_secs)
            .await?;
        Ok(())
    }

    // ── JSON helpers ──────────────────────────────────────────────────────────
    //
    // Used for structured state values: positions, signal state, instrument lists.

    /// `GET key` → deserialise JSON into `T`, or `None` if missing.
    pub async fn get_json<T: DeserializeOwned>(&mut self, key: &str) -> Result<Option<T>> {
        let raw: Option<String> = self.conn.get(key).await?;
        match raw {
            None => Ok(None),
            Some(s) => serde_json::from_str(&s)
                .map(Some)
                .map_err(|e| RedisError::Codec(format!("JSON decode error for key `{key}`: {e}"))),
        }
    }

    /// Serialise `val` to JSON and `SETEX key ttl_secs <json>`.
    pub async fn set_json_ex<T: Serialize>(
        &mut self,
        key: &str,
        val: &T,
        ttl_secs: u64,
    ) -> Result<()> {
        let s = serde_json::to_string(val)
            .map_err(|e| RedisError::Codec(format!("JSON encode error for key `{key}`: {e}")))?;
        self.conn.set_ex::<_, _, ()>(key, s, ttl_secs).await?;
        Ok(())
    }

    // ── Low-level access ──────────────────────────────────────────────────────

    /// Mutable reference to the inner [`ConnectionManager`].
    ///
    /// Needed for [`redis::Script::invoke_async`] which requires a
    /// `&mut impl ConnectionLike`. Prefer the typed helpers above.
    #[inline]
    pub fn conn_mut(&mut self) -> &mut ConnectionManager {
        &mut self.conn
    }
}

/// Log-and-swallow helper used by infallible public API surfaces (e.g.
/// [`KillSwitch::is_enabled`]) that must not panic on Redis errors.
pub(crate) fn log_redis_error(context: &str, err: RedisError) {
    error!("{context}: {err}");
}
