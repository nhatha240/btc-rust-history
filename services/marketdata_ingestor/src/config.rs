use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub kafka_topic_raw_trades: String,
    pub kafka_topic_raw_book: String,
    pub kafka_client_id: String,
    pub symbols: Vec<String>,
    pub ws_base_url: String,
    pub reconnect_base_ms: u64,
    pub reconnect_max_ms: u64,
    pub health_port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let symbols = std::env::var("SYMBOLS")
            .unwrap_or_else(|_| "BTCUSDT".to_string())
            .split(',')
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        Ok(Self {
            kafka_brokers: std::env::var("KAFKA_BROKERS")
                .unwrap_or_else(|_| "localhost:9092".to_string()),
            kafka_topic_raw_trades: std::env::var("KAFKA_TOPIC_RAW_TRADES")
                .unwrap_or_else(|_| "md.raw.trades".to_string()),
            kafka_topic_raw_book: std::env::var("KAFKA_TOPIC_RAW_BOOK")
                .unwrap_or_else(|_| "md.raw.book".to_string()),
            kafka_client_id: std::env::var("KAFKA_CLIENT_ID")
                .unwrap_or_else(|_| "marketdata-ingestor".to_string()),
            symbols,
            ws_base_url: std::env::var("BINANCE_WS_BASE_URL")
                .unwrap_or_else(|_| "wss://stream.binance.com:9443/stream".to_string()),
            reconnect_base_ms: std::env::var("RECONNECT_BASE_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(500),
            reconnect_max_ms: std::env::var("RECONNECT_MAX_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(30_000),
            health_port: std::env::var("HEALTH_PORT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(8081),
        })
    }
}
