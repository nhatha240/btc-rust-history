// src/bin/main
// Minimal, production-leaning paper trading simulator for Kafka pipelines.
// - Consumes approved orders from KAFKA_TOPIC_ORDERS_APPROVED
// - Optionally consumes market snapshots from KAFKA_TOPIC_MC_SNAPSHOT
// - Produces synthetic fills to KAFKA_TOPIC_FILLS
// - Idempotency per order_id via in-memory TTL cache
// - Slippage in bps and artificial latency
// Build: cargo build --bin paper_trader
// Env:
//   KAFKA_BROKERS (e.g., "redpanda:9092")
//   KAFKA_GROUP_ID (default: "paper_trader")
//   KAFKA_TOPIC_ORDERS_APPROVED (e.g., "TOPIC_ORDERS_APPROVED")
//   KAFKA_TOPIC_FILLS (e.g., "TOPIC_FILLS")
//   KAFKA_TOPIC_MC_SNAPSHOT (optional, e.g., "TOPIC_MC_SNAPSHOT")
//   SLIPPAGE_BPS (default: 5)
//   FEE_BPS (default: 2)
//   LATENCY_MS (default: 50)
//   PENDING_TIMEOUT_MS (default: 10_000)

use std::{collections::HashMap, time::{Duration, Instant}};
use std::sync::Arc;

use rdkafka::{consumer::{Consumer, StreamConsumer}, producer::{FutureProducer, FutureRecord}, ClientConfig, Message};
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock, time::sleep};
use uuid::Uuid;

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

#[derive(Clone)]
struct AppCfg {
    brokers: String,
    group_id: String,
    topic_orders: String,
    topic_fills: String,
    topic_snap: Option<String>,
    slippage_bps: f64,
    fee_bps: f64,
    latency: Duration,
    pending_timeout: Duration,
}

impl AppCfg {
    fn from_env() -> Self {
        let getenv = |k: &str, d: &str| std::env::var(k).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| d.to_string());
        Self {
            brokers: getenv("KAFKA_BROKERS", "redpanda:9092"),
            group_id: getenv("KAFKA_GROUP_ID", "paper_trader"),
            topic_orders: getenv("KAFKA_TOPIC_ORDERS_APPROVED", "TOPIC_ORDERS_APPROVED"),
            topic_fills: getenv("KAFKA_TOPIC_FILLS", "TOPIC_FILLS"),
            topic_snap: std::env::var("KAFKA_TOPIC_MC_SNAPSHOT").ok().filter(|v| !v.is_empty()),
            slippage_bps: getenv("SLIPPAGE_BPS", "5").parse().unwrap_or(5.0),
            fee_bps: getenv("FEE_BPS", "2").parse().unwrap_or(2.0),
            latency: Duration::from_millis(getenv("LATENCY_MS", "50").parse().unwrap_or(50)),
            pending_timeout: Duration::from_millis(getenv("PENDING_TIMEOUT_MS", "10000").parse().unwrap_or(10_000)),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();
    let cfg = AppCfg::from_env();

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.brokers)
        .set("message.timeout.ms", "5000")
        .create()?;

    // Shared state
    let prices: Arc<RwLock<HashMap<String, (f64, Instant)>>> = Arc::new(RwLock::new(HashMap::new()));
    let processed: Arc<RwLock<HashMap<String, Instant>>> = Arc::new(RwLock::new(HashMap::new()));

