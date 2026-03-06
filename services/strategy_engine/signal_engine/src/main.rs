use anyhow::{Context, Result};
use hft_proto::md::FeatureState;
use hft_proto::encode::from_bytes;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;
use std::time::Duration;
use tracing::{error, info};
use clickhouse::Client;

#[tokio::main]
async fn main() -> Result<()> {
    // ── Pre-init ─────────────────────────────────────────────────────────────
    // Robust rustls init to prevent panics
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Signal Engine starting...");

    // ── Config ────────────────────────────────────────────────────────────────
    let kafka_brokers = std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "redpanda:9092".to_string());
    let topic_in = std::env::var("KAFKA_TOPIC_FEATURE_STATE").unwrap_or_else(|_| "TOPIC_FEATURE_STATE".to_string());
    let topic_out = std::env::var("KAFKA_TOPIC_SIGNALS").unwrap_or_else(|_| "TOPIC_SIGNALS".to_string());
    let ch_url = std::env::var("CLICKHOUSE_HTTP_URL").unwrap_or_else(|_| "http://clickhouse:8123".to_string());
    let ch_db = std::env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "db_trading".to_string());

    // ── ClickHouse Client ─────────────────────────────────────────────────────
    let ch_client = Client::default()
        .with_url(&ch_url)
        .with_database(&ch_db);

    // ── Kafka Producer ────────────────────────────────────────────────────────
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &kafka_brokers)
        .set("message.timeout.ms", "5000")
        .create()
        .context("Failed to create Kafka producer")?;

    // ── Kafka Consumer ────────────────────────────────────────────────────────
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &kafka_brokers)
        .set("group.id", "signal-engine-group")
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "latest")
        .create()
        .context("Failed to create Kafka consumer")?;

    consumer.subscribe(&[&topic_in]).context("Failed to subscribe to topic")?;

    info!("Consuming from {}, producing to {}", topic_in, topic_out);

    // ── Main Loop ─────────────────────────────────────────────────────────────
    loop {
        match consumer.recv().await {
            Err(e) => error!("Kafka error: {}", e),
            Ok(bm) => {
                let payload = match bm.payload_view::<[u8]>() {
                    Some(Ok(p)) => p,
                    _ => continue,
                };

                let feature: FeatureState = match from_bytes(payload) {
                    Ok(f) => f,
                    Err(e) => {
                        error!("Failed to decode FeatureState: {}", e);
                        continue;
                    }
                };

                // ── Strategy Logic (EMA Crossover) ───────────────────────────
                let side = if feature.ema_fast > feature.ema_slow {
                    1 // LONG
                } else {
                    -1 // SHORT
                };

                // In a real system, we'd check for "crossover" events (previous state).
                // For this implementation, we emit the state-based signal.
                
                info!(
                    symbol = %feature.symbol,
                    ema_fast = feature.ema_fast,
                    ema_slow = feature.ema_slow,
                    side = side,
                    "Signal generated"
                );

                // ── Persist to ClickHouse ────────────────────────────────────
                // db_trading.signals (ts, symbol, side, reason, price, confidence, model_version)
                // We use a simplified version for this placeholder logic.
                let query = format!(
                    "INSERT INTO signals (ts, symbol, side, reason, price, confidence, model_version) VALUES ({}, '{}', {}, '{}', {}, {}, '{}')",
                    feature.ts,
                    feature.symbol,
                    side,
                    "EMA_CROSSOVER",
                    0.0, // We don't have current price in FeatureState proto yet
                    1.0,
                    "v0.1-rule-based"
                );

                if let Err(e) = ch_client.query(&query).execute().await {
                   error!("Failed to persist signal to ClickHouse: {}", e);
                }

                // ── Publish to Kafka ──────────────────────────────────────────
                let signal_msg = serde_json::json!({
                    "symbol": feature.symbol,
                    "ts": feature.ts,
                    "side": side,
                    "reason": "EMA_CROSSOVER",
                    "model_version": "v0.1-rule-based"
                });

                let signal_json = serde_json::to_string(&signal_msg)?;
                
                let _ = producer.send(
                    FutureRecord::to(&topic_out)
                        .payload(&signal_json)
                        .key(&feature.symbol),
                    Duration::from_secs(0)
                ).await;
            }
        }
    }
}
