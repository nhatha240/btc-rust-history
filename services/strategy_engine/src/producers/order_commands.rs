use anyhow::{Context, Result};
use hft_common::{ids::new_trace_id, time::now_ns};
use hft_proto::encode::to_bytes;
use hft_proto::oms::{OrderCommand, OrderType, TimeInForce};
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::time::Duration;

use crate::config::Config;
use crate::planner::idempotency::IdempotencyPlanner;

pub fn build_manual_order(cfg: &Config, idempotency: &IdempotencyPlanner, trace_id: &str) -> OrderCommand {
    let client_order_id = idempotency.client_order_id(trace_id, &cfg.symbol, cfg.side);
    OrderCommand {
        account_id: cfg.account_id.clone(),
        symbol: cfg.symbol.clone(),
        client_order_id,
        side: cfg.side,
        r#type: OrderType::Limit as i32,
        tif: TimeInForce::Gtc as i32,
        qty: cfg.qty,
        price: cfg.price,
        reduce_only: false,
        stop_price: cfg.stop_price,
        decision_reason: if cfg.stop_price > 0.0 {
            format!("manual-test tp/sl=0/{:.2}", cfg.stop_price)
        } else {
            "manual-test".to_string()
        },
        trace_id: trace_id.to_string(),
        decision_time_ns: now_ns(),
    }
}

pub async fn publish_order_command(
    producer: &FutureProducer,
    topic: &str,
    order: &OrderCommand,
) -> Result<()> {
    let payload = to_bytes(order).context("encode OrderCommand failed")?;
    producer
        .send(
            FutureRecord::to(topic)
                .payload(payload.as_ref())
                .key(&order.account_id),
            Duration::from_secs(0),
        )
        .await
        .map_err(|(e, _)| anyhow::anyhow!("publish OrderCommand failed: {e}"))?;
    Ok(())
}

pub fn new_trace() -> String {
    new_trace_id()
}
