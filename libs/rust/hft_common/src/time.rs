
use std::time::{Instant, SystemTime, UNIX_EPOCH};
#[inline]
pub fn now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_nanos() as i64
}

#[inline]
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_millis() as i64
}

#[inline]
pub fn now_secs_f64() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_secs_f64()
}
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

#[inline]
pub fn us_to_ns(us: i64) -> i64 {
    us * 1_000
}
#[inline]
pub fn elapsed_ns(start_ns: i64, end_ns: i64) -> i64 {
    (end_ns - start_ns).max(0)
}
