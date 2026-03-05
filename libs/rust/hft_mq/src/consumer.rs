//! Consumer wrapper + safe poll → decode → handle → commit loop.
//!
//! # Guarantees
//! - `enable.auto.commit = false` — offsets are never committed automatically.
//! - An offset is committed **only after** the handler returns `Ok(())`.
//! - On handler failure the message is retried with exponential backoff.
//! - After all retries are exhausted the message is forwarded to the DLQ
//!   (if configured) and the offset is committed to advance past it.
//!
//! # Backpressure
//! The run loop is intentionally sequential: one message is fully processed
//! (including retries) before the next is polled. This provides natural
//! backpressure — the consumer pauses if the handler is slow, which lets
//! Redpanda apply flow control. For parallelism, run multiple consumer
//! instances with the same `group_id`.

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use anyhow::Context;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{Headers, Message};
use rdkafka::Timestamp;
use tracing::{debug, error, info, info_span, warn, Instrument};

use crate::commit::commit_offset;
use crate::config::KafkaConfig;
use crate::dlq::DlqPublisher;

// ── MessageCtx ────────────────────────────────────────────────────────────────

/// Owned, cloneable view of a Kafka message passed to every handler.
#[derive(Debug, Clone)]
pub struct MessageCtx {
    /// Topic the message was consumed from.
    pub topic: String,
    /// Partition key bytes (may be empty if the producer used no key).
    pub key: Option<Vec<u8>>,
    /// Raw message payload (typically a serialised protobuf).
    pub payload: Vec<u8>,
    /// All headers attached to the message, decoded as UTF-8 keys / raw bytes.
    pub headers: HashMap<String, Vec<u8>>,
    /// Broker-assigned creation timestamp in milliseconds, if available.
    pub timestamp_ms: Option<i64>,
    /// Partition the message was read from.
    pub partition: i32,
    /// Message offset within the partition.
    pub offset: i64,
}

impl MessageCtx {
    /// Attempt to interpret the key as a UTF-8 string (e.g. symbol).
    pub fn key_str(&self) -> Option<&str> {
        self.key.as_deref().and_then(|b| std::str::from_utf8(b).ok())
    }

    /// Retrieve a header value by name.
    pub fn header(&self, name: &str) -> Option<&[u8]> {
        self.headers.get(name).map(Vec::as_slice)
    }

