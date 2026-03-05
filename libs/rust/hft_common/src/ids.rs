//! Unique ID generation and trace propagation helpers.
//!
//! All IDs are UUID v4 strings — easy to log, correlate, and store in Postgres.

use uuid::Uuid;

// ── Generators ────────────────────────────────────────────────────────────────

/// Generate a new **trace ID** (UUID v4 string, lowercase hyphenated).
///
/// One trace ID should be created at the signal/decision boundary and
/// forwarded on every downstream message: order → risk → fill.
#[inline]
pub fn new_trace_id() -> String {
    Uuid::new_v4().to_string()
}

/// Generate a new **client order ID** (UUID v4 string).
///
/// Must be globally unique across all accounts and symbols.
/// Matches the `client_order_id` field in `OrderCommand` proto.
#[inline]
pub fn new_client_order_id() -> String {
    Uuid::new_v4().to_string()
}

// ── Propagation helpers ───────────────────────────────────────────────────────

/// Return `existing` if it is a valid UUID, otherwise generate a fresh trace ID.
///
/// Useful when consuming Kafka messages where `trace_id` may be empty:
/// ```rust,ignore
/// let trace = hft_common::ids::trace_id_or_new(Some(&msg.trace_id));
/// ```
pub fn trace_id_or_new(existing: Option<&str>) -> String {
    existing
        .filter(|s| !s.is_empty())
        .filter(|s| Uuid::parse_str(s).is_ok())
        .map(str::to_owned)
        .unwrap_or_else(new_trace_id)
}

/// Parse a string as a UUID; returns `None` on invalid input.
#[inline]
pub fn parse_uuid(s: &str) -> Option<Uuid> {
    Uuid::parse_str(s).ok()
}
