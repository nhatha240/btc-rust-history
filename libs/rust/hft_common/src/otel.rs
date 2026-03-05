//! OpenTelemetry OTLP exporter setup.
//!
//! Call [`init_otel`] **before** initialising the tracing subscriber so the
//! OTEL layer can be wired in.  Hold the returned [`OtelGuard`] for the full
//! lifetime of the process — it flushes and shuts down the tracer pipeline
//! on drop.
//!
//! # Example
//! ```rust,ignore
//! let _otel = hft_common::otel::init_otel("risk_guard", "http://otelcol:4317")?;
//! ```

use anyhow::{Context, Result};
use opentelemetry::global;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    trace::{Config as TraceConfig, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;

/// RAII guard that shuts down the global OTLP tracer provider on drop.
///
/// Keep alive for the full process lifetime:
/// ```rust,ignore
/// let _guard = hft_common::otel::init_otel(...)?;
/// // ... run service ...
/// // guard dropped here → pipeline flushed
/// ```
pub struct OtelGuard(TracerProvider);

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Err(e) = self.0.shutdown() {
            eprintln!("[otel] shutdown error: {e}");
        }
    }
}

/// Initialise the OTLP span exporter and register it as the global tracer
/// provider.
///
/// # Arguments
/// - `service_name` — sets `service.name` in every span's resource attributes
/// - `endpoint`     — gRPC OTLP endpoint, e.g. `"http://otelcol:4317"`
///
/// # Errors
/// Returns an error if the exporter cannot connect or the provider fails to
/// build.
pub fn init_otel(service_name: &str, endpoint: &str) -> Result<OtelGuard> {
    let resource = Resource::new(vec![KeyValue::new(SERVICE_NAME, service_name.to_owned())]);

    let trace_config = TraceConfig::default().with_resource(resource);

    let provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint),
        )
        .with_trace_config(trace_config)
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .context("failed to install OTLP trace pipeline")?;

    // Register as the global provider so `tracing_opentelemetry::layer()` picks it up.
    global::set_tracer_provider(provider.clone());

    Ok(OtelGuard(provider))
}
