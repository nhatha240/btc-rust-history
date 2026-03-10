use axum::Router;
use axum_openapi::{OpenApi, OpenApiHandlerExt};
use std::sync::Arc;

// Import all route modules
mod routes;
use routes::orders::OrderState;
use routes::positions::PositionState;
use routes::strategies::StrategyState;
use routes::risk::RiskState;
use routes::trades::TradeState;
use routes::pnl::PnlState;
use routes::logs::LogsState;
use routes::verification::VerificationState;
use routes::md::MdState;

// Import all route handlers
use routes::orders::router as orders_router;
use routes::positions::router as positions_router;
use routes::strategies::router as strategies_router;
use routes::risk::router as risk_router;
use routes::trades::router as trades_router;
use routes::pnl::router as pnl_router;
use routes::logs::router as logs_router;
use routes::verification::router as verification_router;
use routes::md::router as md_router;

// Define the OpenAPI spec
pub fn api_spec() -> OpenApi {
    OpenApi::new("BTC Rust Backend API", "1.0.0")
        .info(|info| {
            info.description("API Gateway for BTC Rust Backend trading system")
                .contact(|c| {
                    c.name("Trading System Team")
                        .url("https://github.com/nhatha240/btc-rust-history")
                        .email("team@trading.system")
                })
        })
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
        .nest("/api/trades", trades_router(trade_state))
        .nest("/api/positions", positions_router(position_state))
        .nest("/api/pnl", pnl_router(pnl_state))
        .nest("/api/risk", risk_router(risk_state))
        .nest("/api/verification", verification_router(verification_state))
        .nest("/api/signals", routes::signals::router())
        .nest("/api/strategies", strategies_router(strategy_state))
        .nest("/api/md", md_router(md_state))
        .nest("/api/logs", logs_router(logs_state));

    // Combine with OpenAPI documentation
    base_router
        .openapi(api_spec())
        .nest("/api-docs", axum::Router::new().get(axum::routing::get(|| async { axum::response::Html("<h1>API Documentation</h1><p>Visit <a href=\"/openapi.json\">/openapi.json</a> for OpenAPI spec</p>") })))
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