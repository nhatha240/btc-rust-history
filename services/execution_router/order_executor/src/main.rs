use anyhow::{Context, Result};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::Message;
use sqlx::{Pool, Postgres};
use tracing::{error, info};
use uuid::Uuid;

use hft_proto::encode::from_bytes;
use hft_proto::oms::{ExecutionReport, ExecutionStatus};
use hft_store::pg::create_pool;
use hft_store::repos::{insert_order_event, insert_trade, update_position, upsert_order};

mod config;
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    info!("Starting Order Executor service...");
    let cfg = Config::from_env().context("Failed to load config")?;
    info!(config = ?cfg, "Config loaded");

    // Initialize Database pool
    let pool = create_pool(&cfg.database_url, 10).await?;

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
        .subscribe(&[&cfg.kafka_topic_fills])
        .context("Failed to subscribe to topic")?;

    info!(
        "Order Executor is ready. Consuming fills from topic: {}",
        cfg.kafka_topic_fills
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

                // Deserialize ExecutionReport
                let report: ExecutionReport = match from_bytes(payload) {
                    Ok(r) => r,
                    Err(e) => {
                        error!("Failed to decode ExecutionReport: {}", e);
                        continue;
                    }
                };

                info!(
                    order_id = %report.client_order_id,
                    status = ?report.status,
                    "Received fill report, persisting..."
                );

                if let Err(e) = persist_report(&pool, &report).await {
                    error!(order_id = %report.client_order_id, "Failed to persist report: {}", e);
                }
            }
        }
    }
}

async fn persist_report(pool: &Pool<Postgres>, report: &ExecutionReport) -> Result<()> {
    let client_order_id =
        Uuid::parse_str(&report.client_order_id).unwrap_or_else(|_| Uuid::now_v7());
    let mut tx = pool.begin().await?;

    // 1. Process orders table - Upsert
    upsert_order(&mut *tx, client_order_id, report).await?;

    // 2. Insert into order_events
    insert_order_event(&mut *tx, client_order_id, "EXECUTION_REPORT", report).await?;

    // 3. If filled, insert into trades
    if report.status == (ExecutionStatus::Filled as i32)
        || report.status == (ExecutionStatus::PartiallyFilled as i32)
    {
        insert_trade(&mut *tx, client_order_id, report).await?;
        update_position(&mut *tx, report).await?;
    }

    tx.commit().await?;
    Ok(())
}
