use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use crate::mq::OrderProducer;
use hft_store::pg::models::{EventRow, OrderExitTagSnapshot, OrderRow, OrderTrainingEventRow};
use hft_store::pg::types::{DbOrderSide, DbOrderStatus, DbOrderType, DbTimeInForce};
use hft_store::repos::{
    get_order_by_id, list_events_for_order, list_order_exit_tag_snapshots, list_order_training_events,
    list_orders,
};
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use std::str::FromStr;
use uuid::Uuid;

// ── helpers ──────────────────────────────────────────────────────────────────

fn is_mock() -> bool {
    std::env::var("MOCK_DATA").map(|v| v == "1" || v == "true").unwrap_or(false)
}

// ── mock fixtures ─────────────────────────────────────────────────────────────

fn mock_orders() -> Vec<OrderRow> {
    use chrono::{Duration, Utc};
    let now = Utc::now();
    let ids = [
        "00000000-0000-0000-0000-000000000001",
        "00000000-0000-0000-0000-000000000002",
        "00000000-0000-0000-0000-000000000003",
    ];
    ids.iter()
        .enumerate()
        .map(|(i, id)| OrderRow {
            id: (i + 1) as i64,
            client_order_id: Uuid::parse_str(id).unwrap(),
            exchange_order_id: Some(1_000_000_i64 + i as i64),
            account_id: "main_account".to_string(),
            symbol: if i % 2 == 0 { "BTCUSDT".to_string() } else { "ETHUSDT".to_string() },
            side: if i % 2 == 0 { DbOrderSide::Buy } else { DbOrderSide::Sell },
            r#type: DbOrderType::Limit,
            tif: DbTimeInForce::Gtc,
            qty: Decimal::from_str("0.05").unwrap(),
            price: Some(Decimal::from_str(if i == 0 { "62000.00" } else if i == 1 { "3100.00" } else { "61500.00" }).unwrap()),
            stop_price: None,
            take_profit_price: Some(Decimal::from_str(if i == 0 { "63000.00" } else if i == 1 { "3250.00" } else { "62000.00" }).unwrap()),
            coin_tags: serde_json::json!(["hot", "long"]),
            status: match i {
                0 => DbOrderStatus::Filled,
                1 => DbOrderStatus::Canceled,
                _ => DbOrderStatus::PartiallyFilled,
            },
            filled_qty: Decimal::from_str(match i { 0 => "0.05", 1 => "0.00", _ => "0.02" }).unwrap(),
            avg_price: if i == 0 { Some(Decimal::from_str("62005.50").unwrap()) } else { None },
            reduce_only: false,
            trace_id: None,
            strategy_version: Some("v2.1.0".to_string()),
            ack_at: Some(now - Duration::hours(2 + i as i64) + Duration::seconds(1)),
            done_at: if i == 0 { Some(now - Duration::hours(2 + i as i64) + Duration::minutes(5)) } else { None },
            created_at: now - Duration::hours(2 + i as i64),
            updated_at: now - Duration::minutes(30 + i as i64 * 10),
        })
        .collect()
}

fn mock_events(client_order_id: Uuid) -> Vec<EventRow> {
    use chrono::{Duration, Utc};
    let now = Utc::now();
    let base = now - chrono::Duration::hours(2);

    let events = vec![
        ("SUBMITTED",        Duration::seconds(0),  serde_json::json!({"action":"submit","qty":"0.05","price":"62000.00"})),
        ("ACKNOWLEDGED",     Duration::seconds(1),  serde_json::json!({"exchange_order_id":1000001,"latency_ms":47})),
        ("PARTIALLY_FILLED", Duration::seconds(30), serde_json::json!({"filled_qty":"0.02","avg_price":"61998.50","trade_id":9001})),
        ("PARTIALLY_FILLED", Duration::minutes(2),  serde_json::json!({"filled_qty":"0.03","avg_price":"62010.00","trade_id":9002})),
        ("FILLED",           Duration::minutes(5),  serde_json::json!({"filled_qty":"0.05","avg_price":"62005.50","commission":"0.000025 BTC"})),
    ];

    events
        .into_iter()
        .enumerate()
        .map(|(i, (ev_type, offset, payload))| EventRow {
            id: i as i64 + 1,
            client_order_id,
            event_type: ev_type.to_string(),
            payload,
            event_time: base + offset,
        })
        .collect()
}

