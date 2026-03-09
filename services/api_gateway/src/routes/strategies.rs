use crate::mq::ControlProducer;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, patch, post},
    Json, Router,
};
use hft_store::pg::models::{StratConfigAudit, StratDefinition, StratInstance};
use hft_store::pg::types::DbStratStatus;
use hft_store::repos::{
    get_strategy_by_id, list_strategies, list_strategy_audit_logs, list_strategy_instances,
    update_strategy_config, update_strategy_status,
};
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use uuid::Uuid;

fn is_mock() -> bool {
    std::env::var("MOCK_DATA").map(|v| v == "1" || v == "true").unwrap_or(false)
}

#[derive(Clone)]
pub struct StrategyState {
    pub pool: Pool<Postgres>,
    pub control_mq: Arc<ControlProducer>,
}

pub fn router(state: StrategyState) -> Router {
    Router::new()
        .route("/", get(handle_list_strategies))
        .route("/:id", get(handle_get_strategy))
        .route("/:id/action", post(handle_strategy_action))
        .route("/:id/config", patch(handle_update_config))
        .route("/:id/instances", get(handle_list_instances))
        .route("/:id/audit", get(handle_list_audit))
        .with_state(state)
}

#[derive(Deserialize)]
pub struct ActionRequest {
    pub action: String, // START, STOP, PAUSE, RESUME, EMERGENCY_STOP
}

#[derive(Deserialize)]
pub struct ConfigUpdateRequest {
    pub config: serde_json::Value,
    pub changed_by: String,
    pub reason: Option<String>,
}

async fn handle_list_strategies(State(state): State<StrategyState>) -> Json<Vec<StratDefinition>> {
    if is_mock() {
        return Json(vec![]); 
    }

    match list_strategies(&state.pool).await {
        Ok(strats) => Json(strats),
        Err(e) => {
            tracing::error!("Failed to list strategies: {}", e);
            Json(vec![])
        }
    }
}

async fn handle_get_strategy(
    State(state): State<StrategyState>,
    Path(id): Path<Uuid>,
) -> Result<Json<StratDefinition>, StatusCode> {
    if is_mock() {
        return Err(StatusCode::NOT_FOUND);
    }

    match get_strategy_by_id(&state.pool, id).await {
        Ok(Some(strat)) => Ok(Json(strat)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get strategy {}: {}", id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn handle_strategy_action(
    State(state): State<StrategyState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ActionRequest>,
) -> Result<StatusCode, StatusCode> {
    if is_mock() {
        return Ok(StatusCode::OK);
    }

    let action_name = req.action.to_uppercase();
    let status = match action_name.as_str() {
        "START" | "RESUME" => DbStratStatus::Running,
        "STOP" | "HALTED" => DbStratStatus::Halted,
        "PAUSE" => DbStratStatus::Paused,
        "EMERGENCY_STOP" => DbStratStatus::Halted, 
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    match update_strategy_status(&state.pool, id, status).await {
        Ok(_) => {
            // Publish to Kafka control topic
            let signal_action = match action_name.as_str() {
                "START" | "RESUME" => "START",
                "PAUSE" => "PAUSE",
                _ => "STOP",
            };
            
            if let Err(e) = state.control_mq.publish_update(id, signal_action).await {
                tracing::error!("Failed to publish status update signal for {}: {}", id, e);
            }

            Ok(StatusCode::OK)
        }
        Err(e) => {
            tracing::error!("Failed to update strategy {} status to {:?}: {}", id, status, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn handle_update_config(
    State(state): State<StrategyState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ConfigUpdateRequest>,
) -> Result<StatusCode, StatusCode> {
    if is_mock() {
        return Ok(StatusCode::OK);
    }

    match update_strategy_config(&state.pool, id, req.config, req.changed_by, req.reason).await {
        Ok(_) => {
            if let Err(e) = state.control_mq.publish_update(id, "RELOAD_CONFIG").await {
                tracing::error!("Failed to publish config update signal for {}: {}", id, e);
            }
            Ok(StatusCode::OK)
        }
        Err(e) => {
            tracing::error!("Failed to update configuration for strategy {}: {}", id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn handle_list_instances(
    State(state): State<StrategyState>,
    Path(id): Path<Uuid>,
) -> Json<Vec<StratInstance>> {
    if is_mock() {
        return Json(vec![]);
    }

    match list_strategy_instances(&state.pool, id).await {
        Ok(instances) => Json(instances),
        Err(e) => {
            tracing::error!("Failed to list instances for strategy {}: {}", id, e);
            Json(vec![])
        }
    }
}

async fn handle_list_audit(
    State(state): State<StrategyState>,
    Path(id): Path<Uuid>,
) -> Json<Vec<StratConfigAudit>> {
    if is_mock() {
        return Json(vec![]);
    }

    match list_strategy_audit_logs(&state.pool, id).await {
        Ok(logs) => Json(logs),
        Err(e) => {
            tracing::error!("Failed to list audit logs for strategy {}: {}", id, e);
            Json(vec![])
        }
    }
}
