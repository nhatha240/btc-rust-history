//! Rate limiter and kill-switch built on [`RedisStore`].
//!
//! # [`RateLimiter`]
//! Uses an atomic Lua script (`INCR` + conditional `EXPIRE`) to avoid the
//! TOCTOU race that exists with a naïve `INCR` followed by a separate
//! `EXPIRE` call.
//!
//! ```text
//! Window starts → INCR(key) → 1  → EXPIRE(key, ttl)
//! Within window  → INCR(key) → 2  → (no EXPIRE, keeps original deadline)
//! Window expires → key deleted automatically by Redis
//! Next call      → INCR(key) → 1  → EXPIRE(key, ttl)  ← new window
//! ```
//!
//! # [`KillSwitch`]
//! Two-level check: global `risk:kill` first, then per-account
//! `risk:kill:{account_id}` if an account ID is provided.
//!
//! **Fail-open**: if Redis is unreachable, `is_enabled` returns `false`
//! (trading continues) and logs an error. This matches the existing
//! service behaviour (`unwrap_or(None)`).

use redis::{AsyncCommands, Script};
use tracing::warn;

use crate::client::{log_redis_error, RedisError, RedisStore, Result};
use crate::keys;

// ── Lua script ────────────────────────────────────────────────────────────────

/// Atomically increment a counter and set its TTL on the first increment.
///
/// KEYS[1] — counter key
/// ARGV[1] — TTL in seconds (applied only when count reaches 1)
const INCR_WITH_TTL_LUA: &str = r#"
local count = redis.call('INCR', KEYS[1])
if count == 1 then
    redis.call('EXPIRE', KEYS[1], ARGV[1])
end
return count
"#;

// ── RateLimiter ───────────────────────────────────────────────────────────────

/// Sliding-window rate limiter backed by Redis.
///
/// Each counter key represents one window. The window duration is set by
/// `ttl_secs` on the first `incr_with_ttl` call for that key.
pub struct RateLimiter {
    store: RedisStore,
}

impl RateLimiter {
    pub fn new(store: RedisStore) -> Self {
        Self { store }
    }

    /// Atomically increment `key` and return the new count.
    ///
    /// The TTL is set **only on the first call** (when count becomes 1),
    /// so the window expires `ttl_secs` after the first event — not after
    /// the most recent one. This is a fixed-window counter.
    ///
    /// # Returns
    /// Current count in the window after this increment.
    pub async fn incr_with_ttl(&mut self, key: &str, ttl_secs: u64) -> Result<u64> {
        // Creating Script inline avoids borrow-checker conflicts when Script
        // would need to be stored alongside &mut self.store.
        // Script::new is cheap (just SHA1 of a fixed string).
        let script = Script::new(INCR_WITH_TTL_LUA);

        let count: i64 = script
            .key(key)
            .arg(ttl_secs)
            .invoke_async(self.store.conn_mut())
            .await
            .map_err(RedisError::Command)?;

        Ok(count.max(0) as u64)
    }

    /// Read the current counter value without incrementing.
    pub async fn count(&mut self, key: &str) -> Result<u64> {
        let val: Option<i64> = self.store.conn_mut().get(key).await.map_err(RedisError::Command)?;
        Ok(val.unwrap_or(0).max(0) as u64)
    }

    /// Return `true` if the current count is already at or above `limit`
    /// (without incrementing).
    ///
    /// Use before a send to avoid incrementing the counter on rejected work:
    /// ```rust,ignore
    /// if rl.is_exceeded(&key, MAX_PER_MIN).await? { bail!("rate limited"); }
    /// let new_count = rl.incr_with_ttl(&key, 60).await?;
    /// ```
    pub async fn is_exceeded(&mut self, key: &str, limit: u64) -> Result<bool> {
        Ok(self.count(key).await? >= limit)
    }

    /// Increment and check in a single round-trip.
    ///
    /// Returns `(new_count, exceeded)`. Use when you want to record the event
    /// *and* check the limit atomically.
    pub async fn incr_check(
        &mut self,
        key: &str,
        ttl_secs: u64,
        limit: u64,
    ) -> Result<(u64, bool)> {
        let count = self.incr_with_ttl(key, ttl_secs).await?;
        Ok((count, count > limit))
    }

