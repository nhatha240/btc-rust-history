//! Audit trail for rejected orders.
//!
//! Every rejection is:
//! 1. Written to `risk_rejections` (PostgreSQL) for the dashboard query.
//! 2. Published as an `ExecutionReport{status=REJECTED}` to
//!    `TOPIC_EXECUTION_REPORTS` so all downstream consumers see it.

use std::time::Duration;

use anyhow::Result;
use hft_proto::{
    encode::to_bytes,
    oms::{ExecutionReport, ExecutionStatus, OrderCommand},
};
use hft_risk::RejectReason;
use rdkafka::producer::{FutureProducer, FutureRecord};
use sqlx::{Pool, Postgres};
use tracing::{error, info};

use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;

// ── PostgreSQL ────────────────────────────────────────────────────────────────

/// Persist a rejection to `risk_rejections`.
pub async fn log_rejection_to_db(
    pool: &Pool<Postgres>,
    order: &OrderCommand,
    reason: &RejectReason,
    detail: &str,
) {
    let qty = Decimal::from_f64(order.qty).unwrap_or_default();
    let price = Decimal::from_f64(order.price).unwrap_or_default();
    let notional = qty * price;

    let result = sqlx::query(
        r#"
        INSERT INTO risk_rejections
            (client_order_id, account_id, symbol, qty, price, notional,
             reject_reason, reject_detail, trace_id, rejected_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, now())
        "#,
    )
    .bind(&order.client_order_id)
    .bind(&order.account_id)
    .bind(&order.symbol)
    .bind(qty)
    .bind(price)
    .bind(notional)
    .bind(reason.as_str())
    .bind(detail)
    .bind(&order.trace_id)
    .execute(pool)
    .await;

    if let Err(e) = result {
        error!(
            order_id = %order.client_order_id,
            reason   = reason.as_str(),
            err      = %e,
            "Failed to persist rejection to DB"
        );
    } else {
        info!(
            order_id = %order.client_order_id,
            reason   = reason.as_str(),
            "Rejection persisted to risk_rejections"
        );
    }
}

// ── Kafka ExecutionReport(REJECTED) ───────────────────────────────────────────

/// Publish a `REJECTED` `ExecutionReport` to `TOPIC_EXECUTION_REPORTS`.
///
/// This is the canonical way to signal a rejection to the web dashboard and
/// any other downstream consumer: it reuses the same protobuf message that
/// real exchange fills use, just with `status = REJECTED`.
pub async fn publish_rejection_report(
    producer: &FutureProducer,
    cfg: &Config,
    order: &OrderCommand,
    reason: &RejectReason,
    detail: &str,
) {
    let report = ExecutionReport {
        account_id:           order.account_id.clone(),
        symbol:               order.symbol.clone(),
        client_order_id:      order.client_order_id.clone(),
        exchange_order_id:    String::new(),
        status:               ExecutionStatus::Rejected as i32,
        side:                 order.side,
        last_filled_qty:      0.0,
        last_filled_price:    0.0,
        cumulative_filled_qty: 0.0,
        avg_price:            0.0,
        commission:           0.0,
        commission_asset:     String::new(),
        reject_reason:        format!("{}|{}", reason.as_str(), detail),
        event_time_ns:        hft_common::now_ns() as i64,
        recv_time_ns:         hft_common::now_ns() as i64,
        trace_id:             order.trace_id.clone(),
        fill_id:              String::new(),
        fill_seq:             0,
    };

    let buf = match to_bytes(&report) {
        Ok(b) => b,
        Err(e) => {
            error!(order_id = %order.client_order_id, err = %e, "Failed to encode rejection report");
            return;
        }
    };

    let res = producer
        .send(
            FutureRecord::to(&cfg.kafka_topic_execution_reports)
                .payload(buf.as_ref())
                .key(&order.account_id),
            Duration::from_secs(0),
        )
        .await;

    match res {
        Ok(_) => info!(
            order_id = %order.client_order_id,
            reason   = reason.as_str(),
            topic    = %cfg.kafka_topic_execution_reports,
            "REJECTED ExecutionReport published"
        ),
        Err((e, _)) => error!(
            order_id = %order.client_order_id,
            err      = %e,
            "Failed to publish rejection ExecutionReport"
        ),
    }
}
