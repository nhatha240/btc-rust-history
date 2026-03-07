use anyhow::{Context, Result};
use clickhouse::Client;
use futures::StreamExt;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::Deserialize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};
use url::Url;

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

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Config {
    symbols: Vec<String>,
    interval: String,
    ws_base_url: String,
    kafka_brokers: String,
    kafka_topic_candles: String,
    ch_url: String,
    ch_db: String,
    ch_user: String,
    ch_password: String,
    health_port: u16,
}

impl Config {
    async fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();
        let symbols_env = std::env::var("SYMBOLS").unwrap_or_else(|_| "BTCUSDT".to_string());
        
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
            symbols,
            interval: std::env::var("INTERVAL").unwrap_or_else(|_| "1m".to_string()),
            ws_base_url: std::env::var("BINANCE_WS_BASE_URL")
                .unwrap_or_else(|_| "wss://stream.binance.com:9443/stream".to_string()),
            kafka_brokers: std::env::var("KAFKA_BROKERS")
                .unwrap_or_else(|_| "localhost:9092".to_string()),
            kafka_topic_candles: std::env::var("KAFKA_TOPIC_CANDLES_1M")
                .unwrap_or_else(|_| "TOPIC_CANDLES_1M".to_string()),
            ch_url: std::env::var("CH_URL")
                .unwrap_or_else(|_| "http://localhost:8123".to_string()),
            ch_db: std::env::var("CH_DB")
                .unwrap_or_else(|_| "db_trading".to_string()),
            ch_user: std::env::var("CH_USER")
                .unwrap_or_else(|_| "default".to_string()),
            ch_password: std::env::var("CH_PASSWORD").unwrap_or_default(),
            health_port: std::env::var("HEALTH_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8085),
        })
    }
}

// ── ClickHouse row ─────────────────────────────────────────────────────────────
// DateTime64(3) columns: we store i64 milliseconds epoch — CH accepts this natively.

#[derive(Debug, clickhouse::Row, serde::Serialize)]
struct Candle1mRow {
    symbol:                       String,
    open_time:                    i64,   // DateTime64(3) ms
    open:                         f64,
    high:                         f64,
    low:                          f64,
    close:                        f64,
    volume:                       f64,
    close_time:                   i64,   // DateTime64(3) ms
    quote_asset_volume:           f64,
    number_of_trades:             u64,
    taker_buy_base_asset_volume:  f64,
    taker_buy_quote_asset_volume: f64,
}

// ── Binance kline WS payload ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Combined {
    data: KlineEvent,
}

#[derive(Debug, Deserialize)]
struct KlineEvent {
    #[serde(rename = "k")]
    k: Kline,
}

#[derive(Debug, Deserialize)]
struct Kline {
    #[serde(rename = "t")] t: i64,      // open time ms
    #[serde(rename = "T")] cap_t: i64,  // close time ms
    #[serde(rename = "s")] s: String,   // symbol
    #[serde(rename = "o")] o: String,   // open
    #[serde(rename = "c")] c: String,   // close
    #[serde(rename = "h")] h: String,   // high
    #[serde(rename = "l")] l: String,   // low
    #[serde(rename = "v")] v: String,   // base volume
    #[serde(rename = "n")] n: u64,      // num trades
    #[serde(rename = "x")] x: bool,     // is closed
    #[serde(rename = "q")] q: String,   // quote volume
    #[serde(rename = "V")] cap_v: String, // taker buy base vol
    #[serde(rename = "Q")] cap_q: String, // taker buy quote vol
}

// ── Health server ──────────────────────────────────────────────────────────────

