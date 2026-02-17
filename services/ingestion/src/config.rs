use anyhow::{Context, Result};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Config {
    pub kafka_brokers: String,
    pub kafka_topic: String,
    pub symbols: Vec<String>,        // list hoặc ["*"] để theo dõi tất cả
    pub interval: String,
    pub exchange: String,
    pub poll_secs: u64,
    // discovery options
    pub only_spot: bool,
    pub quotes: Option<Vec<String>>, // ví dụ: USDT,FDUSD
    pub ws_chunk: usize,             // số stream/kết nối combined
    // optional ClickHouse sink
    pub ch_url: Option<String>,
    pub ch_db: Option<String>,
    pub ch_user: Option<String>,
    pub ch_pass: Option<String>,
    pub ch_table: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();
        Ok(Self {
            kafka_brokers: std::env::var("KAFKA_BROKERS").context("KAFKA_BROKERS")?,
            kafka_topic: std::env::var("TOPIC_CANDLES").unwrap_or_else(|_| common::event::TOPIC_CANDLES_1M.to_string()),
            symbols: std::env::var("MD_SYMBOLS").unwrap_or_else(|_| "*".to_string())
                .split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            interval: std::env::var("MD_INTERVAL").unwrap_or_else(|_| "1m".to_string()),
            exchange: std::env::var("MD_EXCHANGE").unwrap_or_else(|_| "binance".to_string()),
            poll_secs: std::env::var("MD_POLL_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(2),
            only_spot: std::env::var("MD_ONLY_SPOT").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(true),
            quotes: std::env::var("MD_QUOTES").ok().map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s|!s.is_empty()).collect()),
            ws_chunk: std::env::var("MD_WS_CHUNK").ok().and_then(|v| v.parse().ok()).unwrap_or(500),
            ch_url: std::env::var("CLICKHOUSE_URL").ok(),
            ch_db: std::env::var("CLICKHOUSE_DB").ok(),
            ch_user: std::env::var("CLICKHOUSE_USER").ok(),
            ch_pass: std::env::var("CLICKHOUSE_PASSWORD").ok(),
            ch_table: std::env::var("CLICKHOUSE_TABLE").ok(),
        })
    }
    pub fn poll(&self) -> Duration { Duration::from_secs(self.poll_secs) }
}
