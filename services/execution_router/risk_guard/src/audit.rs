//! Audit trail for rejected orders.
//!
//! Every rejection is:
//! 1. Written to `risk_rejections` (PostgreSQL) for the dashboard query.
//! 2. Published as an `ExecutionReport{status=REJECTED}` to
//!    `TOPIC_EXECUTION_REPORTS` so all downstream consumers see it.

use std::time::Duration;

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

use crate::config::Config;

// ── PostgreSQL ────────────────────────────────────────────────────────────────

pub async fn log_risk_event_to_db(
    pool: &Pool<Postgres>,
    order: &OrderCommand,
    check_type: &str,
    pass: bool,
    current_value: Option<f64>,
    limit_value: Option<f64>,
    action: &str,
    severity: &str,
) {
    let current_dec = current_value.and_then(Decimal::from_f64);
    let limit_dec = limit_value.and_then(Decimal::from_f64);

    let result = sqlx::query(
        r#"
        INSERT INTO risk_events
            (event_time, check_type, scope_type, scope_ref, severity, pass_flag,
             current_value, limit_value, action_taken, related_order_id, trace_id)
        VALUES
            (now(), $1, 'SYMBOL', $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(check_type)
    .bind(&order.symbol)
    .bind(severity)
    .bind(pass)
    .bind(current_dec)
    .bind(limit_dec)
    .bind(action)
    .bind(&order.client_order_id)
    .bind(&order.trace_id)
    .execute(pool)
    .await;

    if let Err(e) = result {
        error!(
            order_id = %order.client_order_id,
            check = check_type,
            err = %e,
            "Failed to persist risk event to DB"
        );
    }
}

/// Legacy wrapper for log_rejection_to_db (updates to use new risk_events table)
pub async fn log_rejection_to_db(
    pool: &Pool<Postgres>,
    order: &OrderCommand,
    reason: &RejectReason,
    _detail: &str,
) {
    let qty = order.qty;
    let price = order.price;
    let notional = qty * price;

    log_risk_event_to_db(
        pool,
        order,
        reason.as_str(),
        false,
        Some(notional),
        None, // limit unknown here
        "REJECTED",
        "CRITICAL"
    ).await;
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
        schema_version:       1,
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
