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
    pub status: DbOrderStatus,
    pub filled_qty: Decimal,
    pub avg_price: Option<Decimal>,
    pub reduce_only: bool,
    pub trace_id: Option<Uuid>,
    pub strategy_version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
