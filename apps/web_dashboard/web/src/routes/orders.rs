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
        .route("/{id}", get(handle_get_order))
        .route("/{id}/events", get(handle_get_order_events))
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

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    /// Create a lazy pool (never connects until a query is executed).
    /// Safe to use in mock-mode tests where DB is never queried.
    fn lazy_pool() -> Pool<Postgres> {
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://mock:mock@localhost:5432/mock")
            .expect("connect_lazy must not fail")
    }

    // ── Unit tests: mock_orders() ─────────────────────────────────────────

    #[test]
    fn mock_orders_count() {
        assert_eq!(mock_orders().len(), 3);
    }

    #[test]
    fn mock_orders_ids_are_sequential() {
        for (i, o) in mock_orders().iter().enumerate() {
            assert_eq!(o.id, (i + 1) as i64);
        }
    }

    #[test]
    fn mock_orders_account_id() {
        for o in mock_orders() {
            assert_eq!(o.account_id, "main_account");
        }
    }

    #[test]
    fn mock_orders_first_is_btcusdt_buy_filled() {
        let orders = mock_orders();
        assert_eq!(orders[0].symbol, "BTCUSDT");
        assert_eq!(orders[0].side, DbOrderSide::Buy);
        assert_eq!(orders[0].status, DbOrderStatus::Filled);
        assert!(orders[0].avg_price.is_some());
    }

    #[test]
    fn mock_orders_second_is_ethusdt_sell_canceled() {
        let orders = mock_orders();
        assert_eq!(orders[1].symbol, "ETHUSDT");
        assert_eq!(orders[1].side, DbOrderSide::Sell);
        assert_eq!(orders[1].status, DbOrderStatus::Canceled);
        assert!(orders[1].avg_price.is_none());
    }

    #[test]
    fn mock_orders_third_is_btcusdt_partially_filled() {
        let orders = mock_orders();
        assert_eq!(orders[2].symbol, "BTCUSDT");
        assert_eq!(orders[2].side, DbOrderSide::Buy);
        assert_eq!(orders[2].status, DbOrderStatus::PartiallyFilled);
    }

    #[test]
    fn mock_orders_strategy_version_set() {
        for o in mock_orders() {
            assert_eq!(o.strategy_version.as_deref(), Some("v2.1.0"));
        }
    }

    #[test]
    fn mock_orders_reduce_only_false() {
        for o in mock_orders() {
            assert!(!o.reduce_only);
        }
    }

    // ── Unit tests: mock_events() ─────────────────────────────────────────

    #[test]
    fn mock_events_count() {
        assert_eq!(mock_events(Uuid::nil()).len(), 5);
    }

    #[test]
    fn mock_events_ids_sequential() {
        for (i, ev) in mock_events(Uuid::nil()).iter().enumerate() {
            assert_eq!(ev.id, (i + 1) as i64);
        }
    }

    #[test]
    fn mock_events_client_order_id_propagated() {
        let id = Uuid::new_v4();
        for ev in mock_events(id) {
            assert_eq!(ev.client_order_id, id);
        }
    }

    #[test]
    fn mock_events_types_in_order() {
        let evs = mock_events(Uuid::nil());
        let types: Vec<&str> = evs.iter().map(|e| e.event_type.as_str()).collect();
        assert_eq!(
            types,
            ["SUBMITTED", "ACKNOWLEDGED", "PARTIALLY_FILLED", "PARTIALLY_FILLED", "FILLED"]
        );
    }

    #[test]
    fn mock_events_timestamps_non_decreasing() {
        let evs = mock_events(Uuid::nil());
        for i in 0..evs.len() - 1 {
            assert!(
                evs[i].event_time <= evs[i + 1].event_time,
                "event[{i}] timestamp should be <= event[{}]",
                i + 1
            );
        }
    }

    #[test]
    fn mock_events_first_has_submit_payload() {
        let ev = &mock_events(Uuid::nil())[0];
        assert_eq!(ev.event_type, "SUBMITTED");
        assert!(ev.payload.get("action").is_some());
        assert_eq!(ev.payload["action"], "submit");
    }

    #[test]
    fn mock_events_last_has_commission_payload() {
        let evs = mock_events(Uuid::nil());
        let last = evs.last().unwrap();
        assert_eq!(last.event_type, "FILLED");
        assert!(last.payload.get("commission").is_some());
    }

    // ── Unit tests: is_mock() ─────────────────────────────────────────────
    // NOTE: These tests mutate env vars. Run with `--test-threads=1` if
    // flakiness is observed due to parallel env-var access.

    #[test]
    fn is_mock_true_for_value_1() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        assert!(is_mock());
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[test]
    fn is_mock_true_for_value_true() {
        unsafe { std::env::set_var("MOCK_DATA", "true") };
        assert!(is_mock());
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[test]
    fn is_mock_false_for_value_0() {
        unsafe { std::env::set_var("MOCK_DATA", "0") };
        assert!(!is_mock());
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[test]
    fn is_mock_false_for_value_false() {
        unsafe { std::env::set_var("MOCK_DATA", "false") };
        assert!(!is_mock());
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    // ── Integration tests: Axum router in mock mode ───────────────────────
    // These tests set MOCK_DATA=1 so handlers never touch the DB pool.
    // Run with `cargo test -p web -- --test-threads=1` to avoid env-var races.

    #[tokio::test]
    async fn list_orders_returns_200() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn list_orders_returns_three_in_mock_mode() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let orders: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(orders.len(), 3);
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn list_orders_filter_by_symbol_btcusdt() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(
                Request::builder()
                    .uri("/?symbol=BTCUSDT")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let orders: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(orders.len(), 2, "mock has 2 BTCUSDT orders");
        for o in &orders {
            assert_eq!(o["symbol"], "BTCUSDT");
        }
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn list_orders_filter_by_symbol_ethusdt() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(
                Request::builder()
                    .uri("/?symbol=ETHUSDT")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let orders: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(orders.len(), 1, "mock has 1 ETHUSDT order");
        assert_eq!(orders[0]["symbol"], "ETHUSDT");
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn list_orders_filter_by_status_filled() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(
                Request::builder()
                    .uri("/?status=FILLED")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let orders: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(orders.len(), 1);
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn list_orders_filter_by_status_canceled() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(
                Request::builder()
                    .uri("/?status=CANCELED")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let orders: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(orders.len(), 1);
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn list_orders_filter_symbol_and_status() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(
                Request::builder()
                    .uri("/?symbol=ETHUSDT&status=CANCELED")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let orders: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0]["symbol"], "ETHUSDT");
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn list_orders_filter_no_match_returns_empty() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(
                Request::builder()
                    .uri("/?symbol=SOLUSDT")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let orders: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert!(orders.is_empty());
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn get_order_by_id_found_returns_200() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(
                Request::builder()
                    .uri("/00000000-0000-0000-0000-000000000001")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let order: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(order["symbol"], "BTCUSDT");
        assert_eq!(order["account_id"], "main_account");
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn get_order_by_id_second_order() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(
                Request::builder()
                    .uri("/00000000-0000-0000-0000-000000000002")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let order: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(order["symbol"], "ETHUSDT");
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn get_order_by_id_not_found_returns_404() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(
                Request::builder()
                    .uri("/00000000-0000-0000-0000-000000000099")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), axum::http::StatusCode::NOT_FOUND);
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn get_order_events_returns_200_with_five_events() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(
                Request::builder()
                    .uri("/00000000-0000-0000-0000-000000000001/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let events: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(events.len(), 5);
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn get_order_events_first_is_submitted() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(
                Request::builder()
                    .uri("/00000000-0000-0000-0000-000000000001/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let events: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(events[0]["event_type"], "SUBMITTED");
        assert_eq!(events[4]["event_type"], "FILLED");
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn get_order_events_for_unknown_id_returns_mock_events() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        // In mock mode any UUID returns events (mock_events ignores whether the order exists)
        let resp = router(lazy_pool())
            .oneshot(
                Request::builder()
                    .uri("/00000000-0000-0000-0000-000000000099/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let events: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(events.len(), 5);
        unsafe { std::env::remove_var("MOCK_DATA") };
    }

    #[tokio::test]
    async fn list_orders_response_has_expected_json_fields() {
        unsafe { std::env::set_var("MOCK_DATA", "1") };
        let resp = router(lazy_pool())
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let orders: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        let first = &orders[0];

        assert!(first.get("id").is_some());
        assert!(first.get("client_order_id").is_some());
        assert!(first.get("account_id").is_some());
        assert!(first.get("symbol").is_some());
        assert!(first.get("side").is_some());
        assert!(first.get("status").is_some());
        assert!(first.get("qty").is_some());
        assert!(first.get("filled_qty").is_some());
        assert!(first.get("created_at").is_some());
        unsafe { std::env::remove_var("MOCK_DATA") };
    }
}