// ── router ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct OrderParams {
    pub symbol: Option<String>,
    pub status: Option<String>,
    pub strategy_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Clone)]
pub struct OrderState {
    pub pool: Pool<Postgres>,
    pub order_mq: Arc<OrderProducer>,
}

pub fn router(state: OrderState) -> Router {
    Router::new()
        .route("/", get(handle_list_orders))
        .route("/market", post(handle_place_market_order))
        .route("/exit", post(handle_place_exit_order))
        .route("/training-events", get(handle_list_training_events))
        .route("/{id}/exit-tags", get(handle_get_order_exit_tags))
        .route("/{id}", get(handle_get_order))
        .route("/{id}/events", get(handle_get_order_events))
        .route("/{id}/cancel", post(handle_cancel_order))
        .route("/cancel_all", post(handle_cancel_all_orders))
        .with_state(state)
}

// ── handlers ──────────────────────────────────────────────────────────────────

async fn handle_list_orders(
    State(state): State<OrderState>,
    Query(params): Query<OrderParams>,
) -> Json<Vec<OrderRow>> {
    if is_mock() {
        let orders = mock_orders();
        // Optionally filter by symbol / status
        let filtered: Vec<_> = orders
            .into_iter()
            .filter(|o| {
                params.symbol.as_ref().map_or(true, |s| o.symbol == *s)
                    && params.status.as_ref().map_or(true, |st| {
                        format!("{:?}", o.status).to_uppercase() == st.to_uppercase()
                    })
                    && params.strategy_id.as_ref().map_or(true, |sid| o.strategy_version.as_ref() == Some(sid))
            })
            .collect();
        return Json(filtered);
    }

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    match list_orders(&state.pool, params.symbol, params.status, params.strategy_id, limit, offset).await {
        Ok(orders) => Json(orders),
        Err(e) => {
            tracing::error!("Failed to list orders: {}", e);
            Json(vec![])
        }
    }
}

