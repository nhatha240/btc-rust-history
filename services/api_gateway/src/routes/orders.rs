use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use hft_store::pg::models::{EventRow, OrderRow};
use hft_store::pg::types::{DbOrderSide, DbOrderStatus, DbOrderType, DbTimeInForce};
use hft_store::repos::{get_order_by_id, list_events_for_order, list_orders};
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
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn router(pool: Pool<Postgres>) -> Router {
    Router::new()
        .route("/", get(handle_list_orders))
        .route("/:id", get(handle_get_order))
        .route("/:id/events", get(handle_get_order_events))
        .with_state(pool)
}

// ── handlers ──────────────────────────────────────────────────────────────────

async fn handle_list_orders(
    State(pool): State<Pool<Postgres>>,
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
            })
            .collect();
        return Json(filtered);
    }

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    match list_orders(&pool, params.symbol, params.status, limit, offset).await {
        Ok(orders) => Json(orders),
        Err(e) => {
            tracing::error!("Failed to list orders: {}", e);
            Json(vec![])
        }
    }
}

async fn handle_get_order(
    State(pool): State<Pool<Postgres>>,
    Path(id): Path<Uuid>,
) -> Result<Json<OrderRow>, StatusCode> {
    if is_mock() {
        let order = mock_orders().into_iter().find(|o| o.client_order_id == id);
        return order.map(Json).ok_or(StatusCode::NOT_FOUND);
    }

    match get_order_by_id(&pool, id).await {
        Ok(Some(order)) => Ok(Json(order)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get order {}: {}", id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn handle_get_order_events(
    State(pool): State<Pool<Postgres>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<EventRow>>, StatusCode> {
    if is_mock() {
        return Ok(Json(mock_events(id)));
    }

    match list_events_for_order(&pool, id).await {
        Ok(events) => Ok(Json(events)),
        Err(e) => {
            tracing::error!("Failed to get events for order {}: {}", id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
