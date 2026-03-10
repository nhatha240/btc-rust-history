use anyhow::{Context, Result};
use axum::Router;
use hft_redis::{KillSwitch, RedisStore};
use hft_store::pg::create_pool;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing::{info, error};
use clickhouse::Client;

mod routes;
mod mq;
mod openapi;

use openapi::{create_app, api_spec};
use routes::orders::OrderState;
use routes::positions::PositionState;
use routes::strategies::StrategyState;
use routes::risk::RiskState;
use routes::trades::TradeState;
use routes::pnl::PnlState;
use routes::logs::LogsState;
use routes::verification::VerificationState;
use routes::md::MdState;

fn is_mock() -> bool {
    std::env::var("MOCK_DATA").map(|v| v == "1" || v == "true").unwrap_or(false)
}

#[tokio::main]
async fn main() -> Result<()> {
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
        let kill_switch = Arc::new(Mutex::new(KillSwitch::new(redis_store.clone())));

        let risk_state = RiskState { pool: pool.clone(), kill_switch };

        // Mock ClickHouse client
        let _mock_ch_client = Client::default().with_url("http://mock:8123");

        let control_mq = Arc::new(mq::ControlProducer::new());
        let strat_state = routes::strategies::StrategyState {
            pool: pool.clone(),
            control_mq,
        };

        let order_mq = Arc::new(mq::OrderProducer::new());
        let pos_state = routes::positions::PositionState {
            pool: pool.clone(),
            order_mq: order_mq.clone(),
        };

        let order_state = routes::orders::OrderState {
            pool: pool.clone(),
            order_mq: order_mq.clone(),
        };

        let broadcaster = Arc::new(routes::md::MdBroadcaster::new(
            hft_mq::KafkaConfig::low_latency("localhost:9092", "api-gateway-mock")
        ));

        let md_state = routes::md::MdState {
            redis: redis_store.clone(),
            broadcaster,
        };

        let trade_state = TradeState { pool: pool.clone() };
        let pnl_state = PnlState { pool: pool.clone() };
        let logs_state = LogsState { pool: pool.clone() };
        let verification_state = VerificationState { pool: pool.clone() };

        let app = create_app(
            order_state,
            pos_state,
            strat_state,
            risk_state,
            trade_state,
            pnl_state,
            logs_state,
            verification_state,
            md_state,
        );

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
    let ch_url = std::env::var("CLICKHOUSE_HTTP_URL")
        .unwrap_or_else(|_| "http://clickhouse:8123".to_string());

    let pool = create_pool(&database_url, 10)
        .await
        .context("Failed to connect to PostgreSQL")?;

    let redis_store = RedisStore::new(&redis_url)
        .await
        .context("Failed to connect to Redis")?;
    let kill_switch = Arc::new(Mutex::new(KillSwitch::new(redis_store.clone())));

    let risk_state = RiskState {
        pool: pool.clone(),
        kill_switch,
    };

    // Create ClickHouse client
    let _ch_client = Client::default()
        .with_url(&ch_url)
        .with_database("db_trading");

    let control_mq = Arc::new(mq::ControlProducer::new());
    let strat_state = routes::strategies::StrategyState {
        pool: pool.clone(),
        control_mq,
    };

    let order_mq = Arc::new(mq::OrderProducer::new());
    let pos_state = routes::positions::PositionState {
        pool: pool.clone(),
        order_mq: order_mq.clone(),
    };

    let order_state = routes::orders::OrderState {
        pool: pool.clone(),
        order_mq: order_mq.clone(),
    };

    let kafka_cfg = hft_mq::KafkaConfig::from_env().unwrap_or_else(|_| {
        hft_mq::KafkaConfig::low_latency(
            std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string()),
            "api-gateway"
        )
    });

    let broadcaster = Arc::new(routes::md::MdBroadcaster::new(kafka_cfg));
    let b_clone = broadcaster.clone();
    tokio::spawn(async move {
        if let Err(e) = b_clone.run().await {
            error!("market data broadcaster failed: {e:#}");
        }
    });

    let md_state = routes::md::MdState {
        redis: redis_store.clone(),
        broadcaster,
    };

    let trade_state = TradeState { pool: pool.clone() };
    let pnl_state = PnlState { pool: pool.clone() };
    let logs_state = LogsState { pool: pool.clone() };
    let verification_state = VerificationState { pool: pool.clone() };

    let app = create_app(
        order_state,
        pos_state,
        strat_state,
        risk_state,
        trade_state,
        pnl_state,
        logs_state,
        verification_state,
        md_state,
    );

    let addr: SocketAddr = listen_addr.parse().context("Invalid listen address")?;
    info!("API Gateway listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
