use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub exchange: String,        // binance_spot | binance_usdm
    pub market: String,          // spot | usdm
    pub symbol: String,          // BTCUSDT
    pub interval: String,        // 1m | 5m...
    pub open_time: u64,
    pub close_time: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub number_of_trades: u64,
    pub quote_asset_volume: f64,
    pub taker_buy_base_asset_volume: f64,
    pub taker_buy_quote_asset_volume: f64,
    pub is_closed: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Emas { pub ema9:f64, pub ema25:f64, pub ema50:f64, pub ema100:f64, pub ema200:f64 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Rsi { pub rsi14:f64 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Macd { pub macd:f64, pub signal:f64, pub hist:f64 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowMeta { pub n:usize, pub first_open_time_ms:u64, pub last_open_time_ms:u64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureState {
    pub ts_event_ms: u64,
    pub exchange: String,
    pub symbol: String,
    pub interval: String,
    pub seq: u64,
    pub last_close: f64,
    pub ema: Emas,
    pub rsi: Rsi,
    pub macd: Macd,
    pub vwap: f64,
    pub window: WindowMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub decision_id: String,
    pub ts_event_ms: u64,
    pub strategy_id: String,
    pub variant: String,
    pub exchange: String,
    pub symbol: String,
    pub side: String,         // BUY | SELL | FLAT
    pub confidence: f64,
    pub time_in_force_ms: u64,
}
