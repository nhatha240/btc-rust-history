use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct BinanceKline(
    i64,    // open time ms
    String, // open
    String, // high
    String, // low
    String, // close
    String, // volume (base asset)
    i64,    // close time ms
    String, // quote asset volume
    u64,    // number of trades
    String, // taker buy base asset volume
    String, // taker buy quote asset volume
    String  // ignore
);

impl BinanceKline {
    pub fn to_candle(&self, exchange: &str, symbol: &str, interval: &str) -> common::model::Candle {
        common::model::Candle {
            exchange: exchange.to_string(),
            market: "spot".into(),
            symbol: symbol.into(),
            interval: interval.into(),
            open_time_ms: self.0 as u64,
            close_time_ms: self.6 as u64,
            open: self.1.parse().unwrap_or(0.0),
            high: self.2.parse().unwrap_or(0.0),
            low: self.3.parse().unwrap_or(0.0),
            close: self.4.parse().unwrap_or(0.0),
            volume: self.5.parse().unwrap_or(0.0),
            trade_count: self.8 as u64,
            quote_asset_volume: self.7.parse().unwrap_or(0.0),
            taker_buy_base_asset_volume: self.9.parse().unwrap_or(0.0),
            taker_buy_quote_asset_volume: self.10.parse().unwrap_or(0.0),
        }
    }
}
