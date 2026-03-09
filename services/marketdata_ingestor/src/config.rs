use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Duration;

use crate::candle::KlineInterval;

#[derive(Debug, Clone)]
pub struct Config {
    // ── Raw market data fields ──
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

    // ── Candle ingestion fields ──
    pub candle_interval: KlineInterval,
    #[allow(dead_code)]
    pub candle_futures_rest_base_url: String,
    pub candle_futures_ws_base_url: String,
    pub candle_kafka_topic: String,
    pub candle_ch_table: String,
    pub candle_ws_chunk_size: usize,
    pub candle_worker_connect_stagger_ms: u64,
    pub candle_ch_batch_size: usize,
    pub candle_ch_flush_interval_ms: u64,
    pub candle_pipeline_channel_capacity: usize,
    pub candle_ws_stale_timeout_ms: i64,
    pub candle_dedupe_capacity_per_worker: usize,
}

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

        let futures_rest_base_url = std::env::var("BINANCE_FUTURES_REST_BASE_URL")
            .unwrap_or_else(|_| "https://fapi.binance.com".to_string());

        let http_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .user_agent("marketdata-ingestor/1.0")
            .build()
            .context("build reqwest client")?;

        let mut symbols = Vec::new();
        if symbols_env == "ALL_USDT" {
            let url = format!("{futures_rest_base_url}/fapi/v1/exchangeInfo");
            let response = http_client
                .get(&url)
                .send()
                .await
                .with_context(|| format!("GET {url}"))?
                .error_for_status()
                .with_context(|| format!("exchangeInfo non-success status from {url}"))?;
            let info: ExchangeInfo = response.json().await.context("deserialize exchangeInfo")?;
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

        // ── Candle-specific config ──
        let candle_interval = KlineInterval::from_env_value(
            &std::env::var("INTERVAL").unwrap_or_else(|_| "1m".to_string()),
        )
        .context("parse INTERVAL")?;

        let candle_kafka_topic = std::env::var("KAFKA_TOPIC_CANDLES")
            .or_else(|_| std::env::var("KAFKA_TOPIC_CANDLES_1M"))
            .unwrap_or_else(|_| {
                format!("TOPIC_CANDLES_{}", candle_interval.as_str())
            });

        let candle_ch_table = std::env::var("CH_TABLE_CANDLES")
            .unwrap_or_else(|_| format!("candles_{}_final", candle_interval.table_suffix()));

        let candle_futures_ws_base_url = std::env::var("BINANCE_FUTURES_WS_BASE_URL")
            .or_else(|_| std::env::var("BINANCE_WS_BASE_URL"))
            .unwrap_or_else(|_| "wss://fstream.binance.com/stream".to_string());

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

            // Candle fields
            candle_interval,
            candle_futures_rest_base_url: futures_rest_base_url,
            candle_futures_ws_base_url,
            candle_kafka_topic,
            candle_ch_table,
            candle_ws_chunk_size: std::env::var("WS_CHUNK_SIZE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(crate::candle::DEFAULT_WS_CHUNK_SIZE),
            candle_worker_connect_stagger_ms: std::env::var("WORKER_CONNECT_STAGGER_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(crate::candle::DEFAULT_WORKER_CONNECT_STAGGER_MS),
            candle_ch_batch_size: std::env::var("CH_BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(crate::candle::DEFAULT_CH_BATCH_SIZE),
            candle_ch_flush_interval_ms: std::env::var("CH_FLUSH_INTERVAL_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(crate::candle::DEFAULT_CH_FLUSH_INTERVAL_MS),
            candle_pipeline_channel_capacity: std::env::var("PIPELINE_CHANNEL_CAPACITY")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(crate::candle::DEFAULT_PIPELINE_CHANNEL_CAPACITY),
            candle_ws_stale_timeout_ms: std::env::var("WS_STALE_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(crate::candle::DEFAULT_WS_STALE_TIMEOUT_MS),
            candle_dedupe_capacity_per_worker: std::env::var("DEDUPE_CAPACITY_PER_WORKER")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(crate::candle::DEFAULT_DEDUPE_CAPACITY_PER_WORKER),
        })
    }
}

