//! Kafka configuration: shared settings, per-role tuning presets, env loading.

use hft_common::config::{env_or, env_parse, load_dotenv};

// ── Retry policy ──────────────────────────────────────────────────────────────

/// Exponential backoff retry policy for consumer handlers.
///
/// `max_retries = 3` means the handler is called up to **4 times total**
/// (1 initial attempt + 3 retries).
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Number of retry attempts after the first failure (0 = no retries).
    pub max_retries: u32,
    /// Backoff applied before attempt 1 (doubles each attempt).
    pub initial_backoff_ms: u64,
    /// Cap on the exponential growth.
    pub max_backoff_ms: u64,
}

impl RetryPolicy {
    /// Compute backoff duration for a given `attempt` index (0-based).
    ///
    /// Uses binary exponential backoff capped at `max_backoff_ms`.
    pub fn backoff_ms(&self, attempt: u32) -> u64 {
        // Cap the shift to avoid u64 overflow (2^10 = 1024× max multiplier)
        let factor = 1u64 << attempt.min(10);
        self.initial_backoff_ms.saturating_mul(factor).min(self.max_backoff_ms)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 5_000,
        }
    }
}

// ── DLQ config ────────────────────────────────────────────────────────────────

/// Dead-letter queue settings.
#[derive(Debug, Clone)]
pub struct DlqConfig {
    /// Publish poison messages to a DLQ topic.
    pub enabled: bool,
    /// Appended to the original topic name to form the DLQ topic.
    /// Default: `".dlq"` → `"orders.v1.dlq"`
    pub topic_suffix: String,
    /// If `true`, a DLQ publish failure is fatal (stops the consumer loop).
    /// If `false`, log the error and commit past the poison message anyway.
    pub halt_on_failure: bool,
}

impl Default for DlqConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            topic_suffix: ".dlq".to_owned(),
            halt_on_failure: false,
        }
    }
}

// ── KafkaConfig ───────────────────────────────────────────────────────────────

/// Unified Kafka configuration for both producers and consumers.
///
/// Build via [`KafkaConfig::from_env`] in production or use a preset:
/// - [`KafkaConfig::reliable`]       — acks=all, idempotent, strict retries
/// - [`KafkaConfig::low_latency`]    — linger=0, acks=1
/// - [`KafkaConfig::high_throughput`] — batch + lz4 compression
#[derive(Debug, Clone)]
pub struct KafkaConfig {
    // ── Connection ────────────────────────────────────────────────────────────
    /// Comma-separated broker list. Env: `KAFKA_BROKERS`
    pub brokers: String,
    /// Consumer group ID. Env: `KAFKA_GROUP_ID`
    pub group_id: String,
    /// Client identifier shown in broker logs. Env: `KAFKA_CLIENT_ID`
    pub client_id: String,

    // ── Producer tuning ───────────────────────────────────────────────────────
    /// Milliseconds to wait before sending a non-full batch (latency vs throughput).
    pub linger_ms: u64,
    /// Maximum size of a message batch in bytes.
    pub batch_size: usize,
    /// Compression codec: `"lz4"` | `"snappy"` | `"gzip"` | `"none"`
    pub compression: String,
    /// Broker acknowledgement level: `"all"` | `"1"` | `"0"`
    pub acks: String,
    /// Enable idempotent producer (exactly-once delivery within a session).
    pub idempotent: bool,
    /// Milliseconds before an unacknowledged message is considered failed.
    pub message_timeout_ms: u64,

    // ── Consumer tuning ───────────────────────────────────────────────────────
    /// Consumer session timeout in ms (broker evicts if no heartbeat).
    pub session_timeout_ms: u64,
    /// Max ms between polls before the consumer is considered dead.
    pub max_poll_interval_ms: u64,
    /// Maximum bytes fetched per poll request.
    pub fetch_max_bytes: usize,
    /// Where to start reading when no committed offset exists: `"earliest"` | `"latest"`
    pub auto_offset_reset: String,

    // ── Cross-cutting ─────────────────────────────────────────────────────────
    pub retry: RetryPolicy,
    pub dlq: DlqConfig,
}

