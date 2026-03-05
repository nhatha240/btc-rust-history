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
use hft_proto::md::{RawBookTick, RawTradeTick};

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

    let cfg = Config::from_env().context("load marketdata_ingestor config failed")?;
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

    let trade_seq = Arc::new(AtomicU64::new(0));
    let book_seq = Arc::new(AtomicU64::new(0));

    let ws_url = build_ws_url(&cfg.ws_base_url, &cfg.symbols)?;
    let mut reconnect = ReconnectController::new(cfg.reconnect_base_ms, cfg.reconnect_max_ms);

    loop {
        match reconnect.connecting() {
            ConnectionState::Connecting => info!(url=%ws_url, "connecting websocket"),
            _ => {}
        }

        match connect_async(ws_url.as_str()).await {
            Ok((stream, _)) => {
                let _ = reconnect.on_connected();
                info!("websocket connected");
                let (_, mut read) = stream.split();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Err(e) = handle_text(
                                &producer,
                                &cfg,
                                text.as_str(),
                                &trade_seq,
                                &book_seq,
                            )
                            .await
                            {
                                warn!("handle text failed: {e:#}");
                            }
                        }
                        Ok(Message::Binary(_)) => {}
                        Ok(Message::Ping(_)) => {}
                        Ok(Message::Pong(_)) => {}
                        Ok(Message::Close(frame)) => {
                            warn!(?frame, "websocket closed by peer");
                            break;
                        }
                        Ok(Message::Frame(_)) => {}
                        Err(e) => {
                            warn!("websocket read error: {e}");
                            break;
                        }
                    }
                }
            }
            Err(e) => warn!("websocket connect error: {e}"),
        }

        match reconnect.on_disconnected() {
            ConnectionState::Backoff(delay) => {
                warn!(backoff_ms = delay.as_millis(), "reconnecting after backoff");
                tokio::time::sleep(delay).await;
            }
            _ => tokio::time::sleep(Duration::from_millis(1000)).await,
        }
    }
}

async fn handle_text(
    producer: &FutureProducer,
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
