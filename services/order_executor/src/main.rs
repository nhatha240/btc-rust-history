use anyhow::Result;
use rdkafka::{consumer::{Consumer, StreamConsumer}, producer::{FutureProducer, FutureRecord}, ClientConfig, Message};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignalEvent {
    symbol: String,
    direction: String, // "LONG" | "SHORT" | "FLAT"
    ts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderEvent {
    order_id: String,
    symbol: String,
    side: String, // "BUY" | "SELL"
    qty: f64,
    r#type: String, // "MARKET"
    ts: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() { std::env::set_var("RUST_LOG","info"); }
    env_logger::init();

    let brokers = std::env::var("KAFKA_BROKERS").unwrap_or("redpanda:9092".into());
    let topic_in = std::env::var("KAFKA_TOPIC_SIGNALS").unwrap_or("TOPIC_SIGNALS".into());
    let topic_out = std::env::var("KAFKA_TOPIC_ORDERS").unwrap_or("TOPIC_ORDERS".into());
    let group = std::env::var("KAFKA_GROUP_ID").unwrap_or("order_executor".into());
    let default_qty: f64 = std::env::var("DEFAULT_QTY").ok().and_then(|v| v.parse().ok()).unwrap_or(0.001);

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &brokers)
        .set("group.id", group)
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "latest")
        .create()?;
    consumer.subscribe(&[&topic_in])?;

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &brokers)
        .set("message.timeout.ms", "5000")
        .create()?;

    while let Ok(m) = consumer.recv().await {
        if let Some(Ok(payload)) = m.payload_view::<str>() {
            if let Ok(sig) = serde_json::from_str::<SignalEvent>(payload) {
                let side = match sig.direction.as_str() {
                    "LONG" => "BUY",
                    "SHORT" => "SELL",
                    _ => continue,
                }.to_string();

                let order = OrderEvent {
                    order_id: uuid::Uuid::new_v4().to_string(),
                    symbol: sig.symbol,
                    side,
                    qty: default_qty,
                    r#type: "MARKET".into(),
                    ts: chrono::Utc::now().timestamp_millis(),
                };

                let bytes = serde_json::to_vec(&order)?;
                producer.send(
                    FutureRecord::to(&topic_out).payload(&bytes).key(&order.symbol),
                    Duration::from_secs(5)
                ).await.ok();
            }
        }
    }
    Ok(())
}
