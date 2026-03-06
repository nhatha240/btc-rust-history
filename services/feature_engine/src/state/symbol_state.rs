//! Per-symbol incremental indicator state.
//!
//! `SymbolState` owns all indicator accumulators for one trading pair.
//! `update()` is called on every closed candle and returns the computed
//! feature vector along with a quality bitmask.

use crate::indicators::{
    ema::EmaState,
    macd::MacdState,
    rsi::RsiState,
    vwap::VwapState,
};

/// Bitmask bits for the `x-quality` Kafka header.
pub const QUALITY_EMA_FAST: u8 = 1 << 0;
pub const QUALITY_EMA_SLOW: u8 = 1 << 1;
pub const QUALITY_RSI: u8      = 1 << 2;
pub const QUALITY_MACD: u8     = 1 << 3;
pub const QUALITY_VWAP: u8     = 1 << 4;
/// All indicators ready.
#[allow(dead_code)]
pub const QUALITY_ALL: u8 = QUALITY_EMA_FAST | QUALITY_EMA_SLOW | QUALITY_RSI | QUALITY_MACD | QUALITY_VWAP;

/// Computed feature values for one bar.
#[derive(Debug, Clone)]
pub struct FeatureVector {
    pub symbol: String,
    /// Bar close time in Unix milliseconds.
    pub ts: i64,
    pub ema_fast: f64,
    pub ema_slow: f64,
    pub rsi: f64,
    pub macd: f64,
    pub macd_signal: f64,
    pub macd_hist: f64,
    pub vwap: f64,
    /// Bitmask: bit is set when the corresponding indicator is warmed up.
    pub quality: u8,
}

/// All incremental indicator state for a single symbol.
#[derive(Debug)]
pub struct SymbolState {
    pub symbol: String,
    ema_fast: EmaState,
    ema_slow: EmaState,
    rsi: RsiState,
    macd: MacdState,
    vwap: VwapState,
}

impl SymbolState {
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        // Placeholder for the actual implementation of len()
        // The original instruction had a malformed snippet for the body.
        // Assuming it should return a usize, e.g., 0 for now.
        0
    }

    pub fn new(symbol: impl Into<String>, ema_fast: u32, ema_slow: u32, rsi_period: u32, signal_period: u32) -> Self {
        Self {
            symbol: symbol.into(),
            ema_fast: EmaState::new(ema_fast),
            ema_slow: EmaState::new(ema_slow),
            rsi: RsiState::new(rsi_period),
            macd: MacdState::new(ema_fast, ema_slow, signal_period),
            vwap: VwapState::new(),
        }
    }

    /// Update all indicators with a new closed candle. Always returns a
    /// `FeatureVector`; check `quality` to know which indicators are ready.
    #[allow(dead_code)]
    pub fn value(&self) -> f64 {
        // Placeholder for the actual implementation of value()
        // The original instruction had a malformed snippet for the body.
        // Assuming it should return a f64, e.g., 0.0 for now.
        0.0
    }

    pub fn update(
        &mut self,
        open_time_ms: i64,
        close_time_ms: i64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
    ) -> FeatureVector {
        let mut quality: u8 = 0;

        let ema_fast = self.ema_fast.update_raw(close);
        if self.ema_fast.ready() {
            quality |= QUALITY_EMA_FAST;
        }

        let ema_slow = self.ema_slow.update_raw(close);
        if self.ema_slow.ready() {
            quality |= QUALITY_EMA_SLOW;
        }

        let rsi = self.rsi.update(close).unwrap_or(50.0);
        if self.rsi.ready() {
            quality |= QUALITY_RSI;
        }

        let macd_vals = self.macd.update(close);
        let (macd, macd_signal, macd_hist) = macd_vals
            .map(|v| (v.macd, v.signal, v.hist))
            .unwrap_or((0.0, 0.0, 0.0));
        if self.macd.ready() {
            quality |= QUALITY_MACD;
        }

        let vwap = self.vwap.update(high, low, close, volume, open_time_ms);
        if self.vwap.ready() {
            quality |= QUALITY_VWAP;
        }

        FeatureVector {
            symbol: self.symbol.clone(),
            ts: close_time_ms,
            ema_fast,
            ema_slow,
            rsi,
            macd,
            macd_signal,
            macd_hist,
            vwap,
            quality,
        }
    }
}
