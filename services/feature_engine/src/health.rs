//! HTTP health check endpoint — `GET /health` returns 200 OK.

use std::net::SocketAddr;

use anyhow::Result;
use axum::{routing::get, Router};
use tracing::info;

pub async fn serve(port: u16) -> Result<()> {
    let app = Router::new().route("/health", get(|| async { "ok" }));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!(%addr, "health check listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
