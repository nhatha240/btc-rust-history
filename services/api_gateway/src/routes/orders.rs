use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use hft_store::pg::models::OrderRow;
use hft_store::repos::{list_orders, get_order_by_id};
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

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
        .with_state(pool)
}

async fn handle_list_orders(
    State(pool): State<Pool<Postgres>>,
    Query(params): Query<OrderParams>,
) -> Json<Vec<OrderRow>> {
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
) -> Json<Option<OrderRow>> {
    match get_order_by_id(&pool, id).await {
        Ok(order) => Json(order),
        Err(e) => {
            tracing::error!("Failed to get order {}: {}", id, e);
            Json(None)
        }
    }
}
