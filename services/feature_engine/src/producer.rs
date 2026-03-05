//! Kafka producer: encodes a `FeatureVector` as a protobuf `md.FeatureState`
//! and publishes it to the configured output topic.
//!
//! Quality flags are carried as the `x-quality` Kafka message header (1 byte
//! bitmask) so downstream consumers can skip under-warmed-up bars cheaply
//! without deserialising the payload.

use std::collections::HashMap;

use anyhow::Result;
use hft_mq::KafkaProducer;
use hft_proto::md::FeatureState;
use prost::Message as _;
use tracing::warn;

use crate::state::symbol_state::FeatureVector;

pub struct FeatureProducer {
    inner: KafkaProducer,
    topic: String,
}

impl FeatureProducer {
    pub fn new(inner: KafkaProducer, topic: impl Into<String>) -> Self {
        Self {
            inner,
            topic: topic.into(),
        }
    }

    /// Encode and publish `fv` to the features topic.
    ///
    /// The Kafka message key is the symbol — ensures all features for a symbol
    /// land in the same partition (ordered delivery to consumers).
    pub async fn publish(&self, fv: &FeatureVector) -> Result<()> {
        let proto = FeatureState {
            symbol:       fv.symbol.clone(),
            ts:           fv.ts,
            ema_fast:     fv.ema_fast,
            ema_slow:     fv.ema_slow,
            rsi:          fv.rsi,
            macd:         fv.macd,
            macd_signal:  fv.macd_signal,
            macd_hist:    fv.macd_hist,
            vwap:         fv.vwap,
        };

        let mut buf = Vec::with_capacity(proto.encoded_len());
        proto.encode(&mut buf)?;

        // Carry quality as a single-byte header.
        let mut headers: HashMap<String, Vec<u8>> = HashMap::with_capacity(1);
        headers.insert("x-quality".to_owned(), vec![fv.quality]);

        self.inner
            .send(&self.topic, &fv.symbol, &buf, Some(&headers))
            .await
            .inspect_err(|e| warn!(symbol = %fv.symbol, "feature publish failed: {e}"))?;

        Ok(())
    }
}
