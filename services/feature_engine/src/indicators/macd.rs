//! Incremental MACD (Moving Average Convergence Divergence) — O(1) per bar.
//!
//! MACD line   = EMA(close, fast_period) − EMA(close, slow_period)
//! Signal line = EMA(MACD, signal_period)
//! Histogram   = MACD − Signal
//!
//! All three EMAs are updated with a single close price per bar.
//! The indicator is ready once the slow EMA and signal EMA are both warmed up.

use crate::indicators::ema::EmaState;

#[derive(Debug, Clone)]
pub struct MacdState {
    fast: EmaState,
    slow: EmaState,
    signal: EmaState,
}

#[derive(Debug, Clone, Copy)]
pub struct MacdValues {
    pub macd: f64,
    pub signal: f64,
    pub hist: f64,
}

impl MacdState {
    /// Create with standard defaults: fast=12, slow=26, signal=9.
    pub fn new(fast_period: u32, slow_period: u32, signal_period: u32) -> Self {
        Self {
            fast: EmaState::new(fast_period),
            slow: EmaState::new(slow_period),
            signal: EmaState::new(signal_period),
        }
    }

    /// Feed a new close price. Returns `Some(MacdValues)` once fully warmed up.
    #[inline]
    pub fn update(&mut self, close: f64) -> Option<MacdValues> {
        let fast_val = self.fast.update_raw(close);
        let slow_ready = self.slow.ready();
        let slow_val = self.slow.update_raw(close);

        // Only feed the signal EMA once the slow EMA is ready.
        if !slow_ready {
            return None;
        }

        let macd = fast_val - slow_val;
        let signal_ready = self.signal.ready();
        let sig_val = self.signal.update_raw(macd);

        if !signal_ready {
            return None;
        }

        Some(MacdValues {
            macd,
            signal: sig_val,
            hist: macd - sig_val,
        })
    }

    #[inline]
    pub fn ready(&self) -> bool {
        self.slow.ready() && self.signal.ready()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_series_macd_zero() {
        let mut macd = MacdState::new(12, 26, 9);
        let mut last = None;
        for _ in 0..50 {
            last = macd.update(100.0);
        }
        let v = last.expect("should have values after 50 bars");
        assert!(v.macd.abs() < 1e-9, "flat series MACD ≈ 0, got {}", v.macd);
        assert!(v.signal.abs() < 1e-9, "flat series signal ≈ 0");
        assert!(v.hist.abs() < 1e-9, "flat series hist ≈ 0");
    }
}
