use anyhow::{Context, Result};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;
use std::time::Duration;
use tracing::{error, info, warn};

use hft_proto::encode::to_bytes;
use hft_proto::oms::OrderCommand;
use hft_store::pg::create_pool;

mod config;
mod consumers {
    pub mod order_commands;
}
mod oms {
    pub mod dedup;
    pub mod order_state;
    pub mod reconcile;
}
mod producers {
    pub mod execution_reports;
    pub mod positions_snapshots;
}
mod exchange {
    pub mod binance_rest;
    pub mod binance_ws_userstream;
    pub mod rate_limit;
}
mod store {
    pub mod writer;
}
mod health;

use config::Config;
use consumers::order_commands::decode_order_command;
use oms::dedup::DedupCache;
use producers::execution_reports::build_reports;
use store::writer::DbWriter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let cfg = Config::from_env().context("load execution_router config failed")?;
    info!(topic_in=%cfg.kafka_topic_order_commands, topic_out=%cfg.kafka_topic_execution_reports, "execution_router starting");

    let pool = create_pool(&cfg.database_url, 10).await?;
    let writer = DbWriter::new(pool);
    let dedup = DedupCache::new(Duration::from_secs(60 * 60));

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.kafka_brokers)
        .set("message.timeout.ms", "5000")
        .set("acks", "all")
        .create()
        .context("create Kafka producer failed")?;

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.kafka_brokers)
        .set("group.id", &cfg.kafka_group_id)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "earliest")
        .create()
        .context("create Kafka consumer failed")?;

    consumer
        .subscribe(&[&cfg.kafka_topic_order_commands])
        .context("subscribe order command topic failed")?;

    loop {
        match consumer.recv().await {
            Err(e) => error!("kafka recv error: {e}"),
            Ok(msg) => {
                let payload = match msg.payload() {
                    Some(p) => p,
                    None => continue,
                };

                let order = match decode_order_command(payload) {
                    Ok(o) => o,
                    Err(e) => {
                        error!("decode order command failed: {e:#}");
                        continue;
                    }
                };

                if order.client_order_id.is_empty() {
                    warn!("skip order without client_order_id");
                    continue;
                }

                if dedup.seen_recently_or_insert(&order.client_order_id) {
                    info!(client_order_id=%order.client_order_id, "dedup: skip duplicated order command");
                    continue;
                }

                if let Err(e) = handle_order(
                    &producer,
                    &writer,
                    &cfg,
                    &order,
                )
                .await
                {
                    error!(client_order_id=%order.client_order_id, "handle order failed: {e:#}");
                }
            }
        }
    }
}

async fn handle_order(
    producer: &FutureProducer,
    writer: &DbWriter,
    cfg: &Config,
    order: &OrderCommand,
) -> Result<()> {
    if cfg.execution_mode == "PAPER" {
        info!(client_order_id=%order.client_order_id, "PAPER mode: generating simulated reports");
        let reports = build_reports(order);
        for report in reports {
            writer
                .persist_execution(order, &report)
                .await
                .context("persist execution report failed")?;

            let payload = to_bytes(&report).context("encode execution report failed")?;
            producer
                .send(
                    FutureRecord::to(&cfg.kafka_topic_execution_reports)
                        .payload(payload.as_ref())
                        .key(&report.account_id),
                    Duration::from_secs(0),
                )
                .await
                .map_err(|(e, _)| anyhow::anyhow!("publish execution report failed: {e}"))?;
        }
    } else {
        warn!(client_order_id=%order.client_order_id, "REAL mode requested but not fully implemented in this loop yet");
        // TODO: call binance_rest / gatekeeper logic
    }
    Ok(())
}
