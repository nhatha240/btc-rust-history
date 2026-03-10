use anyhow::{Context, Result};
use chrono::Utc;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

use hft_proto::encode::{from_bytes, to_bytes};
use hft_proto::oms::{ExecutionReport, ExecutionStatus, OrderCommand};

mod config;
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    info!("Starting Paper Trader service...");
    let cfg = Config::from_env().context("Failed to load config")?;
    info!(config = ?cfg, "Config loaded");

    // Initialize Kafka producer
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.kafka_brokers)
        .set("message.timeout.ms", "5000")
        .create()
        .context("Failed to create Kafka producer")?;

    // Initialize Kafka consumer
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.kafka_brokers)
        .set("group.id", &cfg.kafka_group_id)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "earliest")
        .create()
        .context("Failed to create Kafka consumer")?;

    consumer
        .subscribe(&[&cfg.kafka_topic_orders_approved])
        .context("Failed to subscribe to topic")?;

    info!(
        "Paper Trader is ready. Simulating fills for topic: {}",
        cfg.kafka_topic_orders_approved
    );

    loop {
        match consumer.recv().await {
            Err(e) => error!("Kafka error: {}", e),
            Ok(bm) => {
                let payload = match bm.payload_view::<[u8]>() {
                    None => continue,
                    Some(Ok(p)) => p,
                    Some(Err(_e)) => {
                        error!("Error viewing payload");
                        continue;
                    }
                };

                // Deserialize OrderCommand
                let order: OrderCommand = match from_bytes(payload) {
                    Ok(o) => o,
                    Err(e) => {
                        error!("Failed to decode OrderCommand: {}", e);
                        continue;
                    }
                };

                info!(
                    order_id = %order.client_order_id,
                    symbol = %order.symbol,
                    side = ?order.side,
                    "Received approved order, simulating FILL..."
                );

                // Simulation: 100ms latency then full FILL
                tokio::time::sleep(Duration::from_millis(100)).await;

                let exchange_order_id = format!("paper-{}", Uuid::now_v7());
                let now_ns = Utc::now().timestamp_nanos_opt().unwrap_or(0);

                let report = ExecutionReport {
                    account_id: order.account_id.clone(),
                    symbol: order.symbol.clone(),
                    client_order_id: order.client_order_id.clone(),
                    exchange_order_id: exchange_order_id.clone(),
                    status: ExecutionStatus::Filled as i32,
                    side: order.side,
                    last_filled_qty: order.qty,
                    last_filled_price: order.price, // In paper trader, we assume fill at order price
                    cumulative_filled_qty: order.qty,
                    avg_price: order.price,
                    commission: order.qty * order.price * 0.0001, // 1bps commission
                    commission_asset: "USDT".to_string(),
                    reject_reason: "".to_string(),
                    event_time_ns: now_ns,
                    recv_time_ns: now_ns,
                    trace_id: order.trace_id.clone(),
                    fill_id: exchange_order_id.clone(),
                    fill_seq: 1,
                    schema_version: 1,
                    strategy_id: order.strategy_id.clone(),
                    signal_id: order.signal_id.clone(),
                };

                // Publish ExecutionReport to TOPIC_FILLS
                let buf = to_bytes(&report)?;

                let res = producer
                    .send(
                        FutureRecord::to(&cfg.kafka_topic_fills)
                            .payload(buf.as_ref())
                            .key(&report.account_id),
                        Duration::from_secs(0),
                    )
                    .await;

                match res {
                    Ok(_) => info!(
                        order_id = %order.client_order_id,
                        ex_id = %exchange_order_id,
                        "FILL message published"
                    ),
                    Err((e, _)) => {
                        error!(order_id = %order.client_order_id, "Failed to publish FILL: {}", e)
                    }
                }
            }
        }
    }
}
