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
use clickhouse::Client;

use crate::state::symbol_state::FeatureVector;

pub struct FeatureProducer {
    inner: KafkaProducer,
    topic: String,
    ch: Client,
}

#[derive(Debug, clickhouse::Row, serde::Serialize)]
struct FeatureStateRow {
    ts: i64,
    symbol: String,
    ema_fast: f64,
    ema_slow: f64,
    rsi: f64,
    macd: f64,
    macd_signal: f64,
    macd_hist: f64,
    vwap: f64,
    adx: f64,
    atr: f64,
    regime: u8,
    vol_zscore: f64,
    oi_change_pct: f64,
}

impl FeatureProducer {
    pub fn new(inner: KafkaProducer, topic: impl Into<String>, ch: Client) -> Self {
        Self {
            inner,
            topic: topic.into(),
            ch,
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
            adx:          fv.adx,
            atr:          fv.atr,
            regime:       fv.regime,
            vol_zscore:   fv.vol_zscore,
            oi_change_pct:fv.oi_change_pct,
            schema_version: 1,
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

        let ch_clone = self.ch.clone();
        let fv_clone = fv.clone();
        tokio::spawn(async move {
            match ch_clone.insert("feature_state") {
                Ok(mut ins) => {
                    if let Err(e) = ins.write(&FeatureStateRow {
                        ts: fv_clone.ts,
                        symbol: fv_clone.symbol,
                        ema_fast: fv_clone.ema_fast,
                        ema_slow: fv_clone.ema_slow,
                        rsi: fv_clone.rsi,
                        macd: fv_clone.macd,
                        macd_signal: fv_clone.macd_signal,
                        macd_hist: fv_clone.macd_hist,
                        vwap: fv_clone.vwap,
                        adx: fv_clone.adx,
                        atr: fv_clone.atr,
                        regime: fv_clone.regime as u8,
                        vol_zscore: fv_clone.vol_zscore,
                        oi_change_pct: fv_clone.oi_change_pct,
                    }).await {
                        warn!("CH write error: {e}");
                    } else if let Err(e) = ins.end().await {
                        warn!("CH end error: {e}");
                    }
                }
                Err(e) => {
                    warn!("CH insert init error: {e}");
                }
            }
        });

        Ok(())
    }
}
