use anyhow::{Context, Result};
use hft_proto::md::{RawBookTick, RawTradeTick};
use serde::Deserialize;
use url::Url;

#[derive(Debug)]
pub enum NormalizedEvent {
    Trade(RawTradeTick),
    Book(RawBookTick),
}

pub fn build_ws_url(base_url: &str, symbols: &[String]) -> Result<Url> {
    let mut streams = Vec::new();
    for s in symbols {
        let ss = s.to_lowercase();
        streams.push(format!("{ss}@trade"));
        streams.push(format!("{ss}@bookTicker"));
    }
    let joined = streams.join("/");
    let raw = format!("{base_url}?streams={joined}");
    Url::parse(&raw).context("invalid Binance WS URL")
}

pub fn normalize(
    text: &str,
    recv_time_ns: i64,
    trade_seq: u64,
    book_seq: u64,
) -> Result<Option<NormalizedEvent>> {
    let msg: Combined = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    if msg.stream.contains("@trade") {
        let data: TradeData = serde_json::from_value(msg.data)?;
        let price = data.p.parse::<f64>().unwrap_or(0.0);
        let qty = data.q.parse::<f64>().unwrap_or(0.0);
        let event_time_ns = data.e.saturating_mul(1_000_000);
        return Ok(Some(NormalizedEvent::Trade(RawTradeTick {
            symbol: data.s,
            trade_id: data.t,
            price,
            qty,
            is_buyer_maker: data.m,
            exchange_event_time_ms: data.e as i64,
            event_time_ns: event_time_ns as i64,
            recv_time_ns,
            seq: trade_seq,
            trace_id: hft_common::ids::new_trace_id(),
        })));
    }

    if msg.stream.contains("@bookTicker") {
        let data: BookData = serde_json::from_value(msg.data)?;
        let best_bid = data.b.parse::<f64>().unwrap_or(0.0);
        let best_bid_qty = data.b_qty.parse::<f64>().unwrap_or(0.0);
        let best_ask = data.a.parse::<f64>().unwrap_or(0.0);
        let best_ask_qty = data.a_qty.parse::<f64>().unwrap_or(0.0);
        let event_time_ns = data.e.saturating_mul(1_000_000);
        return Ok(Some(NormalizedEvent::Book(RawBookTick {
            symbol: data.s,
            best_bid,
            best_bid_qty,
            best_ask,
            best_ask_qty,
            exchange_event_time_ms: data.e as i64,
            event_time_ns: event_time_ns as i64,
            recv_time_ns,
            seq: book_seq,
            trace_id: hft_common::ids::new_trace_id(),
        })));
    }

    Ok(None)
}

#[derive(Debug, Deserialize)]
struct Combined {
    stream: String,
    data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct TradeData {
    #[serde(rename = "E")]
    e: u64,
    #[serde(rename = "s")]
    s: String,
    #[serde(rename = "t")]
    t: u64,
    #[serde(rename = "p")]
    p: String,
    #[serde(rename = "q")]
    q: String,
    #[serde(rename = "m")]
    m: bool,
}

#[derive(Debug, Deserialize)]
struct BookData {
    #[serde(rename = "E")]
    e: u64,
    #[serde(rename = "s")]
    s: String,
    #[serde(rename = "b")]
    b: String,
    #[serde(rename = "B")]
    b_qty: String,
    #[serde(rename = "a")]
    a: String,
    #[serde(rename = "A")]
    a_qty: String,
}
