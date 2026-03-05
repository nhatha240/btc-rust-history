use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use hft_store::pg::models::PositionRow;
use hft_store::repos::list_positions;
use serde::Deserialize;
use sqlx::{Pool, Postgres};

#[derive(Deserialize)]
pub struct PositionParams {
    pub account_id: Option<String>,
}

pub fn router(pool: Pool<Postgres>) -> Router {
    Router::new()
        .route("/", get(handle_list_positions))
        .with_state(pool)
}

async fn handle_list_positions(
    State(pool): State<Pool<Postgres>>,
    Query(params): Query<PositionParams>,
) -> Json<Vec<PositionRow>> {
    match list_positions(&pool, params.account_id).await {
        Ok(positions) => Json(positions),
        Err(e) => {
            tracing::error!("Failed to list positions: {}", e);
            Json(vec![])
        }
    }
}