    /// Build an owned `MessageCtx` from an rdkafka `BorrowedMessage`.
    ///
    /// All data is copied out so the message reference is not held across
    /// await points or retry loops.
    pub(crate) fn from_borrowed(m: &rdkafka::message::BorrowedMessage<'_>) -> Self {
        let headers = m
            .headers()
            .map(|h| {
                h.iter()
                    .map(|header| (header.key.to_owned(), header.value.unwrap_or(&[]).to_vec()))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();

        let timestamp_ms = match m.timestamp() {
            Timestamp::CreateTime(ms) | Timestamp::LogAppendTime(ms) => Some(ms),
            Timestamp::NotAvailable => None,
        };

        MessageCtx {
            topic: m.topic().to_owned(),
            key: m.key().map(|b| b.to_vec()),
            payload: m.payload().unwrap_or(&[]).to_vec(),
            headers,
            timestamp_ms,
            partition: m.partition(),
            offset: m.offset(),
        }
    }
}

// ── KafkaConsumer ─────────────────────────────────────────────────────────────

/// Kafka consumer with manual commit, retry, and optional DLQ support.
pub struct KafkaConsumer {
    inner: StreamConsumer,
    config: KafkaConfig,
    topics: Vec<String>,
    dlq: Option<DlqPublisher>,
}

impl KafkaConsumer {
    /// Create a consumer subscribed to `topics`.
    ///
    /// Key settings applied:
    /// - `enable.auto.commit = false` (always, non-negotiable)
    /// - `enable.partition.eof = false`
    /// - `auto.offset.reset` from config
    pub fn new(cfg: KafkaConfig, topics: &[&str]) -> anyhow::Result<Self> {
        let inner: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &cfg.brokers)
            .set("group.id", &cfg.group_id)
            .set("client.id", &cfg.client_id)
            // ── The one rule that must never be changed ────────────────────
            .set("enable.auto.commit", "false")
            // ──────────────────────────────────────────────────────────────
            .set("auto.offset.reset", &cfg.auto_offset_reset)
            .set("session.timeout.ms", cfg.session_timeout_ms.to_string())
            .set("max.poll.interval.ms", cfg.max_poll_interval_ms.to_string())
            .set("fetch.message.max.bytes", cfg.fetch_max_bytes.to_string())
            .set("enable.partition.eof", "false")
            .create()
            .context("failed to create Kafka consumer")?;

        let topic_strs: Vec<&str> = topics.to_vec();
        inner
            .subscribe(&topic_strs)
            .context("failed to subscribe to Kafka topics")?;

        let topics_owned: Vec<String> = topics.iter().map(|t| t.to_string()).collect();

        Ok(Self {
            inner,
            config: cfg,
            topics: topics_owned,
            dlq: None,
        })
    }

    /// Attach a [`DlqPublisher`] to forward poison messages after retry exhaustion.
    ///
    /// If not called, handler failures after max retries are logged and the
    /// offset is committed (message is skipped).
    pub fn with_dlq(mut self, dlq: DlqPublisher) -> Self {
        self.dlq = Some(dlq);
        self
    }

    /// Run the consume → handle → commit loop forever.
    ///
    /// Returns only on a fatal, non-recoverable error (e.g. commit failure
    /// or DLQ publish failure when `halt_on_failure = true`).
    ///
    /// # Handler contract
    /// - Return `Ok(())` → message is committed, loop continues.
    /// - Return `Err(_)` → retry with exponential backoff.
    /// - After `max_retries` exhausted → send to DLQ (if configured),
    ///   commit to advance past the poison message.
    ///
    /// # Backpressure
    /// Processing is sequential. Slow handlers apply natural backpressure —
    /// no unbounded buffering.
    pub async fn run<F, Fut>(&self, handler: F) -> anyhow::Result<()>
    where
        F: Fn(MessageCtx) -> Fut + Send + Sync,
        Fut: Future<Output = anyhow::Result<()>> + Send,
    {
        info!(
            topics = ?self.topics,
            group_id = %self.config.group_id,
            "consumer started"
        );

        loop {
            // ── Poll ──────────────────────────────────────────────────────
            let borrowed = match self.inner.recv().await {
                Ok(m) => m,
                Err(e) => {
                    // Non-fatal Kafka errors (e.g. partition rebalance during
                    // startup) — log and continue.
                    error!("kafka recv error: {e}");
                    continue;
                }
            };

            // ── Extract owned data immediately ────────────────────────────
            // BorrowedMessage lifetime is tied to the consumer poll cycle.
            // We copy everything out so the borrow is released before any
            // await point.
            let ctx = MessageCtx::from_borrowed(&borrowed);
            let (topic, partition, offset) =
                (ctx.topic.clone(), ctx.partition, ctx.offset);

            // ── Span for tracing / OTEL ───────────────────────────────────
            let span = info_span!(
                "kafka.consume",
                topic = %topic,
                partition,
                offset,
                otel.kind = "consumer",
            );

            // ── Run handler with retry ────────────────────────────────────
            let result = async { self.run_with_retry(&handler, &ctx).await }
                .instrument(span)
                .await;

            match result {
                Ok(()) => {
                    commit_offset(&self.inner, &topic, partition, offset)
                        .await
                        .with_context(|| {
                            format!("fatal: commit failed for {topic}[{partition}]@{offset}")
                        })?;

                    debug!(topic = %topic, partition, offset, "committed");
                }

                Err(e) => {
                    error!(
                        topic = %topic,
                        partition,
                        offset,
                        max_retries = self.config.retry.max_retries,
                        "handler failed after all retries: {e:#}"
                    );

                    // ── DLQ ───────────────────────────────────────────────
                    if let Some(dlq) = &self.dlq {
                        let dlq_result = dlq.publish(&ctx, &e.to_string()).await;
                        if let Err(dlq_err) = dlq_result {
                            error!("DLQ publish failed: {dlq_err:#}");
                            if self.config.dlq.halt_on_failure {
                                return Err(anyhow::anyhow!("DLQ publish failed: {dlq_err:#}"));
                            }
                        }
                    }

                    // Commit past the poison message regardless so the
                    // consumer doesn't get stuck.
                    commit_offset(&self.inner, &topic, partition, offset)
                        .await
                        .with_context(|| {
                            format!("fatal: commit failed after DLQ for {topic}[{partition}]@{offset}")
                        })?;
                }
            }
        }
    }

    // ── Private ───────────────────────────────────────────────────────────────

    async fn run_with_retry<F, Fut>(&self, handler: &F, ctx: &MessageCtx) -> anyhow::Result<()>
    where
        F: Fn(MessageCtx) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let max = self.config.retry.max_retries;

        for attempt in 0..=max {
            match handler(ctx.clone()).await {
                Ok(()) => return Ok(()),

                Err(e) if attempt < max => {
                    let backoff = self.config.retry.backoff_ms(attempt);
                    warn!(
                        topic = %ctx.topic,
                        partition = ctx.partition,
                        offset = ctx.offset,
                        attempt,
                        backoff_ms = backoff,
                        "handler error, retrying: {e}"
                    );
                    tokio::time::sleep(Duration::from_millis(backoff)).await;
                }

                Err(e) => return Err(e),
            }
        }

        // Unreachable: the last loop iteration always hits the `Err(e)` arm.
        unreachable!("retry loop did not return")
    }
}
