// services/risk_guard/src/main.rs
// Risk guard: consumes orders, applies kill switch, rate limit, and notional limits,
// then forwards approved orders to KAFKA_TOPIC_ORDERS_APPROVED.
// Optional: listens to market snapshots to price orders for notional checks.
// Optional: reads current position from Postgres if DATABASE_URL is set.

use std::{collections::HashMap, time::{Duration, Instant}};
use std::sync::Arc;

use anyhow::Result;
use rdkafka::{consumer::{Consumer, StreamConsumer}, producer::{FutureProducer, FutureRecord}, ClientConfig, Message};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock, time::sleep};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderEvent {
    order_id: String,
    symbol: String,
    side: String,     // "BUY" | "SELL"
    qty: f64,
    #[serde(rename = "type")] r#type: String, // "MARKET" | "LIMIT"
    price: Option<f64>, // present for LIMIT
    ts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MarketSnapshot {
    symbol: String,
    bid: Option<f64>,
    ask: Option<f64>,
    mid: Option<f64>,
    ts: i64,
}

#[derive(Clone)]
struct Cfg {
    brokers: String,
    grp: String,
    topic_in: String,
    topic_out: String,
    topic_snap: Option<String>,
    redis_url: String,
    kill_key_global: String,
    limit_notional_order: f64,
    limit_orders_per_min: i64,
    pending_px_ttl: Duration,
}

impl Cfg {
    fn from_env() -> Self {
        let ge = |k: &str, d: &str| std::env::var(k).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| d.to_string());
        Self {
            brokers: ge("KAFKA_BROKERS", "redpanda:9092"),
            grp: ge("KAFKA_GROUP_ID", "risk_guard"),
            topic_in: ge("KAFKA_TOPIC_ORDERS_IN", "TOPIC_ORDERS"),
            topic_out: ge("KAFKA_TOPIC_ORDERS_APPROVED", "TOPIC_ORDERS_APPROVED"),
            topic_snap: std::env::var("KAFKA_TOPIC_MC_SNAPSHOT").ok().filter(|v| !v.is_empty()),
            redis_url: ge("REDIS_URL", "redis://redis:6379/0"),
            kill_key_global: ge("KILL_SWITCH_KEY", "risk:kill"),
            limit_notional_order: ge("LIMIT_NOTIONAL_PER_SYMBOL", "5000").parse().unwrap_or(5000.0),
            limit_orders_per_min: ge("MAX_ORDERS_PER_MIN", "30").parse().unwrap_or(30),
            pending_px_ttl: Duration::from_millis(ge("PENDING_TIMEOUT_MS", "10000").parse().unwrap_or(10_000)),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_log();
    let cfg = Cfg::from_env();

    // Kafka
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.brokers)
        .set("group.id", &cfg.grp)
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "latest")
        .create()?;
    consumer.subscribe(&[&cfg.topic_in])?;

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.brokers)
        .set("message.timeout.ms", "5000")
        .create()?;

    // Redis
    let redis = redis::Client::open(cfg.redis_url.clone())?;
    let mut rc = redis.get_async_connection().await?;

    // Snapshot cache
    let prices: Arc<RwLock<HashMap<String, (f64, Instant)>>> = Arc::new(RwLock::new(HashMap::new()));
    if let Some(topic_snap) = cfg.topic_snap.clone() {
        let prices_clone = prices.clone();
        let brokers = cfg.brokers.clone();
        let grp = format!("{}_snap", &cfg.grp);
        tokio::spawn(async move {
            let c: StreamConsumer = ClientConfig::new()
                .set("bootstrap.servers", &brokers)
                .set("group.id", &grp)
                .set("enable.auto.commit", "true")
                .set("auto.offset.reset", "latest")
                .create()
                .expect("snap consumer");
            c.subscribe(&[&topic_snap]).expect("snap subscribe");
            loop {
                match c.recv().await {
                    Ok(m) => {
                        if let Some(Ok(p)) = m.payload_view::<str>() {
                            if let Ok(s) = serde_json::from_str::<MarketSnapshot>(p) {
                                let mid = s.mid.or_else(|| midpoint(s.bid, s.ask));
                                if let Some(px) = mid { prices_clone.write().await.insert(s.symbol, (px, Instant::now())); }
                            }
                        }
                    }
                    Err(e) => { eprintln!("snap recv error: {e:?}"); sleep(Duration::from_millis(200)).await; }
                }
            }
        });
    }

    // Main loop
    loop {
        let msg = consumer.recv().await?;
        if let Some(Ok(payload)) = msg.payload_view::<str>() {
            match serde_json::from_str::<OrderEvent>(payload) {
                Ok(order) => {
                    if let Err(e) = process_order(&cfg, &mut rc, &producer, &prices, order).await {
                        eprintln!("risk_guard error: {e:?}");
                    }
                }
                Err(e) => eprintln!("bad order json: {e:?}"),
            }
        }
    }
}

