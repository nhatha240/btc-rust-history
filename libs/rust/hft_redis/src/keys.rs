//! Canonical Redis key constructors — single source of truth for key names.
//!
//! Every key used across services is defined here as a constant or constructor
//! function. Centralising key names prevents typos and makes it trivial to
//! audit all Redis usage.
//!
//! # Naming conventions (from CLAUDE.md)
//! ```text
//! risk:kill                          — global kill switch
//! risk:kill:{account_id}             — per-account kill switch
//! signal:state:{symbol}              — latest signal (compacted topic mirror)
//! position:{account_id}:{symbol}     — hot position cache (Hash)
//! instruments:binance:{market}       — instrument list (spot | um | cm)
//! rl:{scope}:{id}                    — rate-limit counter
//! ```

// ── Kill switch ───────────────────────────────────────────────────────────────

/// Global kill switch.  Set to `"1"` to halt **all** order submission.
pub const KILL_SWITCH: &str = "risk:kill";

/// Per-account kill switch.  Checked in addition to the global switch.
pub fn kill_switch_account(account_id: &str) -> String {
    format!("risk:kill:{account_id}")
}

// ── Signal state ──────────────────────────────────────────────────────────────

/// Latest signal state per symbol (mirrors `TOPIC_SIGNAL_STATE` compacted topic).
///
/// Value: JSON or proto-encoded signal struct; refreshed on every new signal.
pub fn signal_state(symbol: &str) -> String {
    format!("signal:state:{symbol}")
}

// ── Position cache ────────────────────────────────────────────────────────────

/// Hot position cache for a given account + symbol (used by risk checks).
///
/// Value: JSON-encoded position snapshot; refreshed on every fill.
pub fn position(account_id: &str, symbol: &str) -> String {
    format!("position:{account_id}:{symbol}")
}

// ── Instruments ───────────────────────────────────────────────────────────────

/// Instrument list from Binance.
///
/// `market`: `"spot"` | `"um"` (USDT-margined futures) | `"cm"` (coin-margined futures)
pub fn instruments(exchange: &str, market: &str) -> String {
    format!("instruments:{exchange}:{market}")
}

// ── Rate limiting ─────────────────────────────────────────────────────────────

/// Generic rate-limit counter.
///
/// `scope` — logical group (e.g. `"signal"`, `"order"`, `"api"`).
/// `id`    — entity within the scope (e.g. `"BTCUSDT:LONG"`, `"acct-42"`).
pub fn rate_limit(scope: &str, id: &str) -> String {
    format!("rl:{scope}:{id}")
}

/// Signal fire rate for a specific symbol + side.
///
/// Tracks how many signals fired for `BTCUSDT:LONG` in the current window.
/// Compared against `SIGNAL_MAX_PER_MIN` from the anti-spam config.
pub fn signal_rate_limit(symbol: &str, side: &str) -> String {
    rate_limit("signal", &format!("{symbol}:{side}"))
}

/// Order submission rate per account.
pub fn order_rate_limit(account_id: &str) -> String {
    rate_limit("order", account_id)
}

/// External API call rate per endpoint.
pub fn api_rate_limit(endpoint: &str) -> String {
    rate_limit("api", endpoint)
}

// ── Market Data Health ──────────────────────────────────────────────────────

/// Real-time health metrics for a market data feed (venue:symbol).
///
/// Value: Hash containing `last_msg_ts`, `msg_rate`, `latency_ms`, `reconnects`.
pub fn md_health(venue: &str, symbol: &str) -> String {
    format!("md:health:{venue}:{symbol}")
}
