//! Time utilities for HFT services.
//!
//! All timestamps in the system are Unix epoch-based.
//! Use nanoseconds for intra-service precision; milliseconds for Kafka/proto.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

// ── Wall-clock helpers ────────────────────────────────────────────────────────

/// Current Unix timestamp in **nanoseconds** (i64, matches proto `int64`).
///
/// Panics if the system clock is set before the Unix epoch.
#[inline]
pub fn now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_nanos() as i64
}

/// Current Unix timestamp in **milliseconds** (i64, matches proto `int64`).
#[inline]
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_millis() as i64
}

/// Current Unix timestamp in **seconds** (f64, useful for rates/metrics).
#[inline]
pub fn now_secs_f64() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_secs_f64()
}

// ── Monotonic clock ───────────────────────────────────────────────────────────

/// Return a monotonic [`Instant`] for latency measurement.
///
/// ```rust
/// use hft_common::time::monotonic;
/// let t = monotonic();
/// // ... do work ...
/// let elapsed_us = t.elapsed().as_micros();
/// ```
#[inline]
pub fn monotonic() -> Instant {
    Instant::now()
}

// ── Unit conversions ──────────────────────────────────────────────────────────

/// Convert nanoseconds → milliseconds (integer division, truncates).
#[inline]
pub fn ns_to_ms(ns: i64) -> i64 {
    ns / 1_000_000
}

/// Convert milliseconds → nanoseconds.
#[inline]
pub fn ms_to_ns(ms: i64) -> i64 {
    ms * 1_000_000
}

/// Convert microseconds → nanoseconds.
#[inline]
pub fn us_to_ns(us: i64) -> i64 {
    us * 1_000
}

/// Elapsed nanoseconds between two wall-clock ns timestamps.
///
/// Returns 0 if `end < start` (clock skew protection).
#[inline]
pub fn elapsed_ns(start_ns: i64, end_ns: i64) -> i64 {
    (end_ns - start_ns).max(0)
}
