use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Backoff(Duration),
}

#[derive(Debug, Clone)]
pub struct ReconnectController {
    base_ms: u64,
    max_ms: u64,
    attempt: u32,
    reconnect_count: u64,
}

impl ReconnectController {
    pub fn new(base_ms: u64, max_ms: u64) -> Self {
        Self {
            base_ms: base_ms.max(100),
            max_ms: max_ms.max(base_ms),
            attempt: 0,
            reconnect_count: 0,
        }
    }

    pub fn reconnect_count(&self) -> u64 {
        self.reconnect_count
    }

    pub fn on_connected(&mut self) -> ConnectionState {
        self.attempt = 0;
        ConnectionState::Connected
    }

    pub fn on_disconnected(&mut self) -> ConnectionState {
        self.reconnect_count = self.reconnect_count.saturating_add(1);
        self.attempt = self.attempt.saturating_add(1);
        let exp = 1u64 << self.attempt.min(10);
        let delay_ms = self.base_ms.saturating_mul(exp).min(self.max_ms);
        ConnectionState::Backoff(Duration::from_millis(delay_ms))
    }

    pub fn connecting(&self) -> ConnectionState {
        ConnectionState::Connecting
    }
}
