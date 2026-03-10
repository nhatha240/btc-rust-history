use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct IdempotencyPlanner {
    // key: trace_id:symbol:side -> client_order_id
    seen: Arc<DashMap<String, String>>,
}

impl IdempotencyPlanner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn client_order_id(&self, trace_id: &str, symbol: &str, side: i32) -> String {
        let key = format!("{trace_id}:{symbol}:{side}");
        if let Some(existing) = self.seen.get(&key) {
            return existing.value().clone();
        }
        let id = Uuid::now_v7().to_string();
        self.seen.insert(key, id.clone());
        id
    }
}
