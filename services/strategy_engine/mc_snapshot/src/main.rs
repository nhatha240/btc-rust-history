use anyhow::{Context, Result};
use clickhouse::Client;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Config {
    kafka_brokers: String,
    kafka_topic: String,
    ch_url: String,
    ch_db: String,
    ch_user: String,
    ch_password: String,
    poll_interval_secs: u64,
    coingecko_ids: Vec<String>,
    id_to_symbol: HashMap<String, String>,
    health_port: u16,
}

impl Config {
    fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let sym_to_id: HashMap<&str, &str> = [
            ("BTCUSDT", "bitcoin"),
            ("ETHUSDT", "ethereum"),
            ("BNBUSDT", "binancecoin"),
            ("SOLUSDT", "solana"),
            ("XRPUSDT", "ripple"),
        ].into_iter().collect();

        let symbols: Vec<String> = std::env::var("SYMBOLS")
            .unwrap_or_else(|_| "BTCUSDT,ETHUSDT".to_string())
            .split(',')
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect();

        let mut id_to_symbol: HashMap<String, String> = HashMap::new();
        let mut coingecko_ids: Vec<String> = Vec::new();
        for sym in &symbols {
            if let Some(&cg_id) = sym_to_id.get(sym.as_str()) {
                id_to_symbol.insert(cg_id.to_string(), sym.clone());
                coingecko_ids.push(cg_id.to_string());
            } else {
                warn!(symbol=%sym, "no CoinGecko mapping, skipping");
            }
        }

        Ok(Self {
            kafka_brokers: std::env::var("KAFKA_BROKERS")
                .unwrap_or_else(|_| "localhost:9092".to_string()),
            kafka_topic: std::env::var("KAFKA_TOPIC_MC_SNAPSHOT")
                .unwrap_or_else(|_| "TOPIC_MC_SNAPSHOT".to_string()),
            ch_url: std::env::var("CLICKHOUSE_HTTP_URL")
                .unwrap_or_else(|_| "http://localhost:8123".to_string()),
            ch_db: std::env::var("CLICKHOUSE_DB")
                .unwrap_or_else(|_| "db_trading".to_string()),
            ch_user: std::env::var("CLICKHOUSE_USER")
                .unwrap_or_else(|_| "default".to_string()),
            ch_password: std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default(),
            poll_interval_secs: std::env::var("MC_POLL_INTERVAL_SECS")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(60),
            coingecko_ids,
            id_to_symbol,
            health_port: std::env::var("HEALTH_PORT")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(8086),
        })
    }
}

// ── ClickHouse row ────────────────────────────────────────────────────────────
// DateTime64(3) column: store i64 ms epoch — accepted natively by ClickHouse.

#[derive(Debug, clickhouse::Row, Serialize)]
struct McRow {
    ts:        i64,   // DateTime64(3) ms
    symbol:    String,
    marketcap: f64,
    dominance: f64,
}

// ── CoinGecko types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CgMarketItem {
    id: String,
    market_cap: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct CgGlobal {
    data: CgGlobalData,
}

#[derive(Debug, Deserialize)]
struct CgGlobalData {
    market_cap_percentage: HashMap<String, f64>,
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
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let cfg = Config::from_env().context("load config")?;
    info!(
        kafka_topic=%cfg.kafka_topic,
        coingecko_ids=?cfg.coingecko_ids,
        poll_interval_secs=%cfg.poll_interval_secs,
        "mc_snapshot starting"
    );

    if cfg.coingecko_ids.is_empty() {
        error!("No CoinGecko IDs resolved — nothing to do, sleeping forever");
        tokio::time::sleep(Duration::MAX).await;
        return Ok(());
    }

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

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("hft-mc-snapshot/1.0")
        .build()
        .context("reqwest client")?;

    let ids_param = cfg.coingecko_ids.join(",");

    loop {
        match fetch_and_publish(&http, &ch, &producer, &cfg, &ids_param).await {
            Ok(n)  => info!(rows=%n, "mc_snapshot cycle done"),
            Err(e) => warn!("mc_snapshot cycle error: {e:#}"),
        }
        tokio::time::sleep(Duration::from_secs(cfg.poll_interval_secs)).await;
    }
}

// ── Core fetch+publish logic ───────────────────────────────────────────────────

async fn fetch_and_publish(
    http: &reqwest::Client,
    ch: &Client,
    producer: &FutureProducer,
    cfg: &Config,
    ids_param: &str,
) -> Result<usize> {
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Fetch market cap data
    let url = format!(
        "https://api.coingecko.com/api/v3/coins/markets\
         ?vs_currency=usd&ids={ids_param}&per_page=50&page=1"
    );
    let coins: Vec<CgMarketItem> = http.get(&url)
        .send().await.context("CoinGecko markets request")?
        .error_for_status().context("CoinGecko HTTP error")?
        .json().await.context("CoinGecko parse")?;

    // Fetch dominance
    let global: CgGlobal = http
        .get("https://api.coingecko.com/api/v3/global")
        .send().await.context("CoinGecko global request")?
        .error_for_status().context("CoinGecko global HTTP error")?
        .json().await.context("CoinGecko global parse")?;

    let dom = &global.data.market_cap_percentage;

    let mut ins = ch.insert("mc_snapshot")?;
    let mut count = 0usize;

    for coin in &coins {
        let symbol = match cfg.id_to_symbol.get(&coin.id) {
            Some(s) => s.clone(),
            None => continue,
        };
        let marketcap = coin.market_cap.unwrap_or(0.0);
        // CoinGecko dominance key: first 3 chars of the id, e.g. "bitcoin"→"bit", "ethereum"→"eth"
        // Actually CoinGecko uses "btc", "eth" etc.
        let dom_key = match coin.id.as_str() {
            "bitcoin"     => "btc",
            "ethereum"    => "eth",
            "binancecoin" => "bnb",
            "solana"      => "sol",
            "ripple"      => "xrp",
            _             => "",
        };
        let dominance = dom.get(dom_key).copied().unwrap_or(0.0);

        let row = McRow { ts: ts_ms, symbol: symbol.clone(), marketcap, dominance };
        ins.write(&row).await?;

        let payload = serde_json::to_vec(&serde_json::json!({
            "ts": ts_ms, "symbol": &symbol,
            "marketcap": marketcap, "dominance": dominance,
        }))?;
        producer.send(
            FutureRecord::to(&cfg.kafka_topic).payload(&payload).key(symbol.as_str()),
            Duration::from_secs(0),
        ).await.map_err(|(e, _)| anyhow::anyhow!("kafka: {e}"))?;

        info!(%symbol, %marketcap, %dominance, "mc_snapshot→kafka");
        count += 1;
    }

    ins.end().await?;
    Ok(count)
}
