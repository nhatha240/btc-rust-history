use anyhow::{Context, Result};
use hft_proto::md::{RawBookTick, RawTradeTick};
use serde::Deserialize;
use url::Url;

#[derive(Debug)]
pub enum NormalizedEvent {
    Trade(RawTradeTick),
    Book(RawBookTick),
}

pub fn build_ws_url(base_url: &str) -> Result<Url> {
    Url::parse(base_url).context("invalid Binance WS URL")
}

pub fn build_subscribe_messages(symbols: &[String]) -> Vec<String> {
    let mut streams = Vec::new();
    for s in symbols {
        let ss = s.to_lowercase();
        streams.push(format!("{ss}@trade"));
        streams.push(format!("{ss}@bookTicker"));
    }

    // Binance limit is 50 streams per subscribe request
    let mut messages = Vec::new();
    for chunk in streams.chunks(50) {
        let req = serde_json::json!({
            "method": "SUBSCRIBE",
            "params": chunk,
            "id": 1
        });
        messages.push(req.to_string());
    }
    messages
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
        // data.e is event time in ms; convert to ns
        let event_time_ns = (data.e as i64).saturating_mul(1_000_000);
        return Ok(Some(NormalizedEvent::Trade(RawTradeTick {
            symbol: data.s,
            trade_id: data.t,
            price,
            qty,
            is_buyer_maker: data.m,
            exchange_event_time_ms: data.e as i64,
            event_time_ns,
            recv_time_ns,
            seq: trade_seq,
            trace_id: hft_common::ids::new_trace_id(),
        })));
    }

    if msg.stream.contains("@bookTicker") {
        let data: BookData = serde_json::from_value(msg.data)?;
        let best_bid = data.b.parse::<f64>().unwrap_or(0.0);
        let best_bid_qty = data.cap_b.parse::<f64>().unwrap_or(0.0);
        let best_ask = data.a.parse::<f64>().unwrap_or(0.0);
        let best_ask_qty = data.cap_a.parse::<f64>().unwrap_or(0.0);
        // bookTicker has no 'E' (event time); use recv_time_ns converted to ms
        let exchange_event_time_ms = recv_time_ns / 1_000_000;
        return Ok(Some(NormalizedEvent::Book(RawBookTick {
            symbol: data.s,
            best_bid,
            best_bid_qty,
            best_ask,
            best_ask_qty,
            exchange_event_time_ms,
            event_time_ns: recv_time_ns,
            recv_time_ns,
            seq: book_seq,
            trace_id: hft_common::ids::new_trace_id(),
        })));
    }

    Ok(None)
}

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Combined {
    stream: String,
    data: serde_json::Value,
}

/// Binance `<symbol>@trade` stream payload fields.
#[derive(Debug, Deserialize)]
struct TradeData {
    /// Event time (ms since epoch) — present in trade stream as field "E"
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

/// Binance `<symbol>@bookTicker` stream payload fields.
///
/// **NOTE:** bookTicker does NOT include field `E` (event time).
/// Present fields: `u` (update ID), `s`, `b`, `B`, `a`, `A`.
/// We use recv_time_ns as the event timestamp instead.
#[derive(Debug, Deserialize)]
struct BookData {
    #[serde(rename = "s")]
    s: String,
    /// Best bid price
    #[serde(rename = "b")]
    b: String,
    /// Best bid quantity — uppercase B in Binance JSON
    #[serde(rename = "B")]
    cap_b: String,
    /// Best ask price
    #[serde(rename = "a")]
    a: String,
    /// Best ask quantity — uppercase A in Binance JSON
    #[serde(rename = "A")]
    cap_a: String,
}
