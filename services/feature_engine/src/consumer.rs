//! Kafka consumer: reads closed candles from `TOPIC_CANDLES_1M`, updates
//! the per-symbol indicator state, and publishes feature vectors.
//!
//! ## Candle wire format
//! Candles are expected as JSON objects on `TOPIC_CANDLES_1M`.
//!
//! ```json
//! {
//!   "symbol":     "BTCUSDT",
//!   "open_time":  1700000000000,
//!   "close_time": 1700000059999,
//!   "open":  30000.0,
//!   "high":  30100.0,
//!   "low":   29900.0,
//!   "close": 30050.0,
//!   "volume": 12.345,
//!   "is_closed": true
//! }
//! ```
//!
//! Only candles with `is_closed = true` trigger indicator updates and publishes.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use dashmap::DashMap;
use hft_mq::{DlqConfig, KafkaConfig, KafkaConsumer, MessageCtx, RetryPolicy};
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::config::Config;
use crate::producer::FeatureProducer;
use crate::state::registry::Registry;

#[derive(Debug, Deserialize)]
struct Candle {
    symbol: String,
    open_time: i64,
    close_time: i64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    #[serde(default)]
    is_closed: bool,
}

/// Run the consumer loop — blocks until a fatal error occurs.
pub async fn run(cfg: Arc<Config>, registry: Arc<Registry>, producer: Arc<FeatureProducer>) -> Result<()> {
    let kafka_cfg = KafkaConfig {
        brokers:              cfg.kafka_brokers.clone(),
        group_id:             cfg.kafka_group_id.clone(),
        client_id:            cfg.kafka_client_id.clone(),
        // Low latency: stale features are worse than dropped ones.
        linger_ms:            0,
        batch_size:           16_384,
        compression:          "none".to_owned(),
        acks:                 "1".to_owned(),
        idempotent:           false,
        message_timeout_ms:   3_000,
        session_timeout_ms:   10_000,
        max_poll_interval_ms: 60_000,
        fetch_max_bytes:      10_485_760,
        auto_offset_reset:    "latest".to_owned(),
        retry: RetryPolicy {
            max_retries:        1,
            initial_backoff_ms: 50,
            max_backoff_ms:     500,
        },
        dlq: DlqConfig::default(),
    };

    let topic = cfg.topic_candles.as_str();
    let consumer = KafkaConsumer::new(kafka_cfg, &[topic])?;

    // Per-symbol throttle: tracks the last publish Instant.
    let throttle_map: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
    let throttle_ms = cfg.throttle_ms;

    consumer
        .run(move |ctx: MessageCtx| {
            let registry = Arc::clone(&registry);
            let producer = Arc::clone(&producer);
            let throttle_map = Arc::clone(&throttle_map);

            async move {
                handle_message(ctx, &registry, &producer, &throttle_map, throttle_ms).await
            }
        })
        .await
}

async fn handle_message(
    ctx: MessageCtx,
    registry: &Registry,
    producer: &FeatureProducer,
    throttle_map: &DashMap<String, Instant>,
    throttle_ms: u64,
) -> Result<()> {
    let candle: Candle = match serde_json::from_slice(&ctx.payload) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                topic = %ctx.topic,
                offset = ctx.offset,
                "failed to deserialise candle: {e}"
            );
            return Ok(()); // skip malformed messages
        }
    };

    if !candle.is_closed {
        debug!(symbol = %candle.symbol, "skipping non-closed candle");
        return Ok(());
    }

    // Throttle check: skip if published too recently for this symbol.
    if throttle_ms > 0 {
        let now = Instant::now();
        if let Some(last) = throttle_map.get(&candle.symbol) {
            if now.duration_since(*last) < Duration::from_millis(throttle_ms) {
                debug!(symbol = %candle.symbol, "throttled");
                return Ok(());
            }
        }
    }

    // Update indicators — all O(1).
    let fv = {
        let mut state = registry.get_or_create(&candle.symbol);
        state.update(
            candle.open_time,
            candle.close_time,
            candle.high,
            candle.low,
            candle.close,
            candle.volume,
        )
    };

    // Publish feature vector.
    if let Err(e) = producer.publish(&fv).await {
        error!(symbol = %fv.symbol, "publish error: {e}");
        return Err(e);
    }

    // Record publish time for throttle.
    if throttle_ms > 0 {
        throttle_map.insert(fv.symbol.clone(), Instant::now());
    }

    debug!(
        symbol = %fv.symbol,
        ts = fv.ts,
        quality = fv.quality,
        ema_fast = fv.ema_fast,
        ema_slow = fv.ema_slow,
        rsi = fv.rsi,
        macd = fv.macd,
        vwap = fv.vwap,
        "feature published"
    );

    Ok(())
}
