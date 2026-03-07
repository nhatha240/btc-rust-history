use anyhow::{Context, Result};
use futures::StreamExt;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

use hft_common::time::now_ns;
use hft_proto::encode::to_bytes;
use hft_proto::md::{RawBookTick, RawTradeTick, RawOrderBookL2, RawOpenInterest, RawMarkPrice, RawLiquidation};
use clickhouse::Client;

#[derive(Debug, clickhouse::Row, serde::Serialize)]
struct OrderBookRow {
    venue: String,
    symbol: String,
    exchange_ts: i64,
    receive_ts: i64,
    first_update_id: u64,
    final_update_id: u64,
    prev_final_update_id: u64,
    side: i8,
    price: f64,
    quantity: f64,
    is_snapshot: u8,
}

#[derive(Debug, clickhouse::Row, serde::Serialize)]
struct FuturesContextRow {
    symbol: String,
    ts: i64,
    open_interest: f64,
    funding_rate: f64,
    liq_buy_vol: f64,
    liq_sell_vol: f64,
}

mod config;
mod health;
mod ws {
    pub mod binance;
    pub mod reconnect;
}

use config::Config;
use ws::binance::{build_ws_url, normalize, NormalizedEvent};
use ws::reconnect::{ConnectionState, ReconnectController};

#[tokio::main]
async fn main() -> Result<()> {
    // rustls 0.23 requires selecting a process-wide CryptoProvider.
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("failed to install rustls ring CryptoProvider"))?;

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let _ = rustls::crypto::ring::default_provider().install_default();

    let cfg = Config::from_env()
        .await
        .context("load marketdata_ingestor config failed")?;
    info!(symbols=?cfg.symbols, "marketdata_ingestor starting");

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.kafka_brokers)
        .set("client.id", &cfg.kafka_client_id)
        .set("message.timeout.ms", "5000")
        .set("acks", "all")
        .create()
        .context("create kafka producer failed")?;

    let health_port = cfg.health_port;
    tokio::spawn(async move {
        if let Err(e) = health::serve(health_port).await {
            error!("health server failed: {e:#}");
        }
    });

    let ch = Client::default()
        .with_url(&cfg.ch_url)
        .with_database(&cfg.ch_db)
        .with_user(&cfg.ch_user)
        .with_password(&cfg.ch_password);

    let trade_seq = Arc::new(AtomicU64::new(0));
    let book_seq = Arc::new(AtomicU64::new(0));

    let cfg_arc = Arc::new(cfg);
    let mut handles = Vec::new();

    for (i, chunk) in cfg_arc.symbols.chunks(30).enumerate() {
        let worker_symbols = chunk.to_vec();
        let worker_cfg = Arc::clone(&cfg_arc);
        let worker_producer = producer.clone();
        let worker_ch = ch.clone();
        let worker_trade_seq = Arc::clone(&trade_seq);
        let worker_book_seq = Arc::clone(&book_seq);

        handles.push(tokio::spawn(async move {
            run_ws_worker(
                i,
                worker_symbols,
                worker_cfg,
                worker_producer,
                worker_ch,
                worker_trade_seq,
                worker_book_seq
            ).await;
        }));
        
        // Stagger connection attempts slightly
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    futures::future::join_all(handles).await;

    Ok(())
}

async fn run_ws_worker(
    worker_id: usize,
    symbols: Vec<String>,
    cfg: Arc<Config>,
    producer: FutureProducer,
    ch: Client,
    trade_seq: Arc<AtomicU64>,
    book_seq: Arc<AtomicU64>,
) {
    let ws_url = build_ws_url(&cfg.ws_base_url).unwrap();
    let sub_msgs = ws::binance::build_subscribe_messages(&symbols, cfg.order_book_depth);
    let mut reconnect = ReconnectController::new(cfg.reconnect_base_ms, cfg.reconnect_max_ms);

    loop {
        match reconnect.connecting() {
            ConnectionState::Connecting => info!(worker_id, url=%ws_url, "connecting websocket"),
            _ => {}
        }

        match connect_async(ws_url.as_str()).await {
            Ok((stream, _)) => {
                let _ = reconnect.on_connected();
                info!(worker_id, "websocket connected");
                let (mut write, mut read) = stream.split();

                for msg in &sub_msgs {
                    use futures::SinkExt; // bring send into scope
                    if let Err(e) = write
                        .send(tokio_tungstenite::tungstenite::Message::Text(msg.clone()))
                        .await
                    {
                        warn!(worker_id, "failed to send subscribe message: {}", e);
                    }
                }

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Err(e) =
                                handle_text(&producer, &ch, &cfg, text.as_str(), &trade_seq, &book_seq)
                                    .await
                            {
                                warn!(worker_id, "handle text failed: {e:#}");
                            }
                        }
                        Ok(Message::Binary(_)) => {}
                        Ok(Message::Ping(_)) => {}
                        Ok(Message::Pong(_)) => {}
                        Ok(Message::Close(frame)) => {
                            warn!(worker_id, ?frame, "websocket closed by peer");
                            break;
                        }
                        Ok(Message::Frame(_)) => {}
                        Err(e) => {
                            warn!(worker_id, "websocket read error: {e}");
                            break;
                        }
                    }
                }
            }
            Err(e) => warn!(worker_id, "websocket connect error: {e}"),
        }

        match reconnect.on_disconnected() {
            ConnectionState::Backoff(delay) => {
                warn!(worker_id, backoff_ms = delay.as_millis(), "reconnecting after backoff");
                tokio::time::sleep(delay).await;
            }
            _ => tokio::time::sleep(Duration::from_millis(1000)).await,
        }
    }
}

