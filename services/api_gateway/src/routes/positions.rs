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
use uuid::Uuid;

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
        .route("/:symbol/close", post(handle_close_position))
        .route("/:symbol/partial_close", post(handle_partial_close_position))
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
        account_id: account_id.clone(),
        symbol: symbol.clone(),
        client_order_id: format!("close-{}", Uuid::new_v4()),
        side: close_side.into(),
        r#type: hft_proto::oms::OrderType::Market.into(),
        tif: hft_proto::oms::TimeInForce::Ioc.into(),
        qty: pos.qty.to_string().parse().unwrap_or(0.0), // Need to format Decimal to f64 or equivalent, assuming protos use double
        price: 0.0,
        reduce_only: true, // Key flag for OMS
        stop_price: 0.0,
        decision_reason: "Manual Close from Dashboard".to_string(),
        trace_id: Uuid::new_v4().to_string(),
        decision_time_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        schema_version: 1,
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
        account_id: account_id.clone(),
        symbol: symbol.clone(),
        client_order_id: format!("pclose-{}", Uuid::new_v4()),
        side: close_side.into(),
        r#type: hft_proto::oms::OrderType::Market.into(),
        tif: hft_proto::oms::TimeInForce::Ioc.into(),
        qty: req.qty.to_string().parse().unwrap_or(0.0), // partial qty
        price: 0.0,
        reduce_only: true,
        stop_price: 0.0,
        decision_reason: format!("Manual Partial Close ({}) from Dashboard", req.qty),
        trace_id: Uuid::new_v4().to_string(),
        decision_time_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        schema_version: 1,
    };

    if let Err(e) = state.order_mq.submit_order(cmd).await {
        tracing::error!("Failed to send partial close order: {}", e);
        return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(Json(serde_json::json!({ "status": "partial_closing", "symbol": symbol, "qty": req.qty })))
}
