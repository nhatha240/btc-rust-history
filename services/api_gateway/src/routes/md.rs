use axum::{
    extract::{Path, State, ws as ax_ws},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use hft_mq::{KafkaConfig, KafkaConsumer};
use hft_redis::{RedisStore, keys};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

#[derive(Clone)]
pub struct MdState {
    pub redis: RedisStore,
    pub broadcaster: Arc<MdBroadcaster>,
}

pub struct MdBroadcaster {
    channels: Mutex<HashMap<String, broadcast::Sender<serde_json::Value>>>,
    kafka_config: KafkaConfig,
}

impl MdBroadcaster {
    pub fn new(kafka_config: KafkaConfig) -> Self {
        Self {
            channels: Mutex::new(HashMap::new()),
            kafka_config,
        }
    }

    pub async fn subscribe(&self, symbol: &str) -> broadcast::Receiver<serde_json::Value> {
        let mut channels = self.channels.lock().await;
        if let Some(tx) = channels.get(symbol) {
            tx.subscribe()
        } else {
            let (tx, rx) = broadcast::channel(1024);
            channels.insert(symbol.to_string(), tx);
            rx
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let consumer = KafkaConsumer::new(
            self.kafka_config.clone(),
            &["md.raw.trades", "md.raw.book"],
        )?;

        // In a real system we'd decode proto and broadcast.
        // This is a simplified placeholder.
        consumer.run(move |_ctx| {
            async move {
                Ok(())
            }
        }).await
    }
}

pub fn router(state: MdState) -> Router {
    Router::new()
        .route("/health", get(handle_md_health))
        .route("/live/{symbol}", get(handle_ws_md))
        .with_state(state)
}

async fn handle_ws_md(
    ws: ax_ws::WebSocketUpgrade,
    Path(symbol): Path<String>,
    State(state): State<MdState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_loop(socket, symbol, state))
}

async fn handle_ws_loop(mut socket: ax_ws::WebSocket, symbol: String, state: MdState) {
    let mut rx = state.broadcaster.subscribe(&symbol).await;
    
    while let Ok(msg) = rx.recv().await {
        let text = serde_json::to_string(&msg).unwrap();
        if socket.send(ax_ws::Message::Text(text.into())).await.is_err() {
            break;
        }
    }
}

#[derive(Serialize)]
pub struct SymbolHealth {
    pub symbol: String,
    pub last_msg_ts: u64,
    pub msg_rate: u64,
    pub latency_ms: f64,
}

#[derive(Serialize)]
pub struct VenueHealth {
    pub venue: String,
    pub reconnects: u64,
    pub symbols: Vec<SymbolHealth>,
}

async fn handle_md_health(State(mut state): State<MdState>) -> Json<Vec<VenueHealth>> {
    let venue = "binance";
    let reconnect_key = "md:health:binance:reconnects";
    let reconnects: u64 = state.redis.get(reconnect_key).await.unwrap_or(0);

    let symbols = vec!["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT"];
    let mut symbol_healths = Vec::new();

    for s in symbols {
        let key = keys::md_health(venue, s);
        let last_ts: u64 = state.redis.hget(&key, "last_msg_ts").await.unwrap_or(0);
        let rate: u64 = state.redis.hget(&key, "msg_rate").await.unwrap_or(0);
        let lat: f64 = state.redis.hget(&key, "latency_ms").await.unwrap_or(0.0);

        symbol_healths.push(SymbolHealth {
            symbol: s.to_string(),
            last_msg_ts: last_ts,
            msg_rate: rate,
            latency_ms: lat,
        });
    }

    Json(vec![VenueHealth {
        venue: venue.to_string(),
        reconnects,
        symbols: symbol_healths,
    }])
}
