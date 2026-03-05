use anyhow::{Context, Result};
use rdkafka::config::ClientConfig;
use rdkafka::producer::FutureProducer;
use std::time::Duration;
use tracing::info;

use hft_store::pg::create_pool;

mod config;
mod planner {
    pub mod idempotency;
}
mod producers {
    pub mod order_commands;
}
mod decision_log {
    pub mod writer;
}
mod cache {
    pub mod latest_features;
}
mod consumers {
    pub mod features;
    pub mod predictions;
    pub mod execution_reports;
}
mod risk {
    pub mod sizing;
    pub mod exposure;
}

use config::Config;
use decision_log::writer::DecisionLogWriter;
use planner::idempotency::IdempotencyPlanner;
use producers::order_commands::{build_manual_order, new_trace, publish_order_command};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let cfg = Config::from_env().context("load strategy_engine config failed")?;
    info!(topic=%cfg.kafka_topic_order_commands, symbol=%cfg.symbol, "strategy_engine starting");

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.kafka_brokers)
        .set("message.timeout.ms", "5000")
        .set("acks", "all")
        .create()
        .context("create Kafka producer failed")?;

    let decision_writer = if cfg.decision_log_enabled {
        let pool = create_pool(&cfg.database_url, 4).await?;
        Some(DecisionLogWriter::new(pool))
    } else {
        None
    };

    let planner = IdempotencyPlanner::new();

    loop {
        let trace_id = new_trace();
        let order = build_manual_order(&cfg, &planner, &trace_id);

        publish_order_command(&producer, &cfg.kafka_topic_order_commands, &order).await?;
        info!(
            client_order_id = %order.client_order_id,
            trace_id = %order.trace_id,
            symbol = %order.symbol,
            side = order.side,
            qty = order.qty,
            "order command published"
        );

        if let Some(writer) = &decision_writer {
            writer.write_enter(&order).await?;
        }

        if cfg.emit_once {
            break;
        }
        tokio::time::sleep(Duration::from_millis(cfg.emit_interval_ms)).await;
    }

    Ok(())
}