async fn handle_get_order(
    State(state): State<OrderState>,
    Path(id): Path<Uuid>,
) -> Result<Json<OrderRow>, StatusCode> {
    if is_mock() {
        let order = mock_orders().into_iter().find(|o| o.client_order_id == id);
        return order.map(Json).ok_or(StatusCode::NOT_FOUND);
    }

    match get_order_by_id(&state.pool, id).await {
        Ok(Some(order)) => Ok(Json(order)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get order {}: {}", id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn handle_get_order_events(
    State(state): State<OrderState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<EventRow>>, StatusCode> {
    if is_mock() {
        return Ok(Json(mock_events(id)));
    }

    match list_events_for_order(&state.pool, id).await {
        Ok(events) => Ok(Json(events)),
        Err(e) => {
            tracing::error!("Failed to get events for order {}: {}", id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Deserialize)]
pub struct PlaceMarketOrderRequest {
    pub account_id: String,
    pub symbol: String,
    pub side: String, // BUY / SELL
    pub qty: f64,
    pub stop_loss_price: f64,
    pub take_profit_price: f64,
    pub coin_tags: Option<Vec<String>>,
    pub exchange: Option<String>, // "binance" | "okx"
    pub strategy_id: Option<String>,
    pub signal_id: Option<String>,
}

fn parse_side(side: &str) -> Option<i32> {
    let normalized = side.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "BUY" => Some(hft_proto::oms::OrderSide::Buy as i32),
        "SELL" => Some(hft_proto::oms::OrderSide::Sell as i32),
        _ => None,
    }
}

async fn handle_place_market_order(
    State(state): State<OrderState>,
    Json(req): Json<PlaceMarketOrderRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if req.qty <= 0.0 || req.stop_loss_price <= 0.0 || req.take_profit_price <= 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let side = parse_side(&req.side).ok_or(StatusCode::BAD_REQUEST)?;
    let exchange = req.exchange.unwrap_or_else(|| "binance".to_string()).to_ascii_lowercase();
    let trace_id = uuid::Uuid::now_v7().to_string();
    let client_order_id = uuid::Uuid::now_v7().to_string();
    let decision_meta = serde_json::json!({
        "reason": "manual_market_with_tp_sl",
        "exchange": exchange,
        "stop_loss_price": req.stop_loss_price,
        "take_profit_price": req.take_profit_price,
        "coin_tags": req.coin_tags.unwrap_or_default(),
        "origin": "api_gateway:/api/orders/market"
    });

    let cmd = hft_proto::oms::OrderCommand {
        account_id: req.account_id,
        symbol: req.symbol,
        client_order_id: client_order_id.clone(),
        side,
        r#type: hft_proto::oms::OrderType::Market as i32,
        tif: hft_proto::oms::TimeInForce::Ioc as i32,
        qty: req.qty,
        price: 0.0,
        reduce_only: false,
        stop_price: req.stop_loss_price,
        decision_reason: decision_meta.to_string(),
        trace_id: trace_id.clone(),
        decision_time_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        schema_version: 1,
        action: hft_proto::oms::OrderAction::Create as i32,
        strategy_id: req.strategy_id.unwrap_or_default(),
        signal_id: req.signal_id.unwrap_or_default(),
    };

    if let Err(e) = state.order_mq.submit_order(cmd).await {
        tracing::error!("Failed to submit market order: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(Json(serde_json::json!({
        "status": "submitted",
        "client_order_id": client_order_id,
        "trace_id": trace_id,
        "exchange": exchange,
        "tp_sl": {
            "stop_loss_price": req.stop_loss_price,
            "take_profit_price": req.take_profit_price
        }
    })))
}

#[derive(Deserialize)]
pub struct PlaceExitOrderRequest {
    pub account_id: String,
    pub symbol: String,
    pub side: String, // usually opposite side of open position
    pub qty: f64,
    pub exit_kind: String, // STOP_LOSS | TAKE_PROFIT
    pub stop_loss_price: Option<f64>,
    pub take_profit_price: Option<f64>,
    pub coin_tags: Option<Vec<String>>,
    pub exchange: Option<String>,
    pub strategy_id: Option<String>,
    pub signal_id: Option<String>,
}

fn normalize_exit_kind(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "STOP_LOSS" | "SL" => Some("STOP_LOSS"),
        "TAKE_PROFIT" | "TP" => Some("TAKE_PROFIT"),
        _ => None,
    }
}

async fn handle_place_exit_order(
    State(state): State<OrderState>,
    Json(req): Json<PlaceExitOrderRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if req.qty <= 0.0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let side = parse_side(&req.side).ok_or(StatusCode::BAD_REQUEST)?;
    let exit_kind = normalize_exit_kind(&req.exit_kind).ok_or(StatusCode::BAD_REQUEST)?;
    let exchange = req.exchange.unwrap_or_else(|| "binance".to_string()).to_ascii_lowercase();
    let trace_id = uuid::Uuid::now_v7().to_string();
    let client_order_id = uuid::Uuid::now_v7().to_string();
    let stop_loss_price = req.stop_loss_price.unwrap_or(0.0);
    let take_profit_price = req.take_profit_price.unwrap_or(0.0);

    let decision_meta = serde_json::json!({
        "reason": "manual_exit_order",
        "exit_kind": exit_kind,
        "exchange": exchange,
        "stop_loss_price": stop_loss_price,
        "take_profit_price": take_profit_price,
        "coin_tags": req.coin_tags.unwrap_or_default(),
        "origin": "api_gateway:/api/orders/exit"
    });

    let cmd = hft_proto::oms::OrderCommand {
        account_id: req.account_id,
        symbol: req.symbol,
        client_order_id: client_order_id.clone(),
        side,
        r#type: hft_proto::oms::OrderType::Market as i32,
        tif: hft_proto::oms::TimeInForce::Ioc as i32,
        qty: req.qty,
        price: 0.0,
        reduce_only: true,
        stop_price: stop_loss_price,
        decision_reason: decision_meta.to_string(),
        trace_id: trace_id.clone(),
        decision_time_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        schema_version: 1,
        action: hft_proto::oms::OrderAction::Create as i32,
        strategy_id: req.strategy_id.unwrap_or_default(),
        signal_id: req.signal_id.unwrap_or_default(),
    };

    if let Err(e) = state.order_mq.submit_order(cmd).await {
        tracing::error!("Failed to submit exit order: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(Json(serde_json::json!({
        "status": "submitted",
        "client_order_id": client_order_id,
        "trace_id": trace_id,
        "exit_kind": exit_kind
    })))
}

