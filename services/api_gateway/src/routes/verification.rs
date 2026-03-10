use axum::{
    extract::{State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct RiskEventRow {
    pub id: i64,
    pub event_time: DateTime<Utc>,
    pub check_type: String,
    pub scope_type: String,
    pub scope_ref: String,
    pub severity: String,
    pub pass_flag: bool,
    pub current_value: Option<Decimal>,
    pub limit_value: Option<Decimal>,
    pub action_taken: Option<String>,
    pub related_order_id: Option<String>,
    pub trace_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct StratLogRow {
    pub id: i64,
    pub strategy_version_id: String,
    pub symbol: String,
    pub event_time: DateTime<Utc>,
    pub log_level: String,
    pub event_code: String,
    pub message: Option<String>,
    pub context_json: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct StratHealthRow {
    pub id: i64,
    pub instance_id: String,
    pub strategy_name: String,
    pub reported_at: DateTime<Utc>,
    pub cpu_pct: Option<Decimal>,
    pub mem_mb: Option<Decimal>,
    pub queue_lag_ms: Option<i32>,
    pub last_market_ts: Option<DateTime<Utc>>,
    pub last_signal_ts: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct VerificationState {
    pub pool: Pool<Postgres>,
}

pub fn router(pool: Pool<Postgres>) -> Router {
    Router::new()
        .route("/risk_events", get(handle_list_risk_events))
        .route("/strat_logs", get(handle_list_strat_logs))
        .route("/strat_health", get(handle_list_strat_health))
        .with_state(pool)
}

async fn handle_list_risk_events(
    State(pool): State<Pool<Postgres>>,
) -> Json<Vec<RiskEventRow>> {
    let res = sqlx::query_as::<_, RiskEventRow>(
        "SELECT * FROM risk_events ORDER BY event_time DESC LIMIT 100"
    )
    .fetch_all(&pool)
    .await;

    match res {
        Ok(events) => Json(events),
        Err(e) => {
            tracing::error!("Failed to list risk events: {}", e);
            Json(vec![])
        }
    }
}

async fn handle_list_strat_logs(
    State(pool): State<Pool<Postgres>>,
) -> Json<Vec<StratLogRow>> {
    let res = sqlx::query_as::<_, StratLogRow>(
        "SELECT * FROM strat_logs ORDER BY event_time DESC LIMIT 100"
    )
    .fetch_all(&pool)
    .await;

    match res {
        Ok(logs) => Json(logs),
        Err(e) => {
            tracing::error!("Failed to list strat logs: {}", e);
            Json(vec![])
        }
    }
}

async fn handle_list_strat_health(
    State(pool): State<Pool<Postgres>>,
) -> Json<Vec<StratHealthRow>> {
    let res = sqlx::query_as::<_, StratHealthRow>(
        "SELECT * FROM strat_health ORDER BY reported_at DESC"
    )
    .fetch_all(&pool)
    .await;

    match res {
        Ok(health) => Json(health),
        Err(e) => {
            tracing::error!("Failed to list strat health: {}", e);
            Json(vec![])
        }
    }
}
