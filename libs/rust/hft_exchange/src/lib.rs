pub mod binance;
pub mod rate_limit;

pub use binance::{BinanceRestClient, BinanceWsStream};
pub use rate_limit::RateLimiter;

// High-level Trait/Wrapper could be added here for multi-exchange support
pub struct ExchangeClient {
    pub binance: BinanceRestClient,
}

impl ExchangeClient {
    pub fn new(binance: BinanceRestClient) -> Self {
        Self { binance }
    }
}
