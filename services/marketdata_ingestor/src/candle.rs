use anyhow::{Context, Result};
use clickhouse::Client;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};
use url::Url;
use futures::StreamExt;

use crate::config::Config;

// ── KlineInterval ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum KlineInterval {
    M1,
    M3,
    M5,
    M15,
    M30,
    H1,
    H2,
    H4,
    H6,
    H8,
    H12,
    D1,
    D3,
    W1,
    Mo1,
}

impl KlineInterval {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::M1 => "1m",
            Self::M3 => "3m",
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::M30 => "30m",
            Self::H1 => "1h",
            Self::H2 => "2h",
            Self::H4 => "4h",
            Self::H6 => "6h",
            Self::H8 => "8h",
            Self::H12 => "12h",
            Self::D1 => "1d",
            Self::D3 => "3d",
            Self::W1 => "1w",
            Self::Mo1 => "1M",
        }
    }

    pub fn table_suffix(self) -> &'static str {
        self.as_str()
    }

    pub fn from_env_value(value: &str) -> Result<Self> {
        match value {
            "1m" => Ok(Self::M1),
            "3m" => Ok(Self::M3),
            "5m" => Ok(Self::M5),
            "15m" => Ok(Self::M15),
            "30m" => Ok(Self::M30),
            "1h" => Ok(Self::H1),
            "2h" => Ok(Self::H2),
            "4h" => Ok(Self::H4),
            "6h" => Ok(Self::H6),
            "8h" => Ok(Self::H8),
            "12h" => Ok(Self::H12),
            "1d" => Ok(Self::D1),
            "3d" => Ok(Self::D3),
            "1w" => Ok(Self::W1),
            "1M" => Ok(Self::Mo1),
            other => anyhow::bail!("unsupported INTERVAL: {other}"),
        }
    }
}

// ── Wire types ───────────────────────────────────────────────────────────────

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
    #[serde(rename = "t")]
    t: i64,
    #[serde(rename = "T")]
    cap_t: i64,
    #[serde(rename = "s")]
    s: String,
    #[serde(rename = "o")]
    o: String,
    #[serde(rename = "c")]
    c: String,
    #[serde(rename = "h")]
    h: String,
    #[serde(rename = "l")]
    l: String,
    #[serde(rename = "v")]
    v: String,
    #[serde(rename = "n")]
    n: u64,
    #[serde(rename = "x")]
    x: bool,
    #[serde(rename = "q")]
    q: String,
    #[serde(rename = "V")]
    cap_v: String,
    #[serde(rename = "Q")]
    cap_q: String,
}

// ── Row / Event types ────────────────────────────────────────────────────────

#[derive(Debug, clickhouse::Row, Serialize, Clone)]
pub struct CandleRow {
    pub symbol: String,
    pub interval: String,
    pub open_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub close_time: i64,
    pub quote_asset_volume: f64,
    pub number_of_trades: u64,
    pub taker_buy_base_asset_volume: f64,
    pub taker_buy_quote_asset_volume: f64,
    pub exchange: String,
    pub market: String,
    pub event_ingested_at_ms: i64,
}

#[derive(Debug, Serialize, Clone)]
struct ClosedCandleEvent {
    symbol: String,
    interval: String,
    open_time: i64,
    close_time: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    quote_asset_volume: f64,
    number_of_trades: u64,
    taker_buy_base_asset_volume: f64,
    taker_buy_quote_asset_volume: f64,
    exchange: String,
    market: String,
    event_ingested_at_ms: i64,
    is_closed: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedClosedCandle {
    pub row: CandleRow,
    pub kafka_key: String,
    pub kafka_payload: Vec<u8>,
}

// ── Dedup cache ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CandleKey {
    symbol: String,
    interval: String,
    open_time: i64,
}

#[derive(Debug)]
struct DedupeCache {
    capacity: usize,
    order: VecDeque<CandleKey>,
    seen: HashMap<CandleKey, ()>,
}

