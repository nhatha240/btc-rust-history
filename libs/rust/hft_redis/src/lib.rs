//! `hft_redis` — typed Redis client for HFT services.
//!
//! # What this crate standardises
//! - Single connection via [`redis::aio::ConnectionManager`] (auto-reconnect)
//! - Typed key constructors that enforce the project's naming conventions
//! - Boolean helpers (`get_bool` / `set_bool_ttl`) for kill-switch style flags
//! - JSON cache helpers for structured state values
//! - Atomic `INCR + EXPIRE` via Lua script (no TOCTOU race)
//! - [`KillSwitch`] — checks `risk:kill` with per-account override
//! - [`RateLimiter`] — sliding-window counter with configurable TTL
//!
//! # Quick start
//! ```rust,ignore
//! use hft_redis::{RedisStore, KillSwitch, RateLimiter, keys};
//!
//! let store  = RedisStore::new("redis://redis:6379/0").await?;
//!
//! // Kill-switch
//! let mut ks = KillSwitch::new(store.clone());
//! if ks.is_enabled(Some("acct-42")).await { return; }
//!
//! // Rate limiting
//! let mut rl = RateLimiter::new(store.clone());
//! let count  = rl.incr_with_ttl(&keys::signal_rate_limit("BTCUSDT", "LONG"), 60).await?;
//! if count > 5 { return; /* exceeded per-minute limit */ }
//! ```

pub mod client;
pub mod keys;
pub mod rate_limit;

// ── Re-exports ────────────────────────────────────────────────────────────────
pub use client::{RedisError, RedisStore, Result};
pub use rate_limit::{KillSwitch, RateLimiter};
