use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use hft_store::repos::list_positions;
use serde::Serialize;
use sqlx::{Pool, Postgres};
use rust_decimal::Decimal;

#[derive(Serialize)]
pub struct PnlSummary {
    pub total_realized_pnl: Decimal,
    pub total_unrealized_pnl: Decimal,
}

#[derive(Clone)]
pub struct PnlState {
    pub pool: Pool<Postgres>,
}

pub fn router(pool: Pool<Postgres>) -> Router {
    Router::new()
        .route("/", get(handle_get_pnl))
        .with_state(pool)
}

async fn handle_get_pnl(
    State(pool): State<Pool<Postgres>>,
) -> Json<PnlSummary> {
    // Simple PNL calculation by aggregating positions
    match list_positions(&pool, None).await {
        Ok(positions) => {
            let total_realized = positions.iter().map(|p| p.realized_pnl).sum();
            let total_unrealized = positions.iter().map(|p| p.unrealized_pnl).sum();
            Json(PnlSummary {
                total_realized_pnl: total_realized,
                total_unrealized_pnl: total_unrealized,
            })
        },
        Err(e) => {
            tracing::error!("Failed to calculate PNL: {}", e);
            Json(PnlSummary {
                total_realized_pnl: Decimal::ZERO,
                total_unrealized_pnl: Decimal::ZERO,
            })
        }
    }
}
