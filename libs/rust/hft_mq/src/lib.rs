//! `hft_mq` — standardised Kafka/Redpanda producer & consumer for HFT services.
//!
//! # Why this crate exists
//! Every service rolling its own `rdkafka` setup leads to:
//! - Inconsistent commit behaviour (auto vs manual, wrong ordering)
//! - Inconsistent retry / backoff policies
//! - No dead-letter queue (DLQ) discipline
//! - Copy-pasted `ClientConfig` with subtle differences
//!
//! `hft_mq` enforces:
//! - **Producer**: linger, compression, idempotence, acks-all by default
//! - **Consumer**: `enable.auto.commit=false`, manual commit *after* handler
//! - **Retry**: exponential backoff, configurable max attempts
//! - **DLQ**: poison messages forwarded to `{topic}.dlq` with error headers
//!
//! # Quick start
//! ```rust,ignore
//! use hft_mq::{KafkaConfig, KafkaConsumer, KafkaProducer};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let cfg = KafkaConfig::from_env()?;
//!
//!     // Producer
//!     let producer = KafkaProducer::new(&cfg)?;
//!     producer.send("orders.v1", "BTCUSDT", b"<proto bytes>", None).await?;
//!
//!     // Consumer — commits only after successful handler
//!     let consumer = KafkaConsumer::new(cfg, &["orders.v1"])?;
//!     consumer.run(|ctx| async move {
//!         tracing::info!(offset = ctx.offset, "got message");
//!         Ok(())
//!     }).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod commit;
pub mod config;
pub mod consumer;
pub mod dlq;
pub mod producer;

// ── Re-exports ────────────────────────────────────────────────────────────────
pub use config::{DlqConfig, KafkaConfig, RetryPolicy};
pub use consumer::{KafkaConsumer, MessageCtx};
pub use dlq::DlqPublisher;
pub use producer::KafkaProducer;
