//! Service configuration loaded entirely from environment variables.

use anyhow::Result;
use hft_common::config::{env_or, env_parse, load_dotenv};

#[derive(Debug, Clone)]
pub struct Config {
    // ── Kafka ──────────────────────────────────────────────────────────────────
    pub kafka_brokers: String,
    pub kafka_group_id: String,
    pub kafka_client_id: String,

    // ── Topics ─────────────────────────────────────────────────────────────────
    /// Input topic: closed 1-minute candles. Default: `TOPIC_CANDLES_1M`
    pub topic_candles: String,
    /// Output topic: computed feature vectors. Default: `md.features.live`
    pub topic_features: String,

    // ── Indicator parameters ───────────────────────────────────────────────────
    pub ema_fast_period: u32,
    pub ema_slow_period: u32,
    pub rsi_period: u32,
    pub macd_signal_period: u32,

    // ── Publish throttle ───────────────────────────────────────────────────────
    /// Minimum milliseconds between feature publishes per symbol.
    /// 0 = disabled (always publish on closed candle).
    pub throttle_ms: u64,

    // ── HTTP health check ──────────────────────────────────────────────────────
    pub health_port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        load_dotenv();
        Ok(Self {
            kafka_brokers:      env_or("KAFKA_BROKERS",      "redpanda:9092"),
            kafka_group_id:     env_or("KAFKA_GROUP_ID",     "feature-engine"),
            kafka_client_id:    env_or("KAFKA_CLIENT_ID",    "feature_state"),
            topic_candles:      env_or("TOPIC_CANDLES",      "TOPIC_CANDLES_1M"),
            topic_features:     env_or("TOPIC_FEATURES",     "md.features.live"),
            ema_fast_period:    env_parse("EMA_FAST_PERIOD",    12)?,
            ema_slow_period:    env_parse("EMA_SLOW_PERIOD",    26)?,
            rsi_period:         env_parse("RSI_PERIOD",         14)?,
            macd_signal_period: env_parse("MACD_SIGNAL_PERIOD",  9)?,
            throttle_ms:        env_parse("FEATURE_THROTTLE_MS", 0u64)?,
            health_port:        env_parse("HEALTH_PORT",         8080u16)?,
        })
    }
}
