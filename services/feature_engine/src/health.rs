//! HTTP health and readiness endpoints.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use axum::http::StatusCode;
use axum::{routing::get, Router};
use tracing::info;

pub async fn serve(port: u16, ready: Arc<AtomicBool>) -> Result<()> {
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route(
            "/ready",
            get(move || {
                let ready = Arc::clone(&ready);
                async move {
                    if ready.load(Ordering::Relaxed) {
                        (StatusCode::OK, "ready")
                    } else {
                        (StatusCode::SERVICE_UNAVAILABLE, "not_ready")
                    }
                }
            }),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!(%addr, "health check listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
