//! Structured logging initialisation for HFT services.
//!
//! Sets up a `tracing-subscriber` registry with:
//! - JSON log output (for log aggregators like Loki / CloudWatch)
//! - `RUST_LOG` env-filter with `info` default
//! - Optional OpenTelemetry layer when `OTEL_EXPORTER_OTLP_ENDPOINT` is set
//!
//! # Usage
//! Call once at the top of `main`, before any `tracing::info!` calls:
//!
//! ```rust,ignore
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let _guard = hft_common::logging::init("risk_guard")?;
//!     tracing::info!("service started");
//!     // ...
//! }
//! ```
//!
//! The returned [`TracingGuard`] must be kept alive for the process lifetime.
//! Dropping it flushes the OTEL pipeline (if configured).

use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::otel::{init_otel, OtelGuard};

/// Holds the OTEL guard so the pipeline is flushed on process exit.
///
/// Store this in a `let _guard = ...` binding at the top of `main`.
pub struct TracingGuard {
    _otel: Option<OtelGuard>,
}

/// Initialise structured logging (+ optional OTEL tracing) for a service.
///
/// Reads two env vars:
/// - `RUST_LOG`                      — log filter (default: `"info"`)
/// - `OTEL_EXPORTER_OTLP_ENDPOINT`  — if set, activates the OTEL span layer
///
/// # Arguments
/// - `service_name` — propagated to the OTEL `service.name` resource attribute
pub fn init(service_name: &str) -> Result<TracingGuard> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(false) // keep payloads compact
        .with_target(true);

    let otel_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();

    if let Some(endpoint) = otel_endpoint {
        let otel_guard = init_otel(service_name, &endpoint)?;
        let otel_layer = tracing_opentelemetry::layer();

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .with(otel_layer)
            .init();

        Ok(TracingGuard {
            _otel: Some(otel_guard),
        })
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .init();

        Ok(TracingGuard { _otel: None })
    }
}
