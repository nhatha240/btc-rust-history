use anyhow::{Context, Result};
use axum::Router;
use hft_redis::{KillSwitch, RedisStore};
use hft_store::pg::create_pool;
use routes::risk::RiskState;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing::info;

mod routes;

fn is_mock() -> bool {
    std::env::var("MOCK_DATA").map(|v| v == "1" || v == "true").unwrap_or(false)
}

#[tokio::main]
async fn main() -> Result<()> {
    // robust rustls init
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting API Gateway service...");

    let listen_addr = std::env::var("LISTEN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:3001".to_string());

    // ── Mock mode ─────────────────────────────────────────────────────────────
    if is_mock() {
        info!("⚠️  MOCK_DATA=1 — running with in-memory fixtures, no DB/Redis required");

        // Create a fake pool URL just to satisfy the type; handlers won't use it.
        let fake_url = "postgres://mock:mock@localhost:5432/mock";
        let pool = create_pool(fake_url, 1).await.unwrap_or_else(|_| {
            // If even the fake pool fails, we still need a placeholder.
            // Handlers guard against real DB calls with is_mock().
            panic!("Cannot create pool placeholder for mock mode. Set DATABASE_URL to any valid URL or leave as default.");
        });

        // Dummy kill switch so RiskState compiles.
        let dummy_redis = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379/0".to_string());
        let redis_store = RedisStore::new(&dummy_redis).await.unwrap_or_else(|_| {
            panic!("Cannot create Redis placeholder for mock mode.");
        });
        let kill_switch = Arc::new(Mutex::new(KillSwitch::new(redis_store)));

        let risk_state = RiskState { pool: pool.clone(), kill_switch };

        let app = Router::new()
            .nest("/api/orders",    routes::orders::router(pool.clone()))
            .nest("/api/trades",    routes::trades::router(pool.clone()))
            .nest("/api/positions", routes::positions::router(pool.clone()))
            .nest("/api/pnl",       routes::pnl::router(pool.clone()))
            .nest("/api/risk",      routes::risk::router(risk_state))
            .layer(CorsLayer::permissive());

        let addr: SocketAddr = listen_addr.parse().context("Invalid listen address")?;
        info!("API Gateway (MOCK) listening on http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        return Ok(());
    }

    // ── Normal mode ───────────────────────────────────────────────────────────
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://trader:traderpw@localhost:5432/db_trading".to_string());
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://redis:6379/0".to_string());

    let pool = create_pool(&database_url, 10)
        .await
        .context("Failed to connect to PostgreSQL")?;

    let redis_store = RedisStore::new(&redis_url)
        .await
        .context("Failed to connect to Redis")?;
    let kill_switch = Arc::new(Mutex::new(KillSwitch::new(redis_store)));

    let risk_state = RiskState {
        pool: pool.clone(),
        kill_switch,
    };

    let app = Router::new()
        .nest("/api/orders",    routes::orders::router(pool.clone()))
        .nest("/api/trades",    routes::trades::router(pool.clone()))
        .nest("/api/positions", routes::positions::router(pool.clone()))
        .nest("/api/pnl",       routes::pnl::router(pool.clone()))
        .nest("/api/risk",      routes::risk::router(risk_state))
        .layer(CorsLayer::permissive());

    let addr: SocketAddr = listen_addr.parse().context("Invalid listen address")?;
    info!("API Gateway listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
