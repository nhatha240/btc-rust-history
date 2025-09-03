use std::result::Result::Ok;
use anyhow::*;
use tokio::time::{sleep, Duration};
use futures::StreamExt;
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn, error};
use url::Url;

use common::{
    model::Candle,
    event::{TOPIC_CANDLES_1M, key_symbol},
    kafka::{producer},
    clickhouse::{client as ch_client, insert_candles_1m_live}, // chỉ lấy hàm helper
};
// alias kiểu Client của crate clickhouse (ngoại lệ)
use clickhouse::Client as ChClient;

const WS_BASE: &str = "wss://stream.binance.com:9443/stream";

#[derive(serde::Deserialize)]
struct Combined<T>{ stream:String, data:T }

#[derive(serde::Deserialize, Debug)]
struct KEvent { e:String, s:String, k:Kline }
#[derive(serde::Deserialize, Debug)]
struct Kline {
    t:u64, T:u64, s:String, i:String,
    o:String, c:String, h:String, l:String,
    v:String, n:u64, q:String, V:String, Q:String, x:bool
}

#[tokio::main]
async fn main() -> Result<()> {
    init_log();

    let brokers = std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into());
    let ch_url = std::env::var("CH_URL").unwrap_or_else(|_| "http://localhost:8123".into());
    let ch_user = std::env::var("CH_USER").ok();
    let ch_pass = std::env::var("CH_PASSWORD").ok();
    let exchange = std::env::var("EXCHANGE").unwrap_or_else(|_| "binance_spot".into());
    let intervals = std::env::var("INTERVALS").unwrap_or_else(|_| "1m".into());
    let batch_size:usize = std::env::var("BATCH_SIZE").ok().and_then(|v| v.parse().ok()).unwrap_or(200);

    let symbols = fetch_symbols_spot().await?;
    let streams: Vec<String> = symbols.iter().flat_map(|s| {
        intervals.split(',').map(move |i| format!("{}@kline_{}", s.to_lowercase(), i.trim()))
    }).collect();

    let prod = producer(&brokers);
    let ch = ch_client(&ch_url, "db_trading", ch_user.as_deref(), ch_pass.as_deref());

    for chunk in streams.chunks(batch_size) {
        let url = build_ws_url(chunk)?;
        let prod = prod.clone();
        let ch = ch.clone();
        let exchange = exchange.clone();

        tokio::spawn(async move {
            let mut backoff = 1u64;
            loop {
                match session(url.clone(), &exchange, &prod, &ch).await {
                    Ok(()) => { backoff = 1; sleep(Duration::from_secs(1)).await; }
                    Err(e) => { warn!("WS error: {e:?}"); sleep(Duration::from_secs(backoff)).await; backoff = (backoff*2).min(60); }
                }
            }
        });
    }

    loop { sleep(Duration::from_secs(3600)).await; }
}

fn init_log(){ tracing_subscriber::fmt().with_env_filter("info").compact().init(); }

async fn fetch_symbols_spot() -> Result<Vec<String>> {
    #[derive(serde::Deserialize)] struct Info{ symbols:Vec<Sym> }
    #[derive(serde::Deserialize)] struct Sym{ symbol:String, status:String }
    let info:Info = reqwest::Client::new()
        .get("https://api.binance.com/api/v3/exchangeInfo")
        .send().await?.error_for_status()?.json().await?;
    Ok(info.symbols.into_iter().filter(|s| s.status=="TRADING").map(|s| s.symbol).collect())
}

fn build_ws_url(keys: &[String]) -> Result<Url> {
    let mut u = Url::parse(WS_BASE)?;
    u.query_pairs_mut().append_pair("streams", &keys.join("/"));
    Ok(u)
}

async fn session(url: Url, exchange:&str, prod:&rdkafka::producer::FutureProducer, ch:&ChClient) -> Result<()> {
    info!("Connect WS: {}", url);
    let (ws, _) = tokio_tungstenite::connect_async(url.as_str()).await?;
    let (_, mut read) = ws.split();

    let mut buf: Vec<Candle> = Vec::with_capacity(256);

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(txt)) => {
                if let Ok(pkt) = serde_json::from_str::<Combined<KEvent>>(&txt) {
                    let k = pkt.data.k;
                    if k.i != "1m" { continue; } // this service handles 1m; duplicate this for others
                    let candle = Candle{
                        exchange: exchange.into(), market:"spot".into(), symbol:k.s.clone(), interval:k.i.clone(),
                        open_time_ms:k.t, close_time_ms:k.T,
                        open:k.o.parse().unwrap_or(0.0), high:k.h.parse().unwrap_or(0.0),
                        low:k.l.parse().unwrap_or(0.0), close:k.c.parse().unwrap_or(0.0),
                        volume:k.v.parse().unwrap_or(0.0), trade_count:k.n,
                        quote_volume:k.q.parse().unwrap_or(0.0),
                        taker_buy_base_volume:k.V.parse().unwrap_or(0.0),
                        taker_buy_quote_volume:k.Q.parse().unwrap_or(0.0),
                        is_closed:k.x, source:"ws".into()
                    };

                    // produce to bus
                    let key = key_symbol(&candle.exchange, &candle.symbol);
                    common::kafka::send_json(prod, TOPIC_CANDLES_1M, &key, &candle).await.ok();

                    // buffer → ClickHouse
                    if candle.is_closed { buf.push(candle); }
                    if buf.len() >= 200 {
                        if let Err(e) = insert_candles_1m_live(ch, &buf).await { error!("CH insert: {e:?}"); }
                        buf.clear();
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Err(e) => { warn!("WS read: {e:?}"); break; }
            _ => {}
        }
    }
    if !buf.is_empty() { let _ = insert_candles_1m_live(ch, &buf).await; }
    Ok(())
}
