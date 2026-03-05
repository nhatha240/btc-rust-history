use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct DedupCache {
    entries: Arc<DashMap<String, Instant>>,
    ttl: Duration,
}

impl DedupCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            ttl,
        }
    }

    // Returns true if already seen recently.
    pub fn seen_recently_or_insert(&self, client_order_id: &str) -> bool {
        let now = Instant::now();
        self.cleanup_if_needed(now);

        if let Some(ts) = self.entries.get(client_order_id) {
            if now.duration_since(*ts) <= self.ttl {
                return true;
            }
        }

        self.entries.insert(client_order_id.to_string(), now);
        false
    }

    fn cleanup_if_needed(&self, now: Instant) {
        if self.entries.len() < 10_000 {
            return;
        }
        self.entries.retain(|_, ts| now.duration_since(*ts) <= self.ttl);
    }
}