    // Spawn optional snapshot consumer
    if let Some(topic_snap) = cfg.topic_snap.clone() {
        let prices_clone = prices.clone();
        let cfg_clone = cfg.clone();
        tokio::spawn(async move {
            let consumer: StreamConsumer = ClientConfig::new()
                .set("bootstrap.servers", &cfg_clone.brokers)
                .set("group.id", format!("{}_snap", &cfg_clone.group_id))
                .set("enable.auto.commit", "true")
                .set("auto.offset.reset", "latest")
                .create()
                .expect("snapshot consumer");
            consumer.subscribe(&[&topic_snap]).expect("subscribe snapshot");
            loop {
                match consumer.recv().await {
                    Ok(m) => {
                        if let Some(Ok(payload)) = m.payload_view::<str>() {
                            if let Ok(snap) = serde_json::from_str::<MarketSnapshot>(payload) {
                                let mid = snap.mid.or_else(|| midpoint(snap.bid, snap.ask));
                                if let Some(p) = mid {
                                    prices_clone.write().await.insert(snap.symbol, (p, Instant::now()));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("snapshot error: {e:?}");
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        });
    }

    // Orders consumer
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.brokers)
        .set("group.id", &cfg.group_id)
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "latest")
        .create()?;
    consumer.subscribe(&[&cfg.topic_orders])?;

    // Main loop
    while let Ok(msg) = consumer.recv().await {
        if let Some(Ok(payload)) = msg.payload_view::<str>() {
            if let Ok(order) = serde_json::from_str::<OrderApprovedEvent>(payload) {
                if already_processed(&processed, &order.order_id).await { continue; }
                match try_fill(&cfg, &prices, &order).await {
                    Ok(Some(fill)) => {
                        // artificial latency
                        sleep(cfg.latency).await;
                        publish_fill(&producer, &cfg.topic_fills, &fill, &order.symbol).await?;
                        mark_processed(&processed, &order.order_id).await;
                    }
                    Ok(None) => {
                        // Could not fill now (no price or not cross). Leave unprocessed to allow future re-consume if resent.
                    }
                    Err(err) => {
                        eprintln!("fill error for {}: {err:?}", order.order_id);
                    }
                }
            }
        }
        // Commit handled by enable.auto.commit
    }

    Ok(())
}

fn midpoint(bid: Option<f64>, ask: Option<f64>) -> Option<f64> {
    match (bid, ask) {
        (Some(b), Some(a)) if a > 0.0 => Some((a + b) / 2.0),
        (Some(b), None) => Some(b),
        (None, Some(a)) => Some(a),
        _ => None,
    }
}

async fn try_fill(cfg: &AppCfg,
                  prices: &Arc<RwLock<HashMap<String, (f64, Instant)>>>,
                  o: &OrderApprovedEvent) -> anyhow::Result<Option<FillEvent>> {
    // Price selection
    let maybe_mid = {
        let map = prices.read().await;
        map.get(&o.symbol).cloned()
    };

    let now = chrono::Utc::now().timestamp_millis();

    // Determine execution price
    let px = match o.order_type {
        OrderType::MARKET => {
            let (mid, ts) = match maybe_mid {
                Some((p, t)) => (p, t),
                None => return Ok(None),
            };
            if ts.elapsed() > cfg.pending_timeout { return Ok(None); }
            // Apply slippage: BUY pays more, SELL receives less
            let slip = cfg.slippage_bps / 10_000.0;
            match o.side {
                Side::BUY => mid * (1.0 + slip),
                Side::SELL => mid * (1.0 - slip),
            }
        }
        OrderType::LIMIT => {
            let limit = o.price.ok_or_else(|| anyhow::anyhow!("limit price missing"))?;
            let (mid, _ts) = match maybe_mid { Some(v) => v, None => return Ok(None) };
            match o.side {
                Side::BUY => {
                    if mid <= limit { mid.min(limit) } else { return Ok(None); }
                }
                Side::SELL => {
                    if mid >= limit { mid.max(limit) } else { return Ok(None); }
                }
            }
        }
    };

    // Fee
    let fee = o.qty.abs() * px * (cfg.fee_bps / 10_000.0);

    let fill = FillEvent {
        fill_id: Uuid::new_v4().to_string(),
        order_id: o.order_id.clone(),
        symbol: o.symbol.clone(),
        side: o.side.clone(),
        qty: o.qty,
        price: px,
        fee,
        ts: now,
        venue: "PAPER".to_string(),
        paper: true,
    };

    Ok(Some(fill))
}

async fn publish_fill(producer: &FutureProducer, topic: &str, fill: &FillEvent, key: &str) -> anyhow::Result<()> {
    let payload = serde_json::to_vec(fill)?;
    let rec = FutureRecord::to(topic)
        .payload(&payload)
        .key(key);
    producer.send(rec, Duration::from_secs(5)).await
        .map_err(|(e, _)| anyhow::anyhow!("produce error: {e}"))?;
    Ok(())
}

async fn already_processed(processed: &Arc<RwLock<HashMap<String, Instant>>>, order_id: &str) -> bool {
    // TTL 10 minutes
    let ttl = Duration::from_secs(600);
    let mut map = processed.write().await;
    // GC
    let now = Instant::now();
    map.retain(|_, t| now.duration_since(*t) < ttl);
    if map.contains_key(order_id) { true } else { false }
}

async fn mark_processed(processed: &Arc<RwLock<HashMap<String, Instant>>>, order_id: &str) {
    processed.write().await.insert(order_id.to_string(), Instant::now());
}

fn init_logging() {
    // Simple stderr logger
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    let _ = env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .try_init();
}
