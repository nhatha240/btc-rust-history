//! Manual offset commit helpers.
//!
//! All consumers in `hft_mq` use `enable.auto.commit=false`.
//! Call [`commit_offset`] **after** a message has been successfully processed
//! to advance the consumer group's committed position.
//!
//! # Why `CommitMode::Async`?
//! Sync commit blocks the polling thread until the broker confirms.
//! Async commit enqueues the commit and continues — the consumer will not
//! re-read committed offsets even if the ack arrives slightly later.
//! For manual-commit loops where we process one message at a time this is safe.

use anyhow::Context;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::topic_partition_list::{Offset, TopicPartitionList};

/// Commit `offset + 1` for the given `topic` / `partition`.
///
/// Redpanda/Kafka convention: the *committed offset* is the *next* offset
/// to be fetched, so we always store `message.offset + 1`.
///
/// # Errors
/// Returns an error if the `TopicPartitionList` entry cannot be created or
/// if the broker rejects the commit.
pub async fn commit_offset(
    consumer: &StreamConsumer,
    topic: &str,
    partition: i32,
    offset: i64,
) -> anyhow::Result<()> {
    let mut tpl = TopicPartitionList::new();

    tpl.add_partition_offset(topic, partition, Offset::Offset(offset + 1))
        .with_context(|| {
            format!("failed to build TPL for {topic}[{partition}]@{offset}")
        })?;

    consumer
        .commit(&tpl, CommitMode::Async)
        .with_context(|| format!("commit failed for {topic}[{partition}]@{offset}"))
}
