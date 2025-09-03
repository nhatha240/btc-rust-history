/// Stateless EMA step using smoothing factor 2/(n+1).
#[inline]
pub fn ema_next(prev: Option<f64>, price: f64, period: f64) -> f64 {
    let k = 2.0 / (period + 1.0);
    match prev { Some(p) => p + k * (price - p), None => price }
}

/// Wilder RSI incremental accumulator.
#[derive(Default, Debug, Clone)]
pub struct RsiAccum {
    pub period: usize,
    pub avg_gain: f64,
    pub avg_loss: f64,
    pub prev_close: Option<f64>,
    pub n: usize,
}
impl RsiAccum {
    pub fn new(period: usize) -> Self { Self { period, ..Default::default() } }
    pub fn next(&mut self, close: f64) -> f64 {
        self.n += 1;
        if let Some(pc) = self.prev_close.replace(close) {
            let ch = close - pc;
            let gain = ch.max(0.0);
            let loss = (-ch).max(0.0);
            if self.n <= self.period {
                // seed with SMA
                self.avg_gain = (self.avg_gain * (self.n as f64 - 1.0) + gain) / self.n as f64;
                self.avg_loss = (self.avg_loss * (self.n as f64 - 1.0) + loss) / self.n as f64;
            } else {
                self.avg_gain = (self.avg_gain * (self.period as f64 - 1.0) + gain) / self.period as f64;
                self.avg_loss = (self.avg_loss * (self.period as f64 - 1.0) + loss) / self.period as f64;
            }
            let rs = if self.avg_loss == 0.0 { return 100.0 } else { self.avg_gain / self.avg_loss };
            100.0 - (100.0 / (1.0 + rs))
        } else { 50.0 }
    }
}

/// MACD (12,26,9) incremental state.
#[derive(Default, Debug, Clone)]
pub struct MacdAccum {
    pub ema12: Option<f64>,
    pub ema26: Option<f64>,
    pub signal: Option<f64>,
}
impl MacdAccum {
    pub fn next(&mut self, close: f64) -> (f64, f64, f64) {
        self.ema12 = Some(super::indicators::ema_next(self.ema12, close, 12.0));
        self.ema26 = Some(super::indicators::ema_next(self.ema26, close, 26.0));
        let macd = self.ema12.unwrap() - self.ema26.unwrap();
        self.signal = Some(super::indicators::ema_next(self.signal, macd, 9.0));
        let hist = macd - self.signal.unwrap();
        (macd, self.signal.unwrap(), hist)
    }
}
