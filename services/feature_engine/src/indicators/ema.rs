//! Incremental Exponential Moving Average — O(1) per update.
//!
//! Uses the standard EMA formula:
//!   alpha = 2 / (period + 1)
//!   ema_t = alpha * price_t + (1 - alpha) * ema_{t-1}
//!
//! The first `period` bars seed the value (first bar is taken as-is, then
//! the EMA formula is applied). The indicator is considered "ready" once
//! at least `period` bars have been processed.

#[derive(Debug, Clone)]
pub struct EmaState {
    alpha: f64,
    value: f64,
    count: u32,
    period: u32,
}

impl EmaState {
    pub fn new(period: u32) -> Self {
        assert!(period >= 1, "EMA period must be >= 1");
        Self {
            alpha: 2.0 / (period as f64 + 1.0),
            value: 0.0,
            count: 0,
            period,
        }
    }

    /// Feed a new price. Returns `Some(ema)` once the warmup period is complete.
    #[inline]
    pub fn update(&mut self, price: f64) -> Option<f64> {
        if self.count == 0 {
            self.value = price;
        } else {
            self.value = self.alpha * price + (1.0 - self.alpha) * self.value;
        }
        self.count += 1;
        self.ready().then_some(self.value)
    }

    /// Feed a new price and return the current EMA regardless of warmup state.
    #[inline]
    pub fn update_raw(&mut self, price: f64) -> f64 {
        self.update(price);
        self.value
    }

    #[inline]
    #[allow(dead_code)]
    pub fn value(&self) -> f64 {
        self.value
    }

    #[inline]
    pub fn ready(&self) -> bool {
        self.count >= self.period
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warmup_then_converges() {
        let mut ema = EmaState::new(3);
        assert_eq!(ema.update(10.0), None);
        assert_eq!(ema.update(10.0), None);
        // 3rd bar completes warmup
        let v = ema.update(10.0).expect("should be ready after period bars");
        assert!((v - 10.0).abs() < 1e-9, "flat series EMA == price");
    }
}
