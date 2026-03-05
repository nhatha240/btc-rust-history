use anyhow::Result;
use hft_proto::oms::OrderSide;

#[derive(Debug, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub kafka_topic_order_commands: String,
    pub account_id: String,
    pub symbol: String,
    pub side: i32,
    pub qty: f64,
    pub price: f64,
    pub stop_price: f64,
    pub emit_interval_ms: u64,
    pub emit_once: bool,
    pub decision_log_enabled: bool,
    pub database_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let side_raw = std::env::var("MANUAL_SIDE").unwrap_or_else(|_| "BUY".to_string());
        let side = if side_raw.eq_ignore_ascii_case("SELL") {
            OrderSide::Sell as i32
        } else {
            OrderSide::Buy as i32
        };

        Ok(Self {
            kafka_brokers: std::env::var("KAFKA_BROKERS")
                .unwrap_or_else(|_| "localhost:9092".to_string()),
            kafka_topic_order_commands: std::env::var("KAFKA_TOPIC_ORDER_COMMANDS")
                .or_else(|_| std::env::var("KAFKA_TOPIC_ORDERS_APPROVED"))
                .unwrap_or_else(|_| "TOPIC_ORDERS_APPROVED".to_string()),
            account_id: std::env::var("MANUAL_ACCOUNT_ID").unwrap_or_else(|_| "paper-main".to_string()),
            symbol: std::env::var("MANUAL_SYMBOL").unwrap_or_else(|_| "BTCUSDT".to_string()),
            side,
            qty: std::env::var("MANUAL_QTY")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.01),
            price: std::env::var("MANUAL_PRICE")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(62000.0),
            stop_price: std::env::var("MANUAL_STOP_PRICE")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0),
            emit_interval_ms: std::env::var("EMIT_INTERVAL_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5000),
            emit_once: std::env::var("EMIT_ONCE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
            decision_log_enabled: std::env::var("DECISION_LOG_ENABLED")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://trader:traderpw@localhost:5432/db_trading".to_string()),
        })
    }
}
