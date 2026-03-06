//! `/api/risk` — Kill-switch control + rejection audit log.
//!
//! Routes:
//!   GET  /api/risk/status          — kill-switch state + 24 h rejection summary
//!   POST /api/risk/kill-switch      — activate global kill switch
//!   DELETE /api/risk/kill-switch    — deactivate global kill switch
//!   GET  /api/risk/rejections       — paginated rejection log

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use hft_redis::KillSwitch;
use hft_store::repos::{list_risk_rejections, rejection_summary, RejectionSummary};
use hft_store::pg::models::RiskRejectionRow;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;

// ── Shared state ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct RiskState {
    pub pool: Pool<Postgres>,
    /// Wrapped in Arc<Mutex> because KillSwitch holds a mutable ConnectionManager.
    pub kill_switch: Arc<Mutex<KillSwitch>>,
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router(state: RiskState) -> Router {
    Router::new()
        .route("/status",      get(handle_status))
        .route("/kill-switch", post(handle_enable_kill_switch))
        .route("/kill-switch", delete(handle_disable_kill_switch))
        .route("/rejections",  get(handle_list_rejections))
        .with_state(state)
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct RiskStatusResponse {
    pub kill_switch_active: bool,
    pub rejection_summary: Vec<RejectionSummary>,
}

#[derive(Serialize)]
pub struct ApiError {
    pub error: String,
}

fn internal(msg: impl std::fmt::Display) -> (StatusCode, Json<ApiError>) {
    error!("{msg}");
    (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { error: msg.to_string() }))
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /api/risk/status
/// Returns global kill-switch state and 24-hour rejection counts by reason.
async fn handle_status(
    State(state): State<RiskState>,
) -> Result<Json<RiskStatusResponse>, (StatusCode, Json<ApiError>)> {
    let kill_switch_active = {
        let mut ks = state.kill_switch.lock().await;
        ks.check_global().await.unwrap_or(false)
    };

    let summary = rejection_summary(&state.pool, 24)
        .await
        .map_err(|e| internal(format!("DB error: {e}")))?;

    Ok(Json(RiskStatusResponse {
        kill_switch_active,
        rejection_summary: summary,
    }))
}

/// POST /api/risk/kill-switch  — activate global kill switch
async fn handle_enable_kill_switch(
    State(state): State<RiskState>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let mut ks = state.kill_switch.lock().await;
    ks.enable()
        .await
        .map_err(|e| internal(format!("Redis error enabling kill switch: {e}")))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/risk/kill-switch  — deactivate global kill switch
async fn handle_disable_kill_switch(
    State(state): State<RiskState>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let mut ks = state.kill_switch.lock().await;
    ks.disable()
        .await
        .map_err(|e| internal(format!("Redis error disabling kill switch: {e}")))?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/risk/rejections?symbol=&reason=&account_id=&limit=&offset=
#[derive(Deserialize)]
pub struct RejectionParams {
    pub symbol:     Option<String>,
    pub reason:     Option<String>,
    pub account_id: Option<String>,
    pub limit:      Option<i64>,
    pub offset:     Option<i64>,
}

async fn handle_list_rejections(
    State(state): State<RiskState>,
    Query(params): Query<RejectionParams>,
) -> Result<Json<Vec<RiskRejectionRow>>, (StatusCode, Json<ApiError>)> {
    let limit  = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let rows = list_risk_rejections(
        &state.pool,
        params.symbol.as_deref(),
        params.reason.as_deref(),
        params.account_id.as_deref(),
        limit,
        offset,
    )
    .await
    .map_err(|e| internal(format!("DB error: {e}")))?;

    Ok(Json(rows))
}