async fn handle_text(
    producer: &FutureProducer,
    ch: &Client,
    cfg: &Config,
    text: &str,
    trade_seq: &AtomicU64,
    book_seq: &AtomicU64,
) -> Result<()> {
    let recv_time_ns = now_ns();
    let next_trade_seq = trade_seq.fetch_add(1, Ordering::Relaxed) + 1;
    let next_book_seq = book_seq.fetch_add(1, Ordering::Relaxed) + 1;

    match normalize(text, recv_time_ns, next_trade_seq, next_book_seq)? {
        Some(NormalizedEvent::Trade(tick)) => {
            publish_trade(producer, cfg, &tick).await?;
        }
        Some(NormalizedEvent::Book(tick)) => {
            publish_book(producer, cfg, &tick).await?;
        }
        Some(NormalizedEvent::OrderBookL2(tick)) => {
            publish_orderbook(producer, ch, cfg, &tick).await?;
        }
        Some(NormalizedEvent::OpenInterest(tick)) => {
            publish_open_interest(producer, ch, cfg, &tick).await?;
        }
        Some(NormalizedEvent::MarkPrice(tick)) => {
            publish_mark_price(producer, ch, cfg, &tick).await?;
        }
        Some(NormalizedEvent::Liquidation(tick)) => {
            publish_liquidation(producer, ch, cfg, &tick).await?;
        }
        None => {}
    }
    Ok(())
}

async fn publish_trade(producer: &FutureProducer, cfg: &Config, tick: &RawTradeTick) -> Result<()> {
    let payload = to_bytes(tick)?;
    producer
        .send(
            FutureRecord::to(&cfg.kafka_topic_raw_trades)
                .payload(payload.as_ref())
                .key(&tick.symbol),
            Duration::from_secs(0),
        )
        .await
        .map_err(|(e, _)| anyhow::anyhow!("publish trade failed: {e}"))?;
    Ok(())
}

async fn publish_book(producer: &FutureProducer, cfg: &Config, tick: &RawBookTick) -> Result<()> {
    let payload = to_bytes(tick)?;
    producer
        .send(
            FutureRecord::to(&cfg.kafka_topic_raw_book)
                .payload(payload.as_ref())
                .key(&tick.symbol),
            Duration::from_secs(0),
        )
        .await
        .map_err(|(e, _)| anyhow::anyhow!("publish book failed: {e}"))?;
    Ok(())
}

