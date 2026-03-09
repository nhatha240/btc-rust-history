use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use hft_store::pg::models::PositionRow;
use hft_store::repos::{list_positions, get_position_by_symbol};
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use rust_decimal::prelude::ToPrimitive;

use crate::mq::OrderProducer;

#[derive(Clone)]
pub struct PositionState {
    pub pool: Pool<Postgres>,
    pub order_mq: Arc<OrderProducer>,
}

#[derive(Deserialize)]
pub struct PositionParams {
    pub account_id: Option<String>,
}

#[derive(Deserialize)]
pub struct PartialCloseReq {
    pub account_id: Option<String>,
    pub qty: rust_decimal::Decimal,
}

pub fn router(state: PositionState) -> Router {
    Router::new()
        .route("/", get(handle_list_positions))
        .route("/{symbol}/close", post(handle_close_position))
        .route("/{symbol}/partial_close", post(handle_partial_close_position))
        .with_state(state)
}

async fn handle_list_positions(
    State(state): State<PositionState>,
    Query(params): Query<PositionParams>,
) -> Json<Vec<PositionRow>> {
    match list_positions(&state.pool, params.account_id).await {
        Ok(positions) => Json(positions),
        Err(e) => {
            tracing::error!("Failed to list positions: {}", e);
            Json(vec![])
        }
    }
}

async fn handle_close_position(
    State(state): State<PositionState>,
    Path(symbol): Path<String>,
    Query(params): Query<PositionParams>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let account_id = params.account_id.unwrap_or_else(|| "main_account".to_string());
    
    // Fetch current position from DB to know the side
    // (In reality, if it's long, we sell. If short, we buy)
    let pos = match get_position_by_symbol(&state.pool, &account_id, &symbol).await {
        Ok(Some(p)) => p,
        Ok(None) => return Err(axum::http::StatusCode::NOT_FOUND),
        Err(_) => return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    };

    let close_side = if format!("{:?}", pos.side).to_uppercase() == "LONG" {
        hft_proto::oms::OrderSide::Sell
    } else {
        hft_proto::oms::OrderSide::Buy
    };

    let cmd = hft_proto::oms::OrderCommand {
        account_id: pos.account_id,
        symbol: pos.symbol,
        client_order_id: uuid::Uuid::new_v4().to_string(),
        side: close_side as i32,
        r#type: hft_proto::oms::OrderType::Market as i32,
        tif: hft_proto::oms::TimeInForce::Ioc as i32,
        qty: pos.qty.to_f64().unwrap_or(0.0),
        price: 0.0,
        reduce_only: true,
        stop_price: 0.0,
        decision_reason: "Manual Market Close via Dashboard".to_string(),
        trace_id: uuid::Uuid::new_v4().to_string(),
        decision_time_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        schema_version: 1,
        action: hft_proto::oms::OrderAction::Create as i32,
        strategy_id: "manual-dashboard".to_string(),
        signal_id: String::new(),
    };

    if let Err(e) = state.order_mq.submit_order(cmd).await {
        tracing::error!("Failed to send close order: {}", e);
        return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(Json(serde_json::json!({ "status": "closing", "symbol": symbol })))
}

async fn handle_partial_close_position(
    State(state): State<PositionState>,
    Path(symbol): Path<String>,
    Json(req): Json<PartialCloseReq>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let account_id = req.account_id.unwrap_or_else(|| "main_account".to_string());
    let close_qty = req.qty.to_f64().unwrap_or(0.0);
    
    // Fetch current position from DB to know the side
    let pos = match get_position_by_symbol(&state.pool, &account_id, &symbol).await {
        Ok(Some(p)) => p,
        Ok(None) => return Err(axum::http::StatusCode::NOT_FOUND),
        Err(_) => return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    };

    let close_side = if format!("{:?}", pos.side).to_uppercase() == "LONG" {
        hft_proto::oms::OrderSide::Sell
    } else {
        hft_proto::oms::OrderSide::Buy
    };

    let cmd = hft_proto::oms::OrderCommand {
        account_id: pos.account_id,
        symbol: pos.symbol,
        client_order_id: uuid::Uuid::new_v4().to_string(),
        side: close_side as i32,
        r#type: hft_proto::oms::OrderType::Market as i32,
        tif: hft_proto::oms::TimeInForce::Ioc as i32,
        qty: close_qty,
        price: 0.0,
        reduce_only: true,
        stop_price: 0.0,
        decision_reason: format!("Manual Partial Close ({}) via Dashboard", close_qty),
        trace_id: uuid::Uuid::new_v4().to_string(),
        decision_time_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        schema_version: 1,
        action: hft_proto::oms::OrderAction::Create as i32,
        strategy_id: "manual-dashboard".to_string(),
        signal_id: String::new(),
    };

    if let Err(e) = state.order_mq.submit_order(cmd).await {
        tracing::error!("Failed to send partial close order: {}", e);
        return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(Json(serde_json::json!({ "status": "partial_closing", "symbol": symbol, "qty": req.qty })))
}
