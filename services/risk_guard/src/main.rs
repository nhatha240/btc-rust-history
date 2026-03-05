use anyhow::{Context, Result};
use hft_proto::encode::{from_bytes, to_bytes};
use hft_proto::oms::OrderCommand;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;
use std::time::Duration;
use tracing::{error, info, warn};

mod config;
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    info!("Starting Risk Guard service...");
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
        .subscribe(&[&cfg.kafka_topic_orders])
        .context("Failed to subscribe to topic")?;

    // Initialize Redis client
    let redis_client =
        redis::Client::open(cfg.redis_url.as_str()).context("Failed to create Redis client")?;
    let mut redis_conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .context("Failed to get Redis connection")?;

    info!(
        "Risk Guard is ready. Waiting for orders on topic: {}",
        cfg.kafka_topic_orders
    );

    loop {
        match consumer.recv().await {
            Err(e) => error!("Kafka error: {}", e),
            Ok(bm) => {
                let payload = match bm.payload_view::<[u8]>() {
                    None => {
                        warn!("Received empty payload");
                        continue;
                    }
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

                info!(order_id = %order.client_order_id, symbol = %order.symbol, "Received order, validating...");

                // 1. Check Kill-switch
                let kill_switch: Option<String> = redis::cmd("GET")
                    .arg(&cfg.kill_switch_key)
                    .query_async(&mut redis_conn)
                    .await
                    .unwrap_or(None);

                if kill_switch.is_some() {
                    warn!(order_id = %order.client_order_id, "REJECTED: Kill-switch is active");
                    continue;
                }

                // 2. Check Notional Limit
                let notional = order.qty * order.price;
                if notional > cfg.max_notional_per_order {
                    warn!(
                        order_id = %order.client_order_id,
                        notional = notional,
                        max = cfg.max_notional_per_order,
                        "REJECTED: Max notional exceeded"
                    );
                    continue;
                }

                // 3. Check Leverage (Placeholder logic: in real world we need current position/balance)
                // For now, just allow if notional is OK
                info!(order_id = %order.client_order_id, "Order APPROVED. Forwarding to TOPIC_ORDERS_APPROVED");

                // Forward to approved topic
                let buf = to_bytes(&order)?;

                let res = producer
                    .send(
                        FutureRecord::to(&cfg.kafka_topic_orders_approved)
                            .payload(buf.as_ref())
                            .key(&order.account_id),
                        Duration::from_secs(0),
                    )
                    .await;

                match res {
                    Ok(_) => {
                        info!(order_id = %order.client_order_id, "Order successfully forwarded")
                    }
                    Err((e, _)) => {
                        error!(order_id = %order.client_order_id, "Failed to send to Kafka: {}", e)
                    }
                }
            }
        }
    }
}
