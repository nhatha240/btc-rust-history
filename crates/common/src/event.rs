pub const TOPIC_CANDLES_1M: &str = "md.candles.1m.v1";
pub const TOPIC_FEATURE_STATE: &str = "features.state.v1";
pub const TOPIC_SIGNAL_DECISION: &str = "signals.decision.v1";

pub fn key_symbol(exchange: &str, symbol: &str) -> String {
    format!("{exchange}|{symbol}")
}
pub fn key_state(exchange: &str, symbol: &str, interval: &str) -> String {
    format!("{exchange}|{symbol}|{interval}")
}
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side { BUY, SELL }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderType { MARKET, LIMIT }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderApprovedEvent {
    pub order_id: String,
    pub client_id: Option<String>,
    pub symbol: String,
    pub side: Side,
    #[serde(rename = "type")] pub order_type: OrderType,
    pub qty: f64,
    pub price: Option<f64>,
    pub ts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub symbol: String,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub mid: Option<f64>,
    pub ts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillEvent {
    pub fill_id: String,
    pub order_id: String,
    pub symbol: String,
    pub side: Side,
    pub qty: f64,
    pub price: f64,
    pub fee: f64,
    pub ts: i64,
    pub venue: String,
    pub paper: bool,
}
