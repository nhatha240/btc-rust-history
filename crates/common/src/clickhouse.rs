use anyhow::Result;
use clickhouse::{Client, Row};
use serde::Serialize;
use crate::model::Candle;

pub fn client(url: &str, db: &str, user: Option<&str>, pass: Option<&str>) -> Client {
    let mut c = Client::default().with_url(url).with_database(db);
    if let (Some(u), Some(p)) = (user, pass) { c = c.with_user(u).with_password(p); }
    c
}

#[derive(Row, Serialize)]
struct Candle1mLiveRow {
    exchange:String, symbol:String, interval:String,
    open_time_ms:u64, close_time_ms:u64,
    open:f64, high:f64, low:f64, close:f64,
    volume:f64, trade_count:u64, quote_volume:f64,
    taker_buy_base_volume:f64, taker_buy_quote_volume:f64,
    is_closed:u8, ver:u64
}

pub async fn insert_candles_1m_live(ch: &Client, batch: &[Candle]) -> Result<()> {
    let mut insert = ch.insert("db_trading.candles_1m_live")?;
    for c in batch {
        insert.write(&Candle1mLiveRow{
            exchange:c.exchange.clone(), symbol:c.symbol.clone(), interval:c.interval.clone(),
            open_time_ms:c.open_time_ms, close_time_ms:c.close_time_ms,
            open:c.open, high:c.high, low:c.low, close:c.close,
            volume:c.volume, trade_count:c.trade_count, quote_volume:c.quote_volume,
            taker_buy_base_volume:c.taker_buy_base_volume, taker_buy_quote_volume:c.taker_buy_quote_volume,
            is_closed: if c.is_closed {1} else {0}, ver:c.close_time_ms
        }).await?;
    }
    insert.end().await?;
    Ok(())
}
