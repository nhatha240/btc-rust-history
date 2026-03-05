use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use hft_store::pg::models::TradeRow;
use hft_store::repos::list_trades;
use serde::Deserialize;
use sqlx::{Pool, Postgres};

#[derive(Deserialize)]
pub struct TradeParams {
    pub symbol: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn router(pool: Pool<Postgres>) -> Router {
    Router::new()
        .route("/", get(handle_list_trades))
        .with_state(pool)
}

async fn handle_list_trades(
    State(pool): State<Pool<Postgres>>,
    Query(params): Query<TradeParams>,
) -> Json<Vec<TradeRow>> {
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);
    
    match list_trades(&pool, params.symbol, limit, offset).await {
        Ok(trades) => Json(trades),
        Err(e) => {
            tracing::error!("Failed to list trades: {}", e);
            Json(vec![])
        }
    }
}
