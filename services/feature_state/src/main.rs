use std::result::Result::Ok;
use anyhow::*;
use futures::StreamExt;
use rdkafka::Message;
use rdkafka::consumer::{Consumer, CommitMode};
use tracing::info;
use std::collections::HashMap;

use common::{
    model::{Candle, FeatureState, Emas, Rsi, Macd, WindowMeta},
    indicators::{RsiAccum, MacdAccum, ema_next},
    event::{TOPIC_CANDLES_1M, TOPIC_FEATURE_STATE, key_state},
    kafka::{consumer, producer, send_json}
};

#[derive(Default)]
struct Acc {
    seq:u64, n:usize, first:u64, last:u64,
    ema5:Option<f64>, ema15:Option<f64>, ema25:Option<f64>, ema50:Option<f64>,
    rsi:RsiAccum, macd:MacdAccum, cum_pv:f64, cum_vol:f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").compact().init();

    let brokers = std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into());
    let group   = std::env::var("GROUP").unwrap_or_else(|_| "features-state-v1".into());

    // ✔️ lấy StreamConsumer (không để dạng Result trong loop)
    let cons = consumer(&brokers, &group, &[TOPIC_CANDLES_1M])?;
    let prod = producer(&brokers);
    let mut state: HashMap<(String,String,String), Acc> = HashMap::new();

    loop {
        match cons.stream().next().await {
            Some(Ok(m)) => {
                if let Some(payload) = m.payload() {
                    let c: Candle = serde_json::from_slice(payload)?;

                    if !c.is_closed {
                        let _ = cons.commit_message(&m, CommitMode::Async);
                        continue;
                    }

                    let key = (c.exchange.clone(), c.symbol.clone(), c.interval.clone());
                    let e = state.entry(key.clone()).or_insert_with(|| {
                        let mut a = Acc::default(); a.rsi = RsiAccum::new(14); a
                    });

                    e.seq += 1; e.n += 1;
                    if e.first == 0 { e.first = c.open_time_ms; }
                    e.last = c.open_time_ms;

                    let close = c.close;
                    e.ema5  = Some(ema_next(e.ema5,  close, 5.0));
                    e.ema15 = Some(ema_next(e.ema15, close, 15.0));
                    e.ema25 = Some(ema_next(e.ema25, close, 25.0));
                    e.ema50 = Some(ema_next(e.ema50, close, 50.0));
                    let rsi14 = e.rsi.next(close);
                    let (macd, signal, hist) = e.macd.next(close);
                    let tp = (c.high + c.low + c.close) / 3.0;
                    e.cum_pv += tp * c.volume; e.cum_vol += c.volume;
                    let vwap = if e.cum_vol > 0.0 { e.cum_pv / e.cum_vol } else { close };

                    let out = FeatureState{
                        ts_event_ms: c.close_time_ms,
                        exchange: c.exchange, symbol: c.symbol, interval: c.interval,
                        seq: e.seq, last_close: close,
                        ema: Emas{ ema5:e.ema5.unwrap(), ema15:e.ema15.unwrap(), ema25:e.ema25.unwrap(), ema50:e.ema50.unwrap() },
                        rsi: Rsi{ rsi14 }, macd: Macd{ macd, signal, hist },
                        vwap,
                        window: WindowMeta{ n:e.n, first_open_time_ms:e.first, last_open_time_ms:e.last }
                    };

                    let routing = key_state(&out.exchange, &out.symbol, &out.interval);
                    send_json(&prod, TOPIC_FEATURE_STATE, &routing, &out).await?;

                    let _ = cons.commit_message(&m, CommitMode::Async);
                }
            }
            Some(Err(e)) => {
                tracing::warn!("Kafka error: {e:?}");
                continue;
            }
            None => {
                tracing::warn!("Kafka stream ended");
                break;
            }
        }
    }
    Ok(())
}
