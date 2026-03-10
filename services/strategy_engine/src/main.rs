use anyhow::{Context, Result};
use rdkafka::config::ClientConfig;
use rdkafka::producer::FutureProducer;
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

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

    // Heartbeat reporting
    if let Some(writer) = &decision_writer {
        let writer = writer.clone();
        let instance_id = Uuid::now_v7().to_string();
        tokio::spawn(async move {
            loop {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO strat_health (
                        instance_id, strategy_name, reported_at, cpu_pct, mem_mb
                    ) VALUES ($1, $2, now(), 1.0, 50.0)
                    "#,
                )
                .bind(&instance_id)
                .bind("strategy-p0")
                .execute(&writer.pool)
                .await;
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });
    }

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
            let _ = writer.write_strat_log(
                "strategy-p0",
                &order.symbol,
                "ORDER_SENT",
                &format!("Manual order sent: {}", order.client_order_id),
                None
            ).await;
        }

        if cfg.emit_once {
            break;
        }
        tokio::time::sleep(Duration::from_millis(cfg.emit_interval_ms)).await;
    }

    Ok(())
}
