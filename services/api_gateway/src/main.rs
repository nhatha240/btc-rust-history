use anyhow::{Context, Result};
use axum::Router;
use hft_redis::{KillSwitch, RedisStore};
use hft_store::pg::create_pool;
use routes::risk::RiskState;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

mod config;
mod routes;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting API Gateway service...");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://trader:traderpw@localhost:5432/db_trading".to_string());
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://redis:6379/0".to_string());
    let listen_addr = std::env::var("LISTEN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string());

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
        .nest("/api/risk",      routes::risk::router(risk_state));

    let addr: SocketAddr = listen_addr.parse().context("Invalid listen address")?;
    info!("API Gateway listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