async fn health_server(port: u16) {
    use axum::{routing::get, Router};
    use std::net::SocketAddr;
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ready",  get(|| async { "ready" }));
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await
        .expect("health server bind failed");
    info!("health server on :{port}");
    axum::serve(listener, app).await.expect("health server failed");
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let cfg = Config::from_env().await.context("load config")?;
    info!(symbols_count=cfg.symbols.len(), interval=%cfg.interval, "ingestion service starting");

    let hp = cfg.health_port;
    tokio::spawn(async move { health_server(hp).await });

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.kafka_brokers)
        .set("message.timeout.ms", "5000")
        .set("acks", "1")
        .create()
        .context("kafka producer")?;

    let ch = Client::default()
        .with_url(&cfg.ch_url)
        .with_database(&cfg.ch_db)
        .with_user(&cfg.ch_user)
        .with_password(&cfg.ch_password);

    let cfg_arc = Arc::new(cfg);
    let mut handles = Vec::new();

    for chunk in cfg_arc.symbols.chunks(50) {
        let worker_symbols = chunk.to_vec();
        let worker_cfg = Arc::clone(&cfg_arc);
        let worker_producer = producer.clone();
        let worker_ch = ch.clone();

        handles.push(tokio::spawn(async move {
            run_ws_worker(worker_symbols, worker_cfg, worker_producer, worker_ch).await;
        }));
        
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    futures::future::join_all(handles).await;
    Ok(())
}

async fn run_ws_worker(symbols: Vec<String>, cfg: Arc<Config>, producer: FutureProducer, ch: Client) {
    let streams = symbols
        .iter()
        .map(|s| format!("{}@kline_{}", s.to_lowercase(), cfg.interval))
        .collect::<Vec<_>>()
        .join("/");
    let ws_url_str = format!("{}?streams={}", cfg.ws_base_url, streams);
    let ws_url = match Url::parse(&ws_url_str).context("parse WS URL") {
        Ok(u) => u,
        Err(e) => { warn!("Invalid WS URL: {e}"); return; }
    };

    info!(%ws_url, "connecting binance kline stream");

    let mut backoff_ms = 500u64;
    loop {
        match connect_async(ws_url.as_str()).await {
            Ok((stream, _)) => {
                info!("binance WS connected");
                backoff_ms = 500;
                let (_, mut read) = stream.split();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Err(e) = handle_message(
                                &ch,
                                &producer,
                                &cfg.kafka_topic_candles,
                                text.as_str(),
                            ).await {
                                warn!("handle_message error: {e:#}");
                            }
                        }
                        Ok(Message::Close(f)) => { warn!(?f, "WS closed"); break; }
                        Err(e)               => { warn!("WS error: {e}"); break; }
                        _ => {}
                    }
                }
            }
            Err(e) => error!("WS connect failed: {e}"),
        }
        warn!(backoff_ms, "reconnecting after backoff");
        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(30_000);
    }
}

async fn handle_message(ch: &Client, producer: &FutureProducer, topic: &str, text: &str) -> Result<()> {
    let combined: Combined = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    let k = combined.data.k;
    if !k.x { return Ok(()); } // only process closed candles

    let p = |s: &str| -> f64 { s.parse().unwrap_or(0.0) };

    let row = Candle1mRow {
        symbol:                      k.s.clone(),
        open_time:                   k.t,
        open:                        p(&k.o),
        high:                        p(&k.h),
        low:                         p(&k.l),
        close:                       p(&k.c),
        volume:                      p(&k.v),
        close_time:                  k.cap_t,
        quote_asset_volume:          p(&k.q),
        number_of_trades:            k.n,
        taker_buy_base_asset_volume: p(&k.cap_v),
        taker_buy_quote_asset_volume:p(&k.cap_q),
    };

    let mut ins = ch.insert("candles_1m_final")?;
    ins.write(&row).await?;
    ins.end().await?;
    info!(symbol=%k.s, open_time=%k.t, close=%k.c, "candle→CH");

    let payload = serde_json::to_vec(&serde_json::json!({
        "symbol": k.s, "open_time": k.t, "close_time": k.cap_t,
        "open": p(&k.o), "high": p(&k.h), "low": p(&k.l), "close": p(&k.c),
        "volume": p(&k.v), "number_of_trades": k.n,
        "is_closed": true,
    }))?;
    producer.send(
        FutureRecord::to(topic).payload(&payload).key(k.s.as_str()),
        Duration::from_secs(0),
    ).await.map_err(|(e, _)| anyhow::anyhow!("kafka: {e}"))?;

    Ok(())
}

#[allow(dead_code)]
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