async fn process_order(cfg: &Cfg,
                       rc: &mut redis::aio::Connection,
                       producer: &FutureProducer,
                       prices: &Arc<RwLock<HashMap<String, (f64, Instant)>>>,
                       o: OrderEvent) -> Result<()> {
    // Idempotency
    let idem_key = format!("risk:processed:{}", o.order_id);
    let set: bool = rc.set_nx(&idem_key, 1).await?;
    if !set { return Ok(()); }
    let _: () = rc.expire(&idem_key, 600).await?;

    // Kill switch
    if is_killed(rc, &cfg.kill_key_global).await? || is_killed(rc, &format!("{}:{}", cfg.kill_key_global, &o.symbol)).await? {
        log::warn!("blocked by kill switch: {} {} {}", o.symbol, o.side, o.qty);
        return Ok(());
    }

    // Rate limit per symbol per minute
    let rate_key = format!("risk:rate:{}", &o.symbol);
    let cnt: i64 = rc.incr(&rate_key, 1).await?;
    if cnt == 1 { let _: () = rc.expire(&rate_key, 60).await?; }
    if cnt > cfg.limit_orders_per_min { log::warn!("rate limited {} count={}", o.symbol, cnt); return Ok(()); }

    // Notional limit per order
    let px = match price_for_order(prices, &o, cfg.pending_px_ttl).await { Some(p) => p, None => { log::warn!("no price for {} -> drop", o.symbol); return Ok(()); } };
    let notional = o.qty.abs() * px;
    if notional > cfg.limit_notional_order { log::warn!("notional too large {} notional={}", o.symbol, notional); return Ok(()); }

    // Approve -> forward identical order
    let bytes = serde_json::to_vec(&o)?;
    producer.send(
        FutureRecord::to(&cfg.topic_out).payload(&bytes).key(&o.symbol),
        Duration::from_secs(5)
    ).await.ok();

    Ok(())
}

async fn price_for_order(prices: &Arc<RwLock<HashMap<String, (f64, Instant)>>>, o: &OrderEvent, ttl: Duration) -> Option<f64> {
    match o.r#type.as_str() {
        "LIMIT" => o.price,
        _ => {
            let m = prices.read().await;
            m.get(&o.symbol).and_then(|(p, t)| if t.elapsed() <= ttl { Some(*p) } else { None })
        }
    }
}

async fn is_killed(rc: &mut redis::aio::Connection, key: &str) -> redis::RedisResult<bool> {
    let v: Option<String> = rc.get(key).await?;
    Ok(matches!(v.as_deref(), Some("1")))
}

fn midpoint(bid: Option<f64>, ask: Option<f64>) -> Option<f64> {
    match (bid, ask) {
        (Some(b), Some(a)) if a > 0.0 => Some((a + b) / 2.0),
        (Some(b), None) => Some(b),
        (None, Some(a)) => Some(a),
        _ => None,
    }
}

fn init_log() {
    if std::env::var("RUST_LOG").is_err() { std::env::set_var("RUST_LOG", "info"); }
    let _ = env_logger::Builder::from_default_env().format_timestamp_millis().try_init();
}