impl DedupeCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: VecDeque::with_capacity(capacity),
            seen: HashMap::with_capacity(capacity),
        }
    }

    fn insert_if_new(&mut self, key: CandleKey) -> bool {
        if self.seen.contains_key(&key) {
            return false;
        }

        self.order.push_back(key.clone());
        self.seen.insert(key, ());

        while self.order.len() > self.capacity {
            if let Some(evicted) = self.order.pop_front() {
                self.seen.remove(&evicted);
            }
        }

        true
    }
}

// ── Metrics / state ──────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct CandleWorkerMetrics {
    pub connected_workers: AtomicU64,
    pub last_ws_message_at_ms: AtomicI64,
    pub last_kafka_success_at_ms: AtomicI64,
    pub last_ch_success_at_ms: AtomicI64,
    pub parsed_candle_count: AtomicU64,
    pub duplicate_candle_count: AtomicU64,
    pub parse_error_count: AtomicU64,
    #[allow(dead_code)]
    pub sink_error_count: AtomicU64,
}

#[derive(Debug)]
pub struct CandleAppState {
    pub ready: AtomicBool,
    pub metrics: CandleWorkerMetrics,
}

impl CandleAppState {
    pub fn new() -> Self {
        Self {
            ready: AtomicBool::new(false),
            metrics: CandleWorkerMetrics::default(),
        }
    }

    pub fn mark_ready(&self, is_ready: bool) {
        self.ready.store(is_ready, Ordering::Relaxed);
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }
}

// ── Default constants ────────────────────────────────────────────────────────

pub const DEFAULT_WS_CHUNK_SIZE: usize = 50;
pub const DEFAULT_WORKER_CONNECT_STAGGER_MS: u64 = 250;
pub const DEFAULT_CH_BATCH_SIZE: usize = 500;
pub const DEFAULT_CH_FLUSH_INTERVAL_MS: u64 = 200;
pub const DEFAULT_PIPELINE_CHANNEL_CAPACITY: usize = 20_000;
pub const DEFAULT_WS_STALE_TIMEOUT_MS: i64 = 90_000;
pub const DEFAULT_DEDUPE_CAPACITY_PER_WORKER: usize = 10_000;
pub const DEFAULT_WS_INITIAL_BACKOFF_MS: u64 = 500;
pub const DEFAULT_WS_MAX_BACKOFF_MS: u64 = 30_000;

// ── Public entry point ───────────────────────────────────────────────────────

/// Spawn all candle workers + sink loop. Returns a future that completes when
/// all workers have finished (should never happen under normal conditions).
pub async fn run_candle_pipeline(
    cfg: Arc<Config>,
    producer: FutureProducer,
    ch: Client,
    state: Arc<CandleAppState>,
) -> Result<()> {
    let (pipeline_tx, pipeline_rx) =
        mpsc::channel::<ParsedClosedCandle>(cfg.candle_pipeline_channel_capacity);

    let mut tasks = tokio::task::JoinSet::new();

    // Sink loop
    {
        let sink_cfg = cfg.clone();
        let sink_state = state.clone();
        let sink_producer = producer.clone();
        let sink_ch = ch.clone();
        tasks.spawn(async move {
            if let Err(error) =
                sink_loop(pipeline_rx, sink_cfg, sink_producer, sink_ch, sink_state).await
            {
                error!(error = %error, "candle sink loop failed");
            }
        });
    }

    // WS workers
    for symbol_chunk in cfg.symbols.chunks(cfg.candle_ws_chunk_size) {
        let worker_symbols = symbol_chunk.to_vec();
        let worker_cfg = cfg.clone();
        let worker_state = state.clone();
        let worker_tx = pipeline_tx.clone();

        tasks.spawn(async move {
            if let Err(error) =
                run_candle_ws_worker(worker_symbols, worker_cfg, worker_tx, worker_state).await
            {
                error!(error = %error, "candle ws worker failed");
            }
        });

        tokio::time::sleep(Duration::from_millis(cfg.candle_worker_connect_stagger_ms)).await;
    }

    drop(pipeline_tx);
    state.mark_ready(true);

    while let Some(join_result) = tasks.join_next().await {
        if let Err(error) = join_result {
            error!(error = %error, "candle task join failure");
        }
    }

    Ok(())
}

