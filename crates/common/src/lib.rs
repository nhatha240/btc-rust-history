pub mod model;        // domain models (Model)
pub mod event;        // topics + message contracts (Module: transport)
pub mod indicators;   // technical indicators (Feature)
pub mod kafka;        // Kafka helpers (Service infra)
pub mod clickhouse;   // ClickHouse helpers (Service infra)

pub use model::{Candle, WindowMeta,FeatureState };
pub use event::{
    TOPIC_CANDLES_1M, TOPIC_FEATURE_STATE, TOPIC_SIGNAL_DECISION,
    key_symbol, key_state, Side, CandleEvent, Decision,
};
pub use indicators::{ema_next, RsiAccum, MacdAccum};

