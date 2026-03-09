use crate::pg::types::*;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrderRow {
    pub id: i64,
    pub client_order_id: Uuid,
    pub exchange_order_id: Option<i64>,
    pub account_id: String,
    pub symbol: String,
    pub side: DbOrderSide,
    pub r#type: DbOrderType,
    pub tif: DbTimeInForce,
    pub qty: Decimal,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub take_profit_price: Option<Decimal>,
    pub coin_tags: serde_json::Value,
    pub status: DbOrderStatus,
    pub filled_qty: Decimal,
    pub avg_price: Option<Decimal>,
    pub reduce_only: bool,
    pub trace_id: Option<Uuid>,
    pub strategy_version: Option<String>,
    pub ack_at: Option<DateTime<Utc>>,
    pub done_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct OrderExitTagSnapshot {
    pub id: i64,
    pub client_order_id: Uuid,
    pub account_id: String,
    pub symbol: String,
    pub exit_trigger: String,
    pub exit_price: Decimal,
    pub coin_tags: serde_json::Value,
    pub trace_id: Option<Uuid>,
    pub event_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct OrderTrainingEventRow {
    pub id: i64,
    pub client_order_id: Uuid,
    pub account_id: String,
    pub symbol: String,
    pub side: String,
    pub order_status: String,
    pub execution_mode: String,
    pub exchange: String,
    pub strategy_id: Option<String>,
    pub signal_id: Option<String>,
    pub exit_kind: Option<String>,
    pub outcome_label: String,
    pub outcome_reason: Option<String>,
    pub coin_tags: serde_json::Value,
    pub decision_meta: serde_json::Value,
    pub filled_qty: Option<Decimal>,
    pub fill_price: Option<Decimal>,
    pub trace_id: Option<Uuid>,
    pub event_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TradeRow {
    pub id: i64,
    pub trade_id: i64,
    pub order_id: i64,
    pub client_order_id: Uuid,
    pub account_id: String,
    pub symbol: String,
    pub side: DbOrderSide,
    pub qty: Decimal,
    pub price: Decimal,
    pub quote_qty: Decimal,
    pub commission: Decimal,
    pub commission_asset: Option<String>,
    pub realized_pnl: Option<Decimal>,
    pub is_maker: bool,
    pub trade_time: DateTime<Utc>,
    pub recv_time: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PositionRow {
    pub id: i64,
    pub account_id: String,
    pub symbol: String,
    pub side: DbPositionSide,
    pub qty: Decimal,
    pub entry_price: Option<Decimal>,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub leverage: i32,
    pub margin_type: String,
    pub liquidation_price: Option<Decimal>,
    pub snapshot_time: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct RiskRejectionRow {
    pub id: i64,
    pub client_order_id: String,
    pub account_id: String,
    pub symbol: String,
    pub qty: Decimal,
    pub price: Decimal,
    pub notional: Decimal,
    /// Normalised code from hft_risk::RejectReason (e.g. "KILL_SWITCH")
    pub reject_reason: String,
    /// Human-readable detail string from the checker
    pub reject_detail: String,
    pub trace_id: Option<String>,
    pub rejected_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct EventRow {
    pub id: i64,
    pub client_order_id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub event_time: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct StratDefinition {
    pub id: Uuid,
    pub strategy_name: String,
    pub version: String,
    pub status: DbStratStatus,
    pub mode: DbStratMode,
    pub config_json: serde_json::Value,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct StratInstance {
    pub id: Uuid,
    pub strategy_id: Uuid,
    pub instance_id: String,
    pub last_heartbeat: DateTime<Utc>,
    pub is_active: bool,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct StratConfigAudit {
    pub id: i32,
    pub strategy_id: Uuid,
    pub changed_by: String,
    pub change_reason: Option<String>,
    pub old_config: serde_json::Value,
    pub new_config: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct ErrorLogRow {
    pub error_id: i64,
    pub service_name: String,
    pub severity: String,
    pub error_type: Option<String>,
    pub message: String,
    pub stack_trace: Option<String>,
    pub context_json: serde_json::Value,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
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

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct RiskEventRow {
    pub event_id: i64,
    pub account_id: String,
    pub symbol: Option<String>,
    pub event_type: String,
    pub decision: String,
    pub reason: Option<String>,
    pub original_order_json: Option<serde_json::Value>,
    pub modified_order_json: Option<serde_json::Value>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
