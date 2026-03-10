use axum::{extract::{Query, State}, Json, response::IntoResponse};
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use hft_store::repos::logs_repo;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct LogParams {
    pub service: Option<String>,
    pub severity: Option<String>,
    pub strategy_id: Option<String>,
    pub symbol: Option<String>,
    pub account_id: Option<String>,
    pub event_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Clone)]
pub struct LogsState {
    pub pool: Pool<Postgres>,
}

pub async fn handle_system_logs(
    State(pool): State<Pool<Postgres>>,
    Query(params): Query<LogParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    match logs_repo::list_system_logs(&pool, params.service.as_deref(), params.severity.as_deref(), limit, offset).await {
        Ok(logs) => Json(serde_json::json!(logs)).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch system logs: {}", e);
            Json(serde_json::json!({ "error": "Internal server error" })).into_response()
        }
    }
}

pub async fn handle_strategy_logs(
    State(pool): State<Pool<Postgres>>,
    Query(params): Query<LogParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    match logs_repo::list_strategy_logs(&pool, params.strategy_id.as_deref(), params.symbol.as_deref(), limit, offset).await {
        Ok(logs) => Json(serde_json::json!(logs)).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch strategy logs: {}", e);
            Json(serde_json::json!({ "error": "Internal server error" })).into_response()
        }
    }
}

pub async fn handle_risk_logs(
    State(pool): State<Pool<Postgres>>,
    Query(params): Query<LogParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    match logs_repo::list_risk_events(&pool, params.account_id.as_deref(), params.event_type.as_deref(), limit, offset).await {
        Ok(logs) => Json(serde_json::json!(logs)).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch risk logs: {}", e);
            Json(serde_json::json!({ "error": "Internal server error" })).into_response()
        }
    }
}

pub async fn handle_audit_logs(
    State(pool): State<Pool<Postgres>>,
    Query(params): Query<LogParams>,
) -> impl IntoResponse {
    // For now audit trail focuses on strategy config changes
    // If a global audit_trail table is added later, this can be expanded.
    let strategy_uuid = match params.strategy_id {
        Some(sid) => match Uuid::parse_str(&sid) {
            Ok(u) => Some(u),
            Err(_) => return Json(serde_json::json!({ "error": "Invalid strategy_id UUID" })).into_response(),
        },
        None => None,
    };

    if let Some(id) = strategy_uuid {
        match hft_store::repos::strategies_repo::list_strategy_audit_logs(&pool, id).await {
            Ok(logs) => Json(serde_json::json!(logs)).into_response(),
            Err(e) => {
                tracing::error!("Failed to fetch audit logs: {}", e);
                Json(serde_json::json!({ "error": "Internal server error" })).into_response()
            }
        }
    } else {
         // Return recent config changes across all strategies
         match sqlx::query_as::<_, hft_store::pg::models::StratConfigAudit>(
             "SELECT * FROM strat_config_audit ORDER BY created_at DESC LIMIT $1"
         )
         .bind(params.limit.unwrap_or(50))
         .fetch_all(&pool)
         .await {
             Ok(logs) => Json(serde_json::json!(logs)).into_response(),
             Err(e) => {
                 tracing::error!("Failed to fetch global audit logs: {}", e);
                 Json(serde_json::json!({ "error": "Internal server error" })).into_response()
             }
         }
    }
}