async fn publish_orderbook(producer: &FutureProducer, ch: &Client, cfg: &Config, tick: &RawOrderBookL2) -> Result<()> {
    let payload = to_bytes(tick)?;
    producer
        .send(
            FutureRecord::to(&cfg.kafka_topic_raw_orderbook)
                .payload(payload.as_ref())
                .key(&tick.symbol),
            Duration::from_secs(0),
        )
        .await
        .map_err(|(e, _)| anyhow::anyhow!("publish orderbook failed: {e}"))?;

    let ch_clone = ch.clone();
    let tick_clone = tick.clone();
    tokio::spawn(async move {
        if let Ok(mut ins) = ch_clone.insert("orderbook_l2_updates") {
            for b in &tick_clone.bids {
                let _ = ins.write(&OrderBookRow {
                    venue: "binance".to_string(),
                    symbol: tick_clone.symbol.clone(),
                    exchange_ts: tick_clone.exchange_event_time_ms * 1_000,
                    receive_ts: tick_clone.recv_time_ns / 1_000,
                    first_update_id: tick_clone.first_update_id,
                    final_update_id: tick_clone.final_update_id,
                    prev_final_update_id: 0,
                    side: 1, // BID
                    price: b.price,
                    quantity: b.qty,
                    is_snapshot: 0,
                }).await;
            }
            for a in &tick_clone.asks {
                let _ = ins.write(&OrderBookRow {
                    venue: "binance".to_string(),
                    symbol: tick_clone.symbol.clone(),
                    exchange_ts: tick_clone.exchange_event_time_ms * 1_000,
                    receive_ts: tick_clone.recv_time_ns / 1_000,
                    first_update_id: tick_clone.first_update_id,
                    final_update_id: tick_clone.final_update_id,
                    prev_final_update_id: 0,
                    side: 2, // ASK
                    price: a.price,
                    quantity: a.qty,
                    is_snapshot: 0,
                }).await;
            }
            let _ = ins.end().await;
        }
    });

    Ok(())
}

async fn publish_open_interest(producer: &FutureProducer, ch: &Client, cfg: &Config, tick: &RawOpenInterest) -> Result<()> {
    let payload = to_bytes(tick)?;
    producer
        .send(
            FutureRecord::to(&cfg.kafka_topic_raw_open_interest)
                .payload(payload.as_ref())
                .key(&tick.symbol),
            Duration::from_secs(0),
        )
        .await
        .map_err(|(e, _)| anyhow::anyhow!("publish open interest failed: {e}"))?;

    let ch_clone = ch.clone();
    let tick_clone = tick.clone();
    tokio::spawn(async move {
        if let Ok(mut ins) = ch_clone.insert("futures_context_1m") {
            let _ = ins.write(&FuturesContextRow {
                symbol: tick_clone.symbol.clone(),
                ts: tick_clone.exchange_event_time_ms,
                open_interest: tick_clone.open_interest,
                funding_rate: 0.0,
                liq_buy_vol: 0.0,
                liq_sell_vol: 0.0,
            }).await;
            let _ = ins.end().await;
        }
    });

    Ok(())
}

async fn publish_mark_price(producer: &FutureProducer, ch: &Client, cfg: &Config, tick: &RawMarkPrice) -> Result<()> {
    let payload = to_bytes(tick)?;
    producer
        .send(
            FutureRecord::to(&cfg.kafka_topic_raw_mark_price)
                .payload(payload.as_ref())
                .key(&tick.symbol),
            Duration::from_secs(0),
        )
        .await
        .map_err(|(e, _)| anyhow::anyhow!("publish mark price failed: {e}"))?;

    let ch_clone = ch.clone();
    let tick_clone = tick.clone();
    tokio::spawn(async move {
        if let Ok(mut ins) = ch_clone.insert("futures_context_1m") {
            let _ = ins.write(&FuturesContextRow {
                symbol: tick_clone.symbol.clone(),
                ts: tick_clone.exchange_event_time_ms,
                open_interest: 0.0,
                funding_rate: tick_clone.funding_rate,
                liq_buy_vol: 0.0,
                liq_sell_vol: 0.0,
            }).await;
            let _ = ins.end().await;
        }
    });

    Ok(())
}

async fn publish_liquidation(producer: &FutureProducer, ch: &Client, cfg: &Config, tick: &RawLiquidation) -> Result<()> {
    let payload = to_bytes(tick)?;
    producer
        .send(
            FutureRecord::to(&cfg.kafka_topic_raw_liquidation)
                .payload(payload.as_ref())
                .key(&tick.symbol),
            Duration::from_secs(0),
        )
        .await
        .map_err(|(e, _)| anyhow::anyhow!("publish liquidation failed: {e}"))?;

    let ch_clone = ch.clone();
    let tick_clone = tick.clone();
    tokio::spawn(async move {
        if let Ok(mut ins) = ch_clone.insert("futures_context_1m") {
            let buy_vol = if tick_clone.side == 1 { tick_clone.executed_qty } else { 0.0 };
            let sell_vol = if tick_clone.side == -1 { tick_clone.executed_qty } else { 0.0 };
            let _ = ins.write(&FuturesContextRow {
                symbol: tick_clone.symbol.clone(),
                ts: tick_clone.exchange_event_time_ms,
                open_interest: 0.0,
                funding_rate: 0.0,
                liq_buy_vol: buy_vol,
                liq_sell_vol: sell_vol,
            }).await;
            let _ = ins.end().await;
        }
    });

    Ok(())
}