impl KafkaConfig {
    /// Load from environment variables with safe defaults.
    ///
    /// | Env var                    | Default               |
    /// |----------------------------|-----------------------|
    /// | `KAFKA_BROKERS`            | `redpanda:9092`       |
    /// | `KAFKA_GROUP_ID`           | `hft-default-group`   |
    /// | `KAFKA_CLIENT_ID`          | `hft`                 |
    /// | `KAFKA_LINGER_MS`          | `5`                   |
    /// | `KAFKA_BATCH_SIZE`         | `65536`               |
    /// | `KAFKA_COMPRESSION`        | `lz4`                 |
    /// | `KAFKA_ACKS`               | `all`                 |
    /// | `KAFKA_IDEMPOTENT`         | `true`                |
    /// | `KAFKA_MSG_TIMEOUT_MS`     | `5000`                |
    /// | `KAFKA_SESSION_TIMEOUT_MS` | `30000`               |
    /// | `KAFKA_MAX_POLL_INTERVAL`  | `300000`              |
    /// | `KAFKA_FETCH_MAX_BYTES`    | `52428800` (50 MiB)   |
    /// | `KAFKA_AUTO_OFFSET_RESET`  | `earliest`            |
    /// | `KAFKA_MAX_RETRIES`        | `3`                   |
    /// | `KAFKA_RETRY_BACKOFF_MS`   | `100`                 |
    /// | `KAFKA_RETRY_MAX_BACKOFF`  | `5000`                |
    /// | `KAFKA_DLQ_ENABLED`        | `false`               |
    /// | `KAFKA_DLQ_SUFFIX`         | `.dlq`                |
    /// | `KAFKA_DLQ_HALT`           | `false`               |
    pub fn from_env() -> anyhow::Result<Self> {
        load_dotenv();
        Ok(Self {
            brokers: env_or("KAFKA_BROKERS", "redpanda:9092"),
            group_id: env_or("KAFKA_GROUP_ID", "hft-default-group"),
            client_id: env_or("KAFKA_CLIENT_ID", "hft"),
            linger_ms: env_parse("KAFKA_LINGER_MS", 5)?,
            batch_size: env_parse("KAFKA_BATCH_SIZE", 65_536)?,
            compression: env_or("KAFKA_COMPRESSION", "lz4"),
            acks: env_or("KAFKA_ACKS", "all"),
            idempotent: env_parse("KAFKA_IDEMPOTENT", true)?,
            message_timeout_ms: env_parse("KAFKA_MSG_TIMEOUT_MS", 5_000)?,
            session_timeout_ms: env_parse("KAFKA_SESSION_TIMEOUT_MS", 30_000)?,
            max_poll_interval_ms: env_parse("KAFKA_MAX_POLL_INTERVAL", 300_000)?,
            fetch_max_bytes: env_parse("KAFKA_FETCH_MAX_BYTES", 52_428_800)?,
            auto_offset_reset: env_or("KAFKA_AUTO_OFFSET_RESET", "earliest"),
            retry: RetryPolicy {
                max_retries: env_parse("KAFKA_MAX_RETRIES", 3)?,
                initial_backoff_ms: env_parse("KAFKA_RETRY_BACKOFF_MS", 100)?,
                max_backoff_ms: env_parse("KAFKA_RETRY_MAX_BACKOFF", 5_000)?,
            },
            dlq: DlqConfig {
                enabled: env_parse("KAFKA_DLQ_ENABLED", false)?,
                topic_suffix: env_or("KAFKA_DLQ_SUFFIX", ".dlq"),
                halt_on_failure: env_parse("KAFKA_DLQ_HALT", false)?,
            },
        })
    }

    /// Preset: maximum reliability (acks=all, idempotent, aggressive retries).
    /// Use for OMS topics: `orders.v1`, `orders.approved.v1`.
    pub fn reliable(brokers: impl Into<String>, group_id: impl Into<String>) -> Self {
        Self {
            brokers: brokers.into(),
            group_id: group_id.into(),
            client_id: "hft".to_owned(),
            linger_ms: 5,
            batch_size: 65_536,
            compression: "lz4".to_owned(),
            acks: "all".to_owned(),
            idempotent: true,
            message_timeout_ms: 10_000,
            session_timeout_ms: 30_000,
            max_poll_interval_ms: 300_000,
            fetch_max_bytes: 52_428_800,
            auto_offset_reset: "earliest".to_owned(),
            retry: RetryPolicy {
                max_retries: 5,
                initial_backoff_ms: 200,
                max_backoff_ms: 10_000,
            },
            dlq: DlqConfig {
                enabled: true,
                topic_suffix: ".dlq".to_owned(),
                halt_on_failure: false,
            },
        }
    }

    /// Preset: minimum latency (linger=0, acks=1).
    /// Use for signal / feature topics where stale > lost.
    pub fn low_latency(brokers: impl Into<String>, group_id: impl Into<String>) -> Self {
        Self {
            brokers: brokers.into(),
            group_id: group_id.into(),
            client_id: "hft".to_owned(),
            linger_ms: 0,
            batch_size: 16_384,
            compression: "none".to_owned(),
            acks: "1".to_owned(),
            idempotent: false,
            message_timeout_ms: 3_000,
            session_timeout_ms: 10_000,
            max_poll_interval_ms: 60_000,
            fetch_max_bytes: 10_485_760,
            auto_offset_reset: "latest".to_owned(),
            retry: RetryPolicy {
                max_retries: 1,
                initial_backoff_ms: 50,
                max_backoff_ms: 500,
            },
            dlq: DlqConfig::default(),
        }
    }

    /// Preset: maximum throughput (larger batches, lz4, longer linger).
    /// Use for candle / feature ingestion with high message rates.
    pub fn high_throughput(brokers: impl Into<String>, group_id: impl Into<String>) -> Self {
        Self {
            brokers: brokers.into(),
            group_id: group_id.into(),
            client_id: "hft".to_owned(),
            linger_ms: 20,
            batch_size: 524_288,
            compression: "lz4".to_owned(),
            acks: "1".to_owned(),
            idempotent: false,
            message_timeout_ms: 5_000,
            session_timeout_ms: 30_000,
            max_poll_interval_ms: 300_000,
            fetch_max_bytes: 104_857_600,
            auto_offset_reset: "earliest".to_owned(),
            retry: RetryPolicy::default(),
            dlq: DlqConfig::default(),
        }
    }
}