async fn handle_get_order_exit_tags(
    State(state): State<OrderState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<OrderExitTagSnapshot>>, StatusCode> {
    if is_mock() {
        return Ok(Json(vec![]));
    }

    match list_order_exit_tag_snapshots(&state.pool, id).await {
        Ok(rows) => Ok(Json(rows)),
        Err(e) => {
            tracing::error!("Failed to get exit tag snapshots for order {}: {}", id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Deserialize)]
pub struct TrainingEventParams {
    pub symbol: Option<String>,
    pub execution_mode: Option<String>,
    pub outcome_label: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

async fn handle_list_training_events(
    State(state): State<OrderState>,
    Query(params): Query<TrainingEventParams>,
) -> Result<Json<Vec<OrderTrainingEventRow>>, StatusCode> {
    if is_mock() {
        return Ok(Json(vec![]));
    }

    let limit = params.limit.unwrap_or(200);
    let offset = params.offset.unwrap_or(0);

    match list_order_training_events(
        &state.pool,
        params.symbol,
        params.execution_mode,
        params.outcome_label,
        limit,
        offset,
    )
    .await
    {
        Ok(rows) => Ok(Json(rows)),
        Err(e) => {
            tracing::error!("Failed to list order training events: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn handle_cancel_order(
    State(state): State<OrderState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if is_mock() {
        return Ok(Json(serde_json::json!({ "status": "ok", "message": "mock cancelled" })));
    }

    let order = match get_order_by_id(&state.pool, id).await {
        Ok(Some(o)) => o,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to fetch order for cancel: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    if format!("{:?}", order.status) == "Filled" || format!("{:?}", order.status) == "Canceled" || format!("{:?}", order.status) == "Rejected" {
         return Err(StatusCode::BAD_REQUEST);
    }

    use hft_proto::oms::{OrderCommand, OrderAction};
    let trace_id = uuid::Uuid::now_v7().to_string();

    let cmd = OrderCommand {
        account_id: order.account_id,
        symbol: order.symbol,
        client_order_id: order.client_order_id.to_string(),
        side: 0,
        r#type: 0,
        tif: 0,
        qty: 0.0,
        price: 0.0,
        reduce_only: false,
        stop_price: 0.0,
        decision_reason: "Manual cancel via dashboard".to_string(),
        trace_id: trace_id.clone(),
        decision_time_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        schema_version: 1,
        action: OrderAction::Cancel as i32,
        strategy_id: order.strategy_version.unwrap_or_default(),
        signal_id: String::new(),
    };

    let client_order_id = cmd.client_order_id.clone();
    if let Err(e) = state.order_mq.submit_order(cmd).await {
        tracing::error!("Failed to submit cancel order {}: {}", client_order_id, e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(Json(serde_json::json!({
        "status": "ok",
        "client_order_id": order.client_order_id.to_string(),
        "trace_id": trace_id
    })))
}

#[derive(Deserialize)]
pub struct CancelAllParams {
    pub symbol: Option<String>,
    pub strategy_id: Option<String>,
}

async fn handle_cancel_all_orders(
    State(state): State<OrderState>,
    Query(params): Query<CancelAllParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match list_orders(&state.pool, params.symbol.clone(), None, params.strategy_id.clone(), 1000, 0).await {
         Ok(orders) => {
            let mut cancel_count = 0;
            use hft_proto::oms::{OrderCommand, OrderAction};
            for order in orders {
                 let status_str = format!("{:?}", order.status).to_uppercase();
                 if status_str == "NEW" || status_str == "PARTIALLY_FILLED" {
                     let trace_id = uuid::Uuid::now_v7().to_string();
                     let cmd = OrderCommand {
                        account_id: order.account_id.clone(),
                        symbol: order.symbol.clone(),
                        client_order_id: order.client_order_id.to_string(),
                        side: 0,
                        r#type: 0,
                        tif: 0,
                        qty: 0.0,
                        price: 0.0,
                        reduce_only: false,
                        stop_price: 0.0,
                        decision_reason: "Manual bulk cancel via dashboard".to_string(),
                        trace_id: trace_id.clone(),
                        decision_time_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
                        schema_version: 1,
                        action: OrderAction::Cancel as i32,
                        strategy_id: order.strategy_version.clone().unwrap_or_default(),
                        signal_id: String::new(),
                     };
                     if state.order_mq.submit_order(cmd).await.is_ok() {
                         cancel_count += 1;
                     }
                 }
            }
            Ok(Json(serde_json::json!({
                "status": "ok",
                "cancelled_count": cancel_count,
                "symbol_filter": params.symbol,
                "strategy_filter": params.strategy_id
            })))
         },
         Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR)
    }

}
