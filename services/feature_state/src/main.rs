use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use axum::http::StatusCode;
use axum::{Router, routing::get};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let health_port = std::env::var("HEALTH_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(8080);

    let ready = Arc::new(AtomicBool::new(false));
    let ready_for_http = Arc::clone(&ready);
    tokio::spawn(async move {
        if let Err(e) = serve_health(health_port, ready_for_http).await {
            tracing::error!("health server failed: {e:#}");
        }
    });

    tracing::info!(health_port, "feature_state placeholder starting");
    ready.store(true, Ordering::Relaxed);

    // Keep process alive until full feature_state pipeline is wired in.
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

async fn serve_health(port: u16, ready: Arc<AtomicBool>) -> Result<()> {
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
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
