//! `hft_common` — cross-cutting utilities shared by every HFT service.
//!
//! # Modules
//! - [`config`]  — env-var helpers with validation
//! - [`error`]   — [`AppError`](error::AppError) enum + [`Result`](error::Result) alias
//! - [`ids`]     — trace ID and client-order-ID generation / propagation
//! - [`logging`] — tracing-subscriber setup (JSON + optional OTEL layer)
//! - [`otel`]    — OpenTelemetry OTLP exporter setup + shutdown guard
//! - [`time`]    — `now_ns`, `now_ms`, monotonic helpers, unit conversions

pub mod config;
pub mod error;
pub mod ids;
pub mod logging;
pub mod otel;
pub mod time;

// Re-export the most-used items at crate root for convenience.
pub use error::{AppError, Result};
pub use ids::{new_client_order_id, new_trace_id};
pub use time::{now_ms, now_ns};