// ── WS worker ────────────────────────────────────────────────────────────────

async fn run_candle_ws_worker(
    symbols: Vec<String>,
    cfg: Arc<Config>,
    pipeline_tx: mpsc::Sender<ParsedClosedCandle>,
    state: Arc<CandleAppState>,
) -> Result<()> {
    if symbols.is_empty() {
        anyhow::bail!("candle worker received empty symbols chunk");
    }

    let streams = symbols
        .iter()
        .map(|symbol| {
            format!(
                "{}@kline_{}",
                symbol.to_lowercase(),
                cfg.candle_interval.as_str()
            )
        })
        .collect::<Vec<_>>()
        .join("/");

    let ws_url_str = format!("{}?streams={}", cfg.candle_futures_ws_base_url, streams);
    let ws_url = Url::parse(&ws_url_str).with_context(|| format!("parse WS URL: {ws_url_str}"))?;

    let dedupe_cache = Arc::new(Mutex::new(DedupeCache::new(
        cfg.candle_dedupe_capacity_per_worker,
    )));
    let mut backoff_ms = DEFAULT_WS_INITIAL_BACKOFF_MS;

    loop {
        info!(ws_url = %ws_url, symbols_count = symbols.len(), "connecting candle ws stream");

        match connect_async(ws_url.as_str()).await {
            Ok((stream, _)) => {
                state
                    .metrics
                    .connected_workers
                    .fetch_add(1, Ordering::Relaxed);
                info!(symbols_count = symbols.len(), "candle ws connected");
                backoff_ms = DEFAULT_WS_INITIAL_BACKOFF_MS;

                let (_, mut read) = stream.split();

                loop {
                    let next_message = tokio::time::timeout(
                        Duration::from_millis(cfg.candle_ws_stale_timeout_ms as u64),
                        read.next(),
                    )
                    .await;

                    let message = match next_message {
                        Ok(value) => value,
                        Err(_) => {
                            warn!(
                                stale_timeout_ms = cfg.candle_ws_stale_timeout_ms,
                                symbols_count = symbols.len(),
                                "candle ws stream stale timeout, reconnecting"
                            );
                            break;
                        }
                    };

                    let Some(message_result) = message else {
                        warn!(
                            symbols_count = symbols.len(),
                            "candle ws stream ended, reconnecting"
                        );
                        break;
                    };

                    match message_result {
                        Ok(Message::Text(text)) => {
                            state
                                .metrics
                                .last_ws_message_at_ms
                                .store(now_ms(), Ordering::Relaxed);

                            match parse_closed_candle_message(
                                text.as_str(),
                                cfg.candle_interval,
                            ) {
                                Ok(Some(parsed)) => {
                                    let dedupe_key = CandleKey {
                                        symbol: parsed.row.symbol.clone(),
                                        interval: parsed.row.interval.clone(),
                                        open_time: parsed.row.open_time,
                                    };

                                    let is_new = {
                                        let mut cache = dedupe_cache.lock().await;
                                        cache.insert_if_new(dedupe_key)
                                    };

                                    if !is_new {
                                        state
                                            .metrics
                                            .duplicate_candle_count
                                            .fetch_add(1, Ordering::Relaxed);
                                        debug!(
                                            symbol = %parsed.row.symbol,
                                            interval = %parsed.row.interval,
                                            open_time = parsed.row.open_time,
                                            "duplicate closed candle skipped"
                                        );
                                        continue;
                                    }

                                    state
                                        .metrics
                                        .parsed_candle_count
                                        .fetch_add(1, Ordering::Relaxed);

                                    if let Err(error) = pipeline_tx.send(parsed).await {
                                        anyhow::bail!("candle pipeline channel closed: {error}");
                                    }
                                }
                                Ok(None) => {}
                                Err(error) => {
                                    state
                                        .metrics
                                        .parse_error_count
                                        .fetch_add(1, Ordering::Relaxed);
                                    warn!(error = %error, "parse closed candle message failed");
                                }
                            }
                        }
                        Ok(Message::Close(frame)) => {
                            warn!(?frame, "candle ws stream closed by peer");
                            break;
                        }
                        Ok(Message::Ping(_)) => {}
                        Ok(Message::Pong(_)) => {}
                        Ok(Message::Binary(_)) => {}
                        Ok(Message::Frame(_)) => {}
                        Err(error) => {
                            warn!(error = %error, "candle ws read error");
                            break;
                        }
                    }
                }

                state
                    .metrics
                    .connected_workers
                    .fetch_sub(1, Ordering::Relaxed);
            }
            Err(error) => {
                error!(error = %error, ws_url = %ws_url, "candle ws connect failed");
            }
        }

        warn!(
            backoff_ms,
            symbols_count = symbols.len(),
            "reconnecting candle ws worker after backoff"
        );
        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(DEFAULT_WS_MAX_BACKOFF_MS);
    }
}

