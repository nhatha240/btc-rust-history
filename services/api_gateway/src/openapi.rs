use axum::{
    response::Html,
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};

use crate::routes;
use crate::routes::logs_router;
use crate::routes::md::router as md_router;
use crate::routes::orders::router as orders_router;
use crate::routes::pnl::router as pnl_router;
use crate::routes::positions::router as positions_router;
use crate::routes::risk::router as risk_router;
use crate::routes::strategies::router as strategies_router;
use crate::routes::trades::router as trades_router;
use crate::routes::verification::router as verification_router;
use crate::routes::{
    LogsState, MdState, OrderState, PnlState, PositionState, RiskState, StrategyState, TradeState,
    VerificationState,
};

pub fn api_spec() -> Value {
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "BTC Rust Backend API",
            "version": "1.0.0",
            "description": "API Gateway for BTC Rust Backend trading system",
            "contact": {
                "name": "Trading System Team",
                "url": "https://github.com/nhatha240/btc-rust-history",
                "email": "team@trading.system"
            }
        },
        "paths": {}
    })
}

async fn openapi_json() -> Json<Value> {
    Json(api_spec())
}

async fn api_docs_html() -> Html<&'static str> {
    Html("<h1>API Documentation</h1><p>Visit <a href=\"/openapi.json\">/openapi.json</a> for OpenAPI spec</p>")
}

pub fn router(
    order_state: OrderState,
    position_state: PositionState,
    strategy_state: StrategyState,
    risk_state: RiskState,
    trade_state: TradeState,
    pnl_state: PnlState,
    logs_state: LogsState,
    verification_state: VerificationState,
    md_state: MdState,
) -> Router {
    // Create the base router with all routes
    let base_router = Router::new()
        .nest("/api/orders", orders_router(order_state))
        .nest("/api/trades", trades_router(trade_state.pool))
        .nest("/api/positions", positions_router(position_state))
        .nest("/api/pnl", pnl_router(pnl_state.pool))
        .nest("/api/risk", risk_router(risk_state))
        .nest("/api/verification", verification_router(verification_state.pool))
        .nest("/api/signals", routes::signals::router())
        .nest("/api/strategies", strategies_router(strategy_state))
        .nest("/api/md", md_router(md_state))
        .nest("/api/logs", logs_router(logs_state.pool));

    // Combine with OpenAPI documentation
    base_router
        .route("/openapi.json", get(openapi_json))
        .route("/api-docs", get(api_docs_html))
}

// Create a convenience function for the main application
pub fn create_app(
    order_state: OrderState,
    position_state: PositionState,
    strategy_state: StrategyState,
    risk_state: RiskState,
    trade_state: TradeState,
    pnl_state: PnlState,
    logs_state: LogsState,
    verification_state: VerificationState,
    md_state: MdState,
) -> Router {
    router(
        order_state,
        position_state,
        strategy_state,
        risk_state,
        trade_state,
        pnl_state,
        logs_state,
        verification_state,
        md_state,
    )
}
