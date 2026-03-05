//! Incremental session VWAP (Volume-Weighted Average Price) — O(1) per bar.
//!
//! The session resets at midnight UTC (day boundary).
//! Typical price = (high + low + close) / 3 (standard VWAP convention).
//!
//! VWAP = Σ(typical_price × volume) / Σ(volume)
//!
//! The indicator is considered ready as soon as at least one bar has been
//! processed in the current session (i.e., it always has a value after
//! the first bar).

#[derive(Debug, Clone)]
pub struct VwapState {
    cumulative_pv: f64,
    cumulative_vol: f64,
    session_day: u32,
    bars_in_session: u32,
}

impl VwapState {
    pub fn new() -> Self {
        Self {
            cumulative_pv: 0.0,
            cumulative_vol: 0.0,
            session_day: 0,
            bars_in_session: 0,
        }
    }

    /// Feed a new bar. `open_time_ms` is the bar open timestamp in milliseconds.
    /// Returns the current session VWAP.
    #[inline]
    pub fn update(&mut self, high: f64, low: f64, close: f64, volume: f64, open_time_ms: i64) -> f64 {
        // Detect day boundary and reset session accumulators.
        let day = (open_time_ms / 86_400_000) as u32;
        if day != self.session_day {
            self.cumulative_pv = 0.0;
            self.cumulative_vol = 0.0;
            self.bars_in_session = 0;
            self.session_day = day;
        }

        let typical = (high + low + close) / 3.0;
        self.cumulative_pv += typical * volume;
        self.cumulative_vol += volume;
        self.bars_in_session += 1;

        self.vwap()
    }

    #[inline]
    fn vwap(&self) -> f64 {
        if self.cumulative_vol == 0.0 {
            0.0
        } else {
            self.cumulative_pv / self.cumulative_vol
        }
    }

    #[inline]
    pub fn ready(&self) -> bool {
        self.bars_in_session > 0
    }
}

impl Default for VwapState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_bar_vwap_equals_typical_price() {
        let mut vwap = VwapState::new();
        // high=12, low=8, close=10 → typical=10
        let v = vwap.update(12.0, 8.0, 10.0, 100.0, 0);
        assert!((v - 10.0).abs() < 1e-9, "got {v}");
    }

    #[test]
    fn session_resets_on_new_day() {
        let mut vwap = VwapState::new();
        vwap.update(12.0, 8.0, 10.0, 1.0, 0);
        // Next day (86_400_000 ms later)
        let v = vwap.update(20.0, 16.0, 18.0, 1.0, 86_400_000);
        // Typical = (20+16+18)/3 = 18
        assert!((v - 18.0).abs() < 1e-9, "session reset expected, got {v}");
    }
}