// ── Message parsing ──────────────────────────────────────────────────────────

fn parse_closed_candle_message(
    text: &str,
    interval: KlineInterval,
) -> Result<Option<ParsedClosedCandle>> {
    let combined: Combined = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };

    let kline = combined.data.k;
    if !kline.x {
        return Ok(None);
    }

    let open = parse_f64_field(&kline.o, "open", &kline.s, kline.t)?;
    let high = parse_f64_field(&kline.h, "high", &kline.s, kline.t)?;
    let low = parse_f64_field(&kline.l, "low", &kline.s, kline.t)?;
    let close = parse_f64_field(&kline.c, "close", &kline.s, kline.t)?;
    let volume = parse_f64_field(&kline.v, "volume", &kline.s, kline.t)?;
    let quote_asset_volume =
        parse_f64_field(&kline.q, "quote_asset_volume", &kline.s, kline.t)?;
    let taker_buy_base_asset_volume =
        parse_f64_field(&kline.cap_v, "taker_buy_base_asset_volume", &kline.s, kline.t)?;
    let taker_buy_quote_asset_volume =
        parse_f64_field(&kline.cap_q, "taker_buy_quote_asset_volume", &kline.s, kline.t)?;

    let event_ingested_at_ms = now_ms();
    let interval_value = interval.as_str().to_string();

    let row = CandleRow {
        symbol: kline.s.clone(),
        interval: interval_value.clone(),
        open_time: kline.t,
        open,
        high,
        low,
        close,
        volume,
        close_time: kline.cap_t,
        quote_asset_volume,
        number_of_trades: kline.n,
        taker_buy_base_asset_volume,
        taker_buy_quote_asset_volume,
        exchange: "binance".to_string(),
        market: "futures".to_string(),
        event_ingested_at_ms,
    };

    let event = ClosedCandleEvent {
        symbol: row.symbol.clone(),
        interval: interval_value,
        open_time: row.open_time,
        close_time: row.close_time,
        open: row.open,
        high: row.high,
        low: row.low,
        close: row.close,
        volume: row.volume,
        quote_asset_volume: row.quote_asset_volume,
        number_of_trades: row.number_of_trades,
        taker_buy_base_asset_volume: row.taker_buy_base_asset_volume,
        taker_buy_quote_asset_volume: row.taker_buy_quote_asset_volume,
        exchange: row.exchange.clone(),
        market: row.market.clone(),
        event_ingested_at_ms: row.event_ingested_at_ms,
        is_closed: true,
    };

    let kafka_key = format!("{}:{}:{}", row.symbol, row.interval, row.open_time);
    let kafka_payload =
        serde_json::to_vec(&event).context("serialize kafka closed candle event")?;

    Ok(Some(ParsedClosedCandle {
        row,
        kafka_key,
        kafka_payload,
    }))
}

fn parse_f64_field(raw: &str, field_name: &'static str, symbol: &str, open_time: i64) -> Result<f64> {
    raw.parse::<f64>().with_context(|| {
        format!("failed to parse field={field_name} symbol={symbol} open_time={open_time} raw={raw}")
    })
}

// ── Sink loop ────────────────────────────────────────────────────────────────

