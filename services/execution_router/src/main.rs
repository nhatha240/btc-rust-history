use anyhow::{Context, Result};
use hft_exchange::{BinanceRestClient, RateLimiter};
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
use producers::execution_reports::{build_cancel, build_exchange_ack, build_reject, build_reports};
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
    let binance_client = build_binance_client(&cfg);

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
                    binance_client.as_ref(),
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
    binance_client: Option<&BinanceRestClient>,
) -> Result<()> {
    if cfg.execution_mode.eq_ignore_ascii_case("PAPER") {
        if order.action == hft_proto::oms::OrderAction::Cancel as i32 {
            info!(client_order_id=%order.client_order_id, "PAPER mode: generating simulated cancel report");
            let report = build_cancel(order);
            writer
                .persist_execution(order, &report, &cfg.execution_mode, &cfg.exchange)
                .await
                .context("persist execution report failed")?;

            publish_report(producer, &cfg.kafka_topic_execution_reports, &report).await?;
            return Ok(());
        }

        info!(client_order_id=%order.client_order_id, "PAPER mode: generating simulated reports");
        let reports = build_reports(order);
        for report in reports {
            writer
                .persist_execution(order, &report, &cfg.execution_mode, &cfg.exchange)
                .await
                .context("persist execution report failed")?;

            publish_report(producer, &cfg.kafka_topic_execution_reports, &report).await?;
        }
    } else {
        let exchange = cfg.exchange.trim().to_ascii_uppercase();
        match exchange.as_str() {
            "BINANCE" => {
                let client = match binance_client {
                    Some(client) => client,
                    None => {
                        let reject = build_reject(order, "BINANCE_CLIENT_NOT_CONFIGURED");
                        writer
                            .persist_execution(order, &reject, &cfg.execution_mode, &cfg.exchange)
                            .await?;
                        publish_report(producer, &cfg.kafka_topic_execution_reports, &reject).await?;
                        return Ok(());
                    }
                };

                if order.r#type != hft_proto::oms::OrderType::Market as i32 {
                    let reject = build_reject(order, "REAL_BINANCE_ONLY_SUPPORTS_MARKET");
                    writer
                        .persist_execution(order, &reject, &cfg.execution_mode, &cfg.exchange)
                        .await?;
                    publish_report(producer, &cfg.kafka_topic_execution_reports, &reject).await?;
                    return Ok(());
                }

                match client.place_order(order).await {
                    Ok(ack) => {
                        info!(
                            client_order_id=%order.client_order_id,
                            exchange_order_id=%ack.order_id,
                            "REAL BINANCE order accepted"
                        );
                        let ack_report = build_exchange_ack(order, &ack);
                        writer
                            .persist_execution(order, &ack_report, &cfg.execution_mode, &cfg.exchange)
                            .await?;
                        publish_report(producer, &cfg.kafka_topic_execution_reports, &ack_report).await?;
                    }
                    Err(e) => {
                        warn!(client_order_id=%order.client_order_id, error=%e, "REAL BINANCE order rejected");
                        let reject = build_reject(order, &format!("BINANCE_ERROR:{e}"));
                        writer
                            .persist_execution(order, &reject, &cfg.execution_mode, &cfg.exchange)
                            .await?;
                        publish_report(producer, &cfg.kafka_topic_execution_reports, &reject).await?;
                    }
                }
            }
            "OKX" => {
                warn!(client_order_id=%order.client_order_id, "OKX adapter not implemented");
                let reject = build_reject(order, "OKX_NOT_IMPLEMENTED");
                writer
                    .persist_execution(order, &reject, &cfg.execution_mode, &cfg.exchange)
                    .await?;
                publish_report(producer, &cfg.kafka_topic_execution_reports, &reject).await?;
            }
            _ => {
                let reject = build_reject(order, "UNSUPPORTED_EXCHANGE");
                writer
                    .persist_execution(order, &reject, &cfg.execution_mode, &cfg.exchange)
                    .await?;
                publish_report(producer, &cfg.kafka_topic_execution_reports, &reject).await?;
            }
        }
    }
    Ok(())
}

fn build_binance_client(cfg: &Config) -> Option<BinanceRestClient> {
    if !cfg.execution_mode.eq_ignore_ascii_case("REAL") {
        return None;
    }
    if !cfg.exchange.eq_ignore_ascii_case("BINANCE") {
        return None;
    }
    if cfg.binance_api_key.is_empty() || cfg.binance_api_secret.is_empty() {
        warn!("REAL BINANCE mode selected but API key/secret is missing");
        return None;
    }

    Some(BinanceRestClient::new(
        cfg.binance_api_key.clone(),
        cfg.binance_api_secret.clone(),
        cfg.binance_base_url.clone(),
        RateLimiter::new(200.0, 10.0),
    ))
}

async fn publish_report(
    producer: &FutureProducer,
    topic: &str,
    report: &hft_proto::oms::ExecutionReport,
) -> Result<()> {
    let payload = to_bytes(report).context("encode execution report failed")?;
    producer
        .send(
            FutureRecord::to(topic)
                .payload(payload.as_ref())
                .key(&report.account_id),
            Duration::from_secs(0),
        )
        .await
        .map_err(|(e, _)| anyhow::anyhow!("publish execution report failed: {e}"))?;
    Ok(())
}
