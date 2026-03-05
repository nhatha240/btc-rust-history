use tokio::time::{sleep, Duration, Instant};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct RateLimiter {
    tokens: Mutex<f64>,
    max_tokens: f64,
    refill_rate: f64,
    last_update: Mutex<Instant>,
}

impl RateLimiter {
    pub fn new(max_tokens: f64, refill_rate_per_sec: f64) -> Arc<Self> {
        Arc::new(Self {
            tokens: Mutex::new(max_tokens),
            max_tokens,
            refill_rate: refill_rate_per_sec,
            last_update: Mutex::new(Instant::now()),
        })
    }

    pub async fn acquire(&self, amount: f64) {
        loop {
            let mut last_update = self.last_update.lock().await;
            let mut tokens = self.tokens.lock().await;

            let now = Instant::now();
            let elapsed = now.duration_since(*last_update).as_secs_f64();
            *tokens = (*tokens + elapsed * self.refill_rate).min(self.max_tokens);
            *last_update = now;

            if *tokens >= amount {
                *tokens -= amount;
                return;
            }

            let wait_time = (amount - *tokens) / self.refill_rate;
            drop(tokens);
            drop(last_update);
            sleep(Duration::from_secs_f64(wait_time)).await;
        }
    }
}