    /// Reset the counter by deleting the key.
    pub async fn reset(&mut self, key: &str) -> Result<()> {
        self.store.del(key).await
    }
}

// ── KillSwitch ────────────────────────────────────────────────────────────────

/// Two-level kill switch: global (`risk:kill`) + per-account override.
///
/// # Fail-open behaviour
/// If Redis is unavailable, `is_enabled` logs the error and returns `false`
/// (trading continues). This matches the existing `risk_guard` behaviour.
/// If you need fail-safe (halt on Redis loss), wrap `is_enabled` and check
/// the `Result` directly via [`RedisStore::get_bool`] instead.
pub struct KillSwitch {
    store: RedisStore,
}

impl KillSwitch {
    pub fn new(store: RedisStore) -> Self {
        Self { store }
    }

    /// Check whether the kill switch is active.
    ///
    /// Evaluation order:
    /// 1. Global key `risk:kill` — if set to `"1"`, returns `true` immediately.
    /// 2. Per-account key `risk:kill:{account_id}` — checked only when
    ///    `account_id` is `Some(...)`.
    ///
    /// Returns `false` on any Redis error (fail-open) and logs the error.
    pub async fn is_enabled(&mut self, account_id: Option<&str>) -> bool {
        // 1. Global kill switch
        match self.store.get_bool(keys::KILL_SWITCH).await {
            Ok(true) => {
                warn!(key = keys::KILL_SWITCH, "global kill switch is ACTIVE");
                return true;
            }
            Ok(false) => {}
            Err(e) => {
                log_redis_error("KillSwitch::is_enabled (global check)", e);
                return false; // fail-open
            }
        }

        // 2. Per-account kill switch (only if account_id provided)
        if let Some(id) = account_id {
            let key = keys::kill_switch_account(id);
            match self.store.get_bool(&key).await {
                Ok(true) => {
                    warn!(account_id = id, key = %key, "per-account kill switch is ACTIVE");
                    return true;
                }
                Ok(false) => {}
                Err(e) => {
                    log_redis_error("KillSwitch::is_enabled (account check)", e);
                    return false; // fail-open
                }
            }
        }

        false
    }

    /// Check only the global kill switch key.  Returns `Ok(bool)` or a Redis
    /// error — callers decide whether to fail-open or fail-safe.
    pub async fn check_global(&mut self) -> Result<bool> {
        self.store.get_bool(keys::KILL_SWITCH).await
    }

    /// Check only the per-account kill switch key.
    pub async fn check_account(&mut self, account_id: &str) -> Result<bool> {
        self.store
            .get_bool(&keys::kill_switch_account(account_id))
            .await
    }

    /// Activate the global kill switch (`SET risk:kill 1`).
    pub async fn enable(&mut self) -> Result<()> {
        self.store.set_bool(keys::KILL_SWITCH, true).await?;
        warn!(key = keys::KILL_SWITCH, "global kill switch ENABLED");
        Ok(())
    }

    /// Activate the kill switch for a specific account.
    pub async fn enable_account(&mut self, account_id: &str) -> Result<()> {
        let key = keys::kill_switch_account(account_id);
        self.store.set_bool(&key, true).await?;
        warn!(account_id, key = %key, "per-account kill switch ENABLED");
        Ok(())
    }

    /// Deactivate the global kill switch by deleting the key.
    ///
    /// Deleting (rather than writing `"0"`) is the canonical way to deactivate
    /// so that `GET` returns `None` which is unambiguously inactive.
    pub async fn disable(&mut self) -> Result<()> {
        self.store.del(keys::KILL_SWITCH).await?;
        warn!(key = keys::KILL_SWITCH, "global kill switch DISABLED");
        Ok(())
    }

    /// Deactivate the kill switch for a specific account.
    pub async fn disable_account(&mut self, account_id: &str) -> Result<()> {
        let key = keys::kill_switch_account(account_id);
        self.store.del(&key).await?;
        warn!(account_id, key = %key, "per-account kill switch DISABLED");
        Ok(())
    }
}
