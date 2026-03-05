//! Kafka producer wrapper — idiomatic send with headers, keyed partitioning.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use rdkafka::config::ClientConfig;
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord};
use tracing::{debug, warn};

use crate::config::KafkaConfig;

/// Kafka producer wrapping [`rdkafka::producer::FutureProducer`].
///
/// `send` awaits broker acknowledgement (subject to `message.timeout.ms`).
/// The inner producer is `Clone`-able (it holds an internal `Arc`), so
/// `KafkaProducer` can be cheaply cloned across tasks.
#[derive(Clone)]
pub struct KafkaProducer {
    inner: FutureProducer,
}

impl KafkaProducer {
    /// Create a producer from the shared [`KafkaConfig`].
    ///
    /// Applies: `bootstrap.servers`, `linger.ms`, `batch.size`,
    /// `compression.type`, `acks`, `enable.idempotence`, `message.timeout.ms`.
    pub fn new(cfg: &KafkaConfig) -> anyhow::Result<Self> {
        let inner: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &cfg.brokers)
            .set("client.id", &cfg.client_id)
            .set("linger.ms", cfg.linger_ms.to_string())
            .set("batch.size", cfg.batch_size.to_string())
            .set("compression.type", &cfg.compression)
            .set("acks", &cfg.acks)
            .set("enable.idempotence", cfg.idempotent.to_string())
            .set("message.timeout.ms", cfg.message_timeout_ms.to_string())
            .create()
            .context("failed to create Kafka producer")?;

        Ok(Self { inner })
    }

    /// Send a message and await broker acknowledgement.
    ///
    /// # Arguments
    /// - `topic`   — destination topic
    /// - `key`     — partition key (routed by `murmur2(key) % partitions`)
    /// - `payload` — raw bytes (typically a serialised protobuf)
    /// - `headers` — optional key→value string headers attached to the message
    ///
    /// # Errors
    /// Returns an error if the local send queue is full or the broker times out.
    pub async fn send(
        &self,
        topic: &str,
        key: &str,
        payload: &[u8],
        headers: Option<&HashMap<String, Vec<u8>>>,
    ) -> anyhow::Result<()> {
        let owned_headers = headers.map(|h| {
            h.iter().fold(OwnedHeaders::new_with_capacity(h.len()), |acc, (k, v)| {
                acc.insert(Header { key: k.as_str(), value: Some(v.as_slice()) })
            })
        });

        let mut record = FutureRecord::to(topic).key(key).payload(payload);
        if let Some(h) = owned_headers {
            record = record.headers(h);
        }

        self.inner
            .send(record, Duration::ZERO)
            .await
            .map(|(partition, offset)| {
                debug!(
                    topic,
                    key,
                    partition,
                    offset,
                    payload_bytes = payload.len(),
                    "message delivered"
                );
            })
            .map_err(|(e, _msg)| {
                warn!(topic, key, "producer send failed: {e}");
                anyhow::anyhow!("kafka send error on topic `{topic}`: {e}")
            })
    }

    /// Like [`send`](Self::send) but accepts a raw byte key.
    ///
    /// Useful for forwarding messages where the key is already `Vec<u8>`
    /// (e.g. re-publishing from a DLQ handler).
    pub async fn send_bytes(
        &self,
        topic: &str,
        key: &[u8],
        payload: &[u8],
        headers: Option<OwnedHeaders>,
    ) -> anyhow::Result<()> {
        let mut record = FutureRecord::to(topic).key(key).payload(payload);
        if let Some(h) = headers {
            record = record.headers(h);
        }

        self.inner
            .send(record, Duration::ZERO)
            .await
            .map(|_| ())
            .map_err(|(e, _msg)| anyhow::anyhow!("kafka send_bytes error on topic `{topic}`: {e}"))
    }
}
