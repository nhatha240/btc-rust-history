//! Dead-letter queue publisher.
//!
//! When a consumer handler fails all retry attempts the message is a "poison
//! pill". Rather than halting the consumer or silently dropping the message,
//! [`DlqPublisher`] republishes it to `{original_topic}{suffix}` (default
//! `".dlq"`) with diagnostic headers so the failure can be investigated later.
//!
//! # Headers injected into every DLQ message
//! | Header key                  | Value                               |
//! |-----------------------------|-------------------------------------|
//! | `dlq.original-topic`        | Topic the message came from         |
//! | `dlq.original-partition`    | Source partition (decimal string)   |
//! | `dlq.original-offset`       | Source offset (decimal string)      |
//! | `dlq.error`                 | Last error message from the handler |
//! | `dlq.failed-at-ms`          | Unix timestamp in ms when it failed |

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use rdkafka::message::{Header, OwnedHeaders};
use tracing::warn;

use crate::config::KafkaConfig;
use crate::consumer::MessageCtx;
use crate::producer::KafkaProducer;

/// Publishes poison messages to a dead-letter topic.
pub struct DlqPublisher {
    producer: KafkaProducer,
    /// Topic suffix appended to the original topic name.
    suffix: String,
}

impl DlqPublisher {
    /// Build a DLQ publisher from the shared config.
    ///
    /// Uses the same producer settings as the service producer
    /// (acks, compression, etc.) so DLQ writes are as reliable as normal writes.
    pub fn new(cfg: &KafkaConfig) -> anyhow::Result<Self> {
        Ok(Self {
            producer: KafkaProducer::new(cfg).context("failed to create DLQ producer")?,
            suffix: cfg.dlq.topic_suffix.clone(),
        })
    }

    /// Publish `ctx` to the dead-letter topic with diagnostic headers.
    ///
    /// The DLQ topic is `{ctx.topic}{self.suffix}`.
    /// The original payload is preserved unchanged — consumers can replay.
    ///
    /// # Errors
    /// Propagates any producer send error.
    pub async fn publish(&self, ctx: &MessageCtx, error: &str) -> anyhow::Result<()> {
        let dlq_topic = format!("{}{}", ctx.topic, self.suffix);

        let failed_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();

        // Build DLQ diagnostic headers, preserving any original headers.
        let headers = ctx
            .headers
            .iter()
            .fold(OwnedHeaders::new_with_capacity(ctx.headers.len() + 5), |acc, (k, v)| {
                acc.insert(Header { key: k.as_str(), value: Some(v.as_slice()) })
            })
            .insert(Header { key: "dlq.original-topic", value: Some(ctx.topic.as_bytes()) })
            .insert(Header {
                key: "dlq.original-partition",
                value: Some(ctx.partition.to_string().as_bytes()),
            })
            .insert(Header {
                key: "dlq.original-offset",
                value: Some(ctx.offset.to_string().as_bytes()),
            })
            .insert(Header { key: "dlq.error", value: Some(error.as_bytes()) })
            .insert(Header { key: "dlq.failed-at-ms", value: Some(failed_at_ms.as_bytes()) });

        let key = ctx.key.as_deref().unwrap_or_default();

        warn!(
            dlq_topic = %dlq_topic,
            original_topic = %ctx.topic,
            partition = ctx.partition,
            offset = ctx.offset,
            error,
            "publishing poison message to DLQ"
        );

        self.producer
            .send_bytes(&dlq_topic, key, &ctx.payload, Some(headers))
            .await
            .with_context(|| format!("DLQ publish failed for topic `{}`", ctx.topic))
    }
}
