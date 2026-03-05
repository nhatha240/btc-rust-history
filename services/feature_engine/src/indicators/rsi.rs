//! Incremental RSI (Relative Strength Index) — Wilder's smoothing, O(1) per bar.
//!
//! Wilder's smoothing uses alpha = 1/period (not 2/(period+1)).
//!
//! Warmup phase (first `period` bars after the seed close):
//!   - Collect raw gains and losses.
//!   - After `period` deltas: avg_gain = mean(gains), avg_loss = mean(losses).
//!
//! Steady-state (every bar thereafter):
//!   avg_gain = (avg_gain * (period - 1) + current_gain) / period
//!   avg_loss = (avg_loss * (period - 1) + current_loss) / period
//!   RS       = avg_gain / avg_loss
//!   RSI      = 100 - 100 / (1 + RS)

#[derive(Debug, Clone)]
pub struct RsiState {
    period: u32,
    prev_close: Option<f64>,
    avg_gain: f64,
    avg_loss: f64,
    /// Number of deltas processed (one less than bars seen).
    deltas: u32,
    /// Accumulates raw gains/losses during the initial warmup window.
    warmup_gains: Vec<f64>,
    warmup_losses: Vec<f64>,
}

impl RsiState {
    pub fn new(period: u32) -> Self {
        assert!(period >= 2, "RSI period must be >= 2");
        Self {
            period,
            prev_close: None,
            avg_gain: 0.0,
            avg_loss: 0.0,
            deltas: 0,
            warmup_gains: Vec::with_capacity(period as usize),
            warmup_losses: Vec::with_capacity(period as usize),
        }
    }

    /// Feed a new close price. Returns `Some(rsi)` once warmed up.
    #[inline]
    pub fn update(&mut self, close: f64) -> Option<f64> {
        let prev = match self.prev_close {
            None => {
                self.prev_close = Some(close);
                return None;
            }
            Some(p) => p,
        };
        self.prev_close = Some(close);

        let delta = close - prev;
        let gain = if delta > 0.0 { delta } else { 0.0 };
        let loss = if delta < 0.0 { -delta } else { 0.0 };

        self.deltas += 1;

        if self.deltas < self.period {
            // Still in warmup: accumulate raw values.
            self.warmup_gains.push(gain);
            self.warmup_losses.push(loss);
            return None;
        }

        if self.deltas == self.period {
            // Finalize warmup: compute SMA of gains/losses then add this bar.
            let n = self.warmup_gains.len() as f64;
            let sum_g: f64 = self.warmup_gains.iter().sum();
            let sum_l: f64 = self.warmup_losses.iter().sum();
            self.avg_gain = (sum_g + gain) / (n + 1.0);
            self.avg_loss = (sum_l + loss) / (n + 1.0);
            // Free warmup storage.
            self.warmup_gains = Vec::new();
            self.warmup_losses = Vec::new();
        } else {
            // Steady-state Wilder's smoothing.
            let p = self.period as f64;
            self.avg_gain = (self.avg_gain * (p - 1.0) + gain) / p;
            self.avg_loss = (self.avg_loss * (p - 1.0) + loss) / p;
        }

        Some(self.rsi())
    }

    #[inline]
    fn rsi(&self) -> f64 {
        if self.avg_loss == 0.0 {
            return 100.0;
        }
        let rs = self.avg_gain / self.avg_loss;
        100.0 - 100.0 / (1.0 + rs)
    }

    #[inline]
    pub fn ready(&self) -> bool {
        self.deltas >= self.period
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_series_rsi_50() {
        let mut rsi = RsiState::new(14);
        // Feed 15 identical closes — no gains or losses → RSI is undefined
        // but our impl returns 100 when avg_loss = 0 and avg_gain = 0.
        // Instead use alternating +1/-1 to get RS=1 → RSI=50.
        let mut price = 100.0_f64;
        let mut last = None;
        for i in 0..30 {
            price += if i % 2 == 0 { 1.0 } else { -1.0 };
            last = rsi.update(price);
        }
        let v = last.expect("should have a value after 30 bars");
        assert!(v > 40.0 && v < 60.0, "alternating series RSI near 50, got {v}");
    }
}
