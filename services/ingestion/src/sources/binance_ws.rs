use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Deserialize)]
struct StreamEnvelope<T> { stream: Option<String>, data: T }

#[derive(Debug, Deserialize)]
struct KlineMsg { e: String, E: u64, s: String, k: KlineInner }

#[derive(Debug, Deserialize)]
struct KlineInner {
    t: u64,
    T: u64,
    s: String,
    i: String,
    o: String,
    c: String,
    h: String,
    l: String,
    v: String,
    n: u64,
    x: bool,
    q: String,
    V: String,
    Q: String,
}

impl KlineInner {
    fn to_candle(&self, exchange: &str) -> common::model::Candle {
        common::model::Candle {
            exchange: exchange.to_string(),
            market: "spot".into(),
            symbol: self.s.clone(),
            interval: self.i.clone(),
            open_time: self.t,
            close_time: self.T,
            open: self.o.parse().unwrap_or(0.0),
            high: self.h.parse().unwrap_or(0.0),
            low: self.l.parse().unwrap_or(0.0),
            close: self.c.parse().unwrap_or(0.0),
            volume: self.v.parse().unwrap_or(0.0),
            number_of_trades: self.n,
            quote_asset_volume: self.q.parse().unwrap_or(0.0),
            taker_buy_base_asset_volume: self.V.parse().unwrap_or(0.0),
            taker_buy_quote_asset_volume: self.Q.parse().unwrap_or(0.0),
            is_closed: false,
            source: "".to_string(),
        }
    }
}

pub async fn stream_symbol(
    exchange: &str,
    symbol: &str,
    interval: &str,
    mut on_evt: impl FnMut(common::event::CandleEvent) + Send,
) -> Result<()> {
    let stream = format!("{}@kline_{}", symbol.to_lowercase(), interval);
    let url = format!("wss://stream.binance.com:9443/ws/{}", stream);
    connect_and_read(exchange, &url, &mut on_evt).await
}

pub async fn stream_combined(
    exchange: &str,
    symbols: &[String],
    interval: &str,
    mut on_evt: impl FnMut(common::event::CandleEvent) + Send,
) -> Result<()> {
    let streams: String = symbols
        .iter()
        .map(|s| format!("{}@kline_{}", s.to_lowercase(), interval))
        .collect::<Vec<_>>()
        .join("/");
    let url = format!("wss://stream.binance.com:9443/stream?streams={}", streams);
    connect_and_read(exchange, &url, &mut on_evt).await
}

async fn connect_and_read(
    exchange: &str,
    url: &str,
    on_evt: &mut (impl FnMut(common::event::CandleEvent) + Send),
) -> Result<()> {
    let mut backoff = 1u64;
    loop {
        match connect_async(url).await {
            Ok((ws, _)) => {
                tracing::info!(%url, "ws connected");
                let (_write, mut read) = ws.split();
                backoff = 1;
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(txt)) => {
                            let parsed: Result<KlineMsg, _> = serde_json::from_str(&txt);
                            let m = match parsed {
                                Ok(v) => v,
                                Err(_) => {
                                    let env: StreamEnvelope<KlineMsg> =
                                        serde_json::from_str(&txt).context("envelope parse")?;
                                    env.data
                                }
                            };
                            if m.k.x {
                                let candle = m.k.to_candle(exchange);
                                on_evt(common::event::CandleEvent {
                                    ts_event_ms: candle.close_time,
                                    payload: candle,
                                });
                            }
                        }
                        Ok(Message::Close(_)) => break,
                        Ok(_) => {}
                        Err(e) => { tracing::error!(error=%e, "ws read error"); break; }
                    }
                }
            }
            Err(e) => { tracing::error!(error=%e, "ws connect failed"); }
        }
        tracing::warn!(backoff, "reconnecting");
        tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(60);
    }
}
