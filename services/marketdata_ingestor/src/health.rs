use axum::{routing::get, Router};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::candle::CandleAppState;



pub async fn serve(port: u16, candle_state: Arc<CandleAppState>) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route(
            "/ready",
            get(move || {
                let state = candle_state.clone();
                async move {
                    if state.is_ready() {
                        "ready"
                    } else {
                        "not_ready"
                    }
                }
            }),
        );
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(port, "health server started");
    axum::serve(listener, app).await?;
    Ok(())
}
