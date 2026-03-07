use anyhow::{Context, Result};
use hft_proto::md::{
    raw_order_book_l2::Level, RawBookTick, RawLiquidation, RawMarkPrice, RawOpenInterest,
    RawOrderBookL2, RawTradeTick,
};
use serde::Deserialize;
use url::Url;

#[derive(Debug)]
pub enum NormalizedEvent {
    Trade(RawTradeTick),
    Book(RawBookTick),
    OrderBookL2(RawOrderBookL2),
    MarkPrice(RawMarkPrice),
    OpenInterest(RawOpenInterest),
    Liquidation(RawLiquidation),
}

pub fn build_ws_url(base_url: &str) -> Result<Url> {
    Url::parse(base_url).context("invalid Binance WS URL")
}

pub fn build_subscribe_messages(symbols: &[String], order_book_depth: u16) -> Vec<String> {
    let mut streams = Vec::new();
    for s in symbols {
        let ss = s.to_lowercase();
        streams.push(format!("{ss}@trade"));
        streams.push(format!("{ss}@bookTicker"));
        streams.push(format!("{ss}@depth{}@100ms", order_book_depth));
        streams.push(format!("{ss}@markPrice@1s"));
        streams.push(format!("{ss}@openInterest"));
        streams.push(format!("{ss}@forceOrder"));
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
            schema_version: 1,
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
            schema_version: 1,
        })));
    }

    if msg.stream.contains("@depth") {
        let data: DepthData = serde_json::from_value(msg.data)?;
        let bids: Vec<Level> = data.b.into_iter().map(|v| Level {
            price: v[0].parse().unwrap_or(0.0),
            qty: v[1].parse().unwrap_or(0.0),
        }).collect();
        let asks: Vec<Level> = data.a.into_iter().map(|v| Level {
            price: v[0].parse().unwrap_or(0.0),
            qty: v[1].parse().unwrap_or(0.0),
        }).collect();
        
        return Ok(Some(NormalizedEvent::OrderBookL2(RawOrderBookL2 {
            symbol: data.s,
            bids,
            asks,
            exchange_event_time_ms: data.e_time as i64,
            recv_time_ns,
            first_update_id: data.u_first,
            final_update_id: data.u_final,
            trace_id: hft_common::ids::new_trace_id(),
            schema_version: 1,
        })));
    }

    if msg.stream.contains("@markPrice") {
        let data: MarkPriceData = serde_json::from_value(msg.data)?;
        return Ok(Some(NormalizedEvent::MarkPrice(RawMarkPrice {
            symbol: data.s,
            mark_price: data.p.parse().unwrap_or(0.0),
            index_price: data.i.parse().unwrap_or(0.0),
            estimated_settle_price: data.p_est.parse().unwrap_or(0.0),
            funding_rate: data.r.parse().unwrap_or(0.0),
            next_funding_time_ms: data.t as i64,
            exchange_event_time_ms: data.e_time as i64,
            recv_time_ns,
            trace_id: hft_common::ids::new_trace_id(),
            schema_version: 1,
        })));
    }
    
    if msg.stream.contains("@openInterest") {
        let data: OpenInterestData = serde_json::from_value(msg.data)?;
        return Ok(Some(NormalizedEvent::OpenInterest(RawOpenInterest {
            symbol: data.s,
            open_interest: data.oo.parse().unwrap_or(0.0),
            exchange_event_time_ms: data.e_time as i64,
            recv_time_ns,
            trace_id: hft_common::ids::new_trace_id(),
            schema_version: 1,
        })));
    }

    if msg.stream.contains("@forceOrder") {
        let data: ForceOrderDataWrapper = serde_json::from_value(msg.data)?;
        let order = data.o;
        return Ok(Some(NormalizedEvent::Liquidation(RawLiquidation {
            symbol: order.s,
            side: if order.side == "BUY" { 1 } else { -1 },
            price: order.p.parse().unwrap_or(0.0),
            orig_qty: order.q.parse().unwrap_or(0.0),
            executed_qty: order.z.parse().unwrap_or(0.0),
            exchange_event_time_ms: data.e_time as i64,
            recv_time_ns,
            trace_id: hft_common::ids::new_trace_id(),
            schema_version: 1,
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

#[derive(Debug, Deserialize)]
struct DepthData {
    #[serde(rename = "E")]
    e_time: u64,
    #[serde(rename = "s")]
    s: String,
    #[serde(rename = "U")]
    u_first: u64,
    #[serde(rename = "u")]
    u_final: u64,
    // Binance sends arrays of [price, qty] arrays
    #[serde(rename = "b")]
    b: Vec<Vec<String>>,
    #[serde(rename = "a")]
    a: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct MarkPriceData {
    #[serde(rename = "E")]
    e_time: u64,
    #[serde(rename = "s")]
    s: String,
    #[serde(rename = "p")]
    p: String,
    #[serde(rename = "i")]
    i: String,
    #[serde(rename = "P")]
    p_est: String,
    #[serde(rename = "r")]
    r: String,
    #[serde(rename = "T")]
    t: u64,
}

#[derive(Debug, Deserialize)]
struct OpenInterestData {
    #[serde(rename = "E")]
    e_time: u64,
    #[serde(rename = "s")]
    s: String,
    #[serde(rename = "oo")]
    oo: String,
}

#[derive(Debug, Deserialize)]
struct ForceOrderDataWrapper {
    #[serde(rename = "E")]
    e_time: u64,
    #[serde(rename = "o")]
    o: ForceOrderData,
}

#[derive(Debug, Deserialize)]
struct ForceOrderData {
    #[serde(rename = "s")]
    s: String,
    #[serde(rename = "S")]
    side: String,
    #[serde(rename = "p")]
    p: String,
    #[serde(rename = "q")]
    q: String,
    #[serde(rename = "z")]
    z: String,
}