async fn sink_loop(
    mut pipeline_rx: mpsc::Receiver<ParsedClosedCandle>,
    cfg: Arc<Config>,
    producer: FutureProducer,
    ch: Client,
    state: Arc<CandleAppState>,
) -> Result<()> {
    let flush_interval = Duration::from_millis(cfg.candle_ch_flush_interval_ms);
    let mut flush_tick = tokio::time::interval(flush_interval);
    let mut batch: Vec<ParsedClosedCandle> = Vec::with_capacity(cfg.candle_ch_batch_size);

    loop {
        tokio::select! {
            maybe_item = pipeline_rx.recv() => {
                match maybe_item {
                    Some(item) => {
                        batch.push(item);
                        if batch.len() >= cfg.candle_ch_batch_size {
                            flush_batch(&mut batch, &cfg, &producer, &ch, &state).await?;
                        }
                    }
                    None => {
                        if !batch.is_empty() {
                            flush_batch(&mut batch, &cfg, &producer, &ch, &state).await?;
                        }
                        info!("candle sink loop channel closed");
                        return Ok(());
                    }
                }
            }
            _ = flush_tick.tick() => {
                if !batch.is_empty() {
                    flush_batch(&mut batch, &cfg, &producer, &ch, &state).await?;
                }
                refresh_readiness(&state, cfg.candle_ws_stale_timeout_ms);
            }
        }
    }
}

async fn flush_batch(
    batch: &mut Vec<ParsedClosedCandle>,
    cfg: &Config,
    producer: &FutureProducer,
    ch: &Client,
    state: &CandleAppState,
) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }

    write_clickhouse_batch(ch, &cfg.candle_ch_table, batch)
        .await
        .with_context(|| format!("write batch to clickhouse table={}", cfg.candle_ch_table))?;

    state
        .metrics
        .last_ch_success_at_ms
        .store(now_ms(), Ordering::Relaxed);

    for item in batch.iter() {
        producer
            .send(
                FutureRecord::to(&cfg.candle_kafka_topic)
                    .payload(&item.kafka_payload)
                    .key(&item.kafka_key),
                Duration::from_secs(0),
            )
            .await
            .map_err(|(error, _)| anyhow::anyhow!("candle kafka send failed: {error}"))?;

        state
            .metrics
            .last_kafka_success_at_ms
            .store(now_ms(), Ordering::Relaxed);
    }

    if let Some(last) = batch.last() {
        debug!(
            batch_size = batch.len(),
            last_symbol = %last.row.symbol,
            last_open_time = last.row.open_time,
            table = %cfg.candle_ch_table,
            topic = %cfg.candle_kafka_topic,
            "flushed candle batch"
        );
    }

    batch.clear();
    refresh_readiness(state, cfg.candle_ws_stale_timeout_ms);

    Ok(())
}

async fn write_clickhouse_batch(
    ch: &Client,
    table_name: &str,
    batch: &[ParsedClosedCandle],
) -> Result<()> {
    let mut insert = ch.insert(table_name)?;
    for item in batch {
        insert.write(&item.row).await?;
    }
    insert.end().await?;
    Ok(())
}

fn refresh_readiness(state: &CandleAppState, ws_stale_timeout_ms: i64) {
    let now = now_ms();
    let connected_workers = state.metrics.connected_workers.load(Ordering::Relaxed);
    let last_ws_message_at_ms = state.metrics.last_ws_message_at_ms.load(Ordering::Relaxed);
    let last_kafka_success_at_ms = state.metrics.last_kafka_success_at_ms.load(Ordering::Relaxed);
    let last_ch_success_at_ms = state.metrics.last_ch_success_at_ms.load(Ordering::Relaxed);

    let ws_recent =
        last_ws_message_at_ms > 0 && now - last_ws_message_at_ms <= ws_stale_timeout_ms;
    let kafka_recent = last_kafka_success_at_ms > 0
        && now - last_kafka_success_at_ms <= ws_stale_timeout_ms * 2;
    let ch_recent =
        last_ch_success_at_ms > 0 && now - last_ch_success_at_ms <= ws_stale_timeout_ms * 2;

    let is_ready = connected_workers > 0 && ws_recent && kafka_recent && ch_recent;
    state.mark_ready(is_ready);
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
