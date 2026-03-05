use anyhow::{Context, Result};
use hft_proto::encode::{from_bytes, to_bytes};
use hft_proto::oms::OrderCommand;
use hft_redis::{KillSwitch, RedisStore};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tracing::{error, info, warn};

mod audit;
mod checker;
mod config;
mod health;

use checker::CheckResult;
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    info!("Starting risk_guard…");
    let cfg = Config::from_env().context("Failed to load config")?;
    info!(config = ?cfg, "Config loaded");

    // ── Health server ─────────────────────────────────────────────────────────
    let health_addr = cfg.health_addr.parse().context("Invalid HEALTH_ADDR")?;
    health::spawn(health_addr);

    // ── PostgreSQL ────────────────────────────────────────────────────────────
    let pg_pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&cfg.database_url)
        .await
        .context("Failed to connect to PostgreSQL")?;

    // ── Redis / KillSwitch ────────────────────────────────────────────────────
    let redis_store = RedisStore::new(&cfg.redis_url)
        .await
        .context("Failed to connect to Redis")?;
    let mut kill_switch = KillSwitch::new(redis_store);

    // ── Kafka producer ────────────────────────────────────────────────────────
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.kafka_brokers)
        .set("message.timeout.ms", "5000")
        .create()
        .context("Failed to create Kafka producer")?;

    // ── Kafka consumer ────────────────────────────────────────────────────────
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
        .subscribe(&[&cfg.kafka_topic_orders])
        .context("Failed to subscribe to TOPIC_ORDERS")?;

    info!(
        topic = %cfg.kafka_topic_orders,
        "risk_guard ready — awaiting orders"
    );

    // ── Main event loop ───────────────────────────────────────────────────────
    loop {
        let bm = match consumer.recv().await {
            Ok(m) => m,
            Err(e) => {
                error!("Kafka recv error: {e}");
                continue;
            }
        };

        let payload = match bm.payload_view::<[u8]>() {
            Some(Ok(p)) => p,
            Some(Err(_)) => {
                error!("Payload byte-view error");
                continue;
            }
            None => {
                warn!("Empty Kafka payload — skipping");
                continue;
            }
        };

        // Deserialise ─────────────────────────────────────────────────────────
        let order: OrderCommand = match from_bytes(payload) {
            Ok(o) => o,
            Err(e) => {
                error!("Failed to decode OrderCommand: {e}");
                // Nothing to reject without an order — just skip
                continue;
            }
        };

        info!(
            order_id = %order.client_order_id,
            symbol   = %order.symbol,
            qty      = order.qty,
            price    = order.price,
            "Order received — running P0 gates"
        );

        // P0 Gate pipeline ────────────────────────────────────────────────────
        match checker::run_gates(&order, &cfg, &mut kill_switch).await {
            CheckResult::Rejected { reason, detail } => {
                warn!(
                    order_id    = %order.client_order_id,
                    symbol      = %order.symbol,
                    reject_code = reason.as_str(),
                    detail      = %detail,
                    "Order REJECTED"
                );

                // 1. Persist to risk_rejections table (best-effort)
                audit::log_rejection_to_db(&pg_pool, &order, &reason, &detail).await;

                // 2. Publish ExecutionReport(REJECTED) for downstream / web
                audit::publish_rejection_report(&producer, &cfg, &order, &reason, &detail).await;

                // Do NOT forward to TOPIC_ORDERS_APPROVED
            }

            CheckResult::Approved => {
                info!(
                    order_id = %order.client_order_id,
                    symbol   = %order.symbol,
                    "Order APPROVED — forwarding"
                );

                let buf = match to_bytes(&order) {
                    Ok(b) => b,
                    Err(e) => {
                        error!(order_id = %order.client_order_id, "Proto encode error: {e}");
                        continue;
                    }
                };

                match producer
                    .send(
                        FutureRecord::to(&cfg.kafka_topic_orders_approved)
                            .payload(buf.as_ref())
                            .key(&order.account_id),
                        Duration::from_secs(0),
                    )
                    .await
                {
                    Ok(_) => info!(order_id = %order.client_order_id, "Forwarded to TOPIC_ORDERS_APPROVED"),
                    Err((e, _)) => error!(order_id = %order.client_order_id, "Forward failed: {e}"),
                }
            }
        }
    }
}
