use anyhow::{Context, Result};
use futures::{Stream, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use hft_proto::oms::ExecutionReport;
use tracing::{error, info};
use std::pin::Pin;

pub struct BinanceWsStream {
    url: String,
}

impl BinanceWsStream {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    pub async fn stream_execution_reports(&self) -> Result<Pin<Box<dyn Stream<Item = ExecutionReport> + Send>>> {
        let (ws_stream, _) = connect_async(&self.url)
            .await
            .context("Failed to connect to Binance WebSocket")?;
        
        info!("Connected to Binance WebSocket: {}", self.url);

        let stream = ws_stream.filter_map(|msg| async move {
            match msg {
                Ok(Message::Text(text)) => {
                    // In a real implementation, parse Binance EXECUTION_REPORT JSON
                    // and map to ExecutionReport protobuf.
                    // For now, this is a placeholder.
                    info!("Received WS message: {}", text);
                    None
                }
                Ok(_) => None,
                Err(e) => {
                    error!("WS error: {}", e);
                    None
                }
            }
        });

        Ok(Box::pin(stream))
    }
}
