//! `/healthz` and `/readyz` endpoints for risk_guard.
//!
//! Runs in a separate Tokio task so the main Kafka loop can never block probes.

use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::net::SocketAddr;
use tracing::{error, info};

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

async fn handle_health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok", service: "risk_guard" })
}

/// Spawn the health-check HTTP server on `addr` in a background task.
/// Errors during startup are logged; a failure does NOT crash the main loop.
pub fn spawn(addr: SocketAddr) {
    tokio::spawn(async move {
        let app = Router::new()
            .route("/healthz", get(handle_health))
            .route("/readyz", get(handle_health));

        info!(%addr, "risk_guard health server listening");
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                if let Err(e) = axum::serve(listener, app).await {
                    error!("Health server error: {e}");
                }
            }
            Err(e) => error!("Failed to bind health server on {addr}: {e}"),
        }
    });
}
