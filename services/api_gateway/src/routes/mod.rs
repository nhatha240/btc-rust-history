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

// Re-export state types for OpenAPI
pub use orders::OrderState;
pub use trades::TradeState;
pub use positions::PositionState;
pub use pnl::PnlState;
pub use risk::RiskState;
pub use verification::VerificationState;
pub use logs::LogsState;
pub use strategies::StrategyState;
pub use md::MdState;
