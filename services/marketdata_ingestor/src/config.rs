use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub kafka_topic_raw_trades: String,
    pub kafka_topic_raw_book: String,
    pub kafka_topic_raw_orderbook: String,
    pub kafka_topic_raw_open_interest: String,
    pub kafka_topic_raw_mark_price: String,
    pub kafka_topic_raw_liquidation: String,
    pub kafka_client_id: String,
    pub symbols: Vec<String>,
    pub ws_base_url: String,
    pub order_book_depth: u16,
    pub reconnect_base_ms: u64,
    pub reconnect_max_ms: u64,
    pub health_port: u16,
    pub ch_url: String,
    pub ch_db: String,
    pub ch_user: String,
    pub ch_password: String,
}

use serde::Deserialize;

#[derive(Deserialize)]
struct ExchangeInfo {
    symbols: Vec<SymbolInfo>,
}

#[derive(Deserialize)]
struct SymbolInfo {
    symbol: String,
    status: String,
    #[serde(rename = "quoteAsset")]
    quote_asset: String,
}

impl Config {
    pub async fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let symbols_env = std::env::var("SYMBOLS").unwrap_or_else(|_| "ALL_USDT".to_string());

        let mut symbols = Vec::new();
        if symbols_env == "ALL_USDT" {
            let info: ExchangeInfo = reqwest::get("https://fapi.binance.com/fapi/v1/exchangeInfo")
                .await?
                .json()
                .await?;
            for s in info.symbols {
                if s.status == "TRADING" && s.quote_asset == "USDT" {
                    symbols.push(s.symbol);
                }
            }
        } else {
            symbols = symbols_env
                .split(',')
                .map(|s| s.trim().to_uppercase())
                .filter(|s| !s.is_empty())
                .collect();
        }

        Ok(Self {
            kafka_brokers: std::env::var("KAFKA_BROKERS")
                .unwrap_or_else(|_| "localhost:9092".to_string()),
            kafka_topic_raw_trades: std::env::var("KAFKA_TOPIC_RAW_TRADES")
                .unwrap_or_else(|_| "md.raw.trades".to_string()),
            kafka_topic_raw_book: std::env::var("KAFKA_TOPIC_RAW_BOOK")
                .unwrap_or_else(|_| "md.raw.book".to_string()),
            kafka_topic_raw_orderbook: std::env::var("KAFKA_TOPIC_RAW_ORDERBOOK")
                .unwrap_or_else(|_| "md.raw.orderbook".to_string()),
            kafka_topic_raw_open_interest: std::env::var("KAFKA_TOPIC_RAW_OPEN_INTEREST")
                .unwrap_or_else(|_| "md.raw.open_interest".to_string()),
            kafka_topic_raw_mark_price: std::env::var("KAFKA_TOPIC_RAW_MARK_PRICE")
                .unwrap_or_else(|_| "md.raw.mark_price".to_string()),
            kafka_topic_raw_liquidation: std::env::var("KAFKA_TOPIC_RAW_LIQUIDATION")
                .unwrap_or_else(|_| "md.raw.liquidation".to_string()),
            kafka_client_id: std::env::var("KAFKA_CLIENT_ID")
                .unwrap_or_else(|_| "marketdata-ingestor".to_string()),
            symbols,
            ws_base_url: std::env::var("BINANCE_WS_BASE_URL")
                .unwrap_or_else(|_| "wss://fstream.binance.com/stream".to_string()),
            order_book_depth: std::env::var("ORDER_BOOK_DEPTH")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                // Binance depth options: 5, 10, 20, 50, 100, 500, 1000, 5000
                // Default to 1000 which covers 256
                .unwrap_or(1000),
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
            ch_url: std::env::var("CH_URL")
                .unwrap_or_else(|_| "http://localhost:8123".to_string()),
            ch_db: std::env::var("CH_DB")
                .unwrap_or_else(|_| "db_trading".to_string()),
            ch_user: std::env::var("CH_USER")
                .unwrap_or_else(|_| "default".to_string()),
            ch_password: std::env::var("CH_PASSWORD").unwrap_or_default(),
        })
    }
}
