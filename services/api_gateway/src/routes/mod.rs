pub mod orders;
pub mod trades;
pub mod positions;
pub mod pnl;
pub mod risk;
pub mod verification;
pub mod strategies;
pub mod signals;
pub mod md;
pub mod logs;

use axum::{routing::get, Router};
use sqlx::{Pool, Postgres};

pub fn logs_router(pool: Pool<Postgres>) -> Router {
    Router::new()
        .route("/system", get(logs::handle_system_logs))
        .route("/strategy", get(logs::handle_strategy_logs))
        .route("/risk", get(logs::handle_risk_logs))
        .route("/audit", get(logs::handle_audit_logs))
        .with_state(pool)
}
