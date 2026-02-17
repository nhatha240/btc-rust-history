use crate::model::Candle;
use anyhow::Result;
use chrono::{TimeZone, Utc};
use clickhouse::{Client, Row};
use serde::Serialize;

pub fn client(url: &str, db: &str, user: Option<&str>, pass: Option<&str>) -> Client {
    let mut c = Client::default().with_url(url).with_database(db);
    if let (Some(u), Some(p)) = (user, pass) { c = c.with_user(u).with_password(p); }
    c
}
#[derive(Row, Serialize)]
struct Candle1mFinalRow {
    symbol: String,
    open_time: chrono::DateTime<chrono::Utc>,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    close_time: chrono::DateTime<chrono::Utc>,
    quote_asset_volume: f64,
    number_of_trades: u64,
    taker_buy_base_asset_volume: f64,
    taker_buy_quote_asset_volume: f64,
}

impl From<&Candle> for Candle1mFinalRow {
    fn from(c: &Candle) -> Self {
        Self {
            symbol: c.symbol.clone(),
            open_time: Utc.timestamp_millis_opt(c.open_time as i64).single().unwrap(),
            open: c.open,
            high: c.high,
            low: c.low,
            close: c.close,
            volume: c.volume,
            close_time: Utc.timestamp_millis_opt(c.close_time as i64).single().unwrap(),
            quote_asset_volume: c.quote_asset_volume,
            number_of_trades: c.number_of_trades,
            taker_buy_base_asset_volume: c.taker_buy_base_asset_volume,
            taker_buy_quote_asset_volume: c.taker_buy_quote_asset_volume,
        }
    }
}

pub async fn insert_candle_1m_final(client: &Client, c: &Candle) -> Result<()> {
    let mut insert = client.insert("db_trading.candles_1m_final")?;
    insert.write(&Candle1mFinalRow::from(c)).await?;
    insert.end().await?;
    Ok(())
}
