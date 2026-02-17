use std::result::Result::Ok;
use anyhow::*;
use rdkafka::Message;
use tracing::info;
use futures::StreamExt;               // <-- thêm dòng này
use rdkafka::consumer::{Consumer};
use common::{
    model::{FeatureState, Decision},
    event::{TOPIC_FEATURE_STATE, TOPIC_SIGNAL_DECISION},
    kafka::{consumer, producer, send_json}
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").compact().init();

    let brokers = std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into());
    let group   = std::env::var("GROUP").unwrap_or_else(|_| "signal-engine-v1".into());
    let prod = producer(&brokers);
    let cons = consumer(&brokers, &group, &[TOPIC_FEATURE_STATE])?;

    info!("signal_engine started");
    while let Some(Ok(m)) = cons.stream().next().await {
        if let Some(p) = m.payload() {
            let s: FeatureState = serde_json::from_slice(p)?;

            // Example: EMA cross + RSI gate
            let maybe_side = if s.ema.ema9 > s.ema.ema25 && s.rsi.rsi14 > 55.0 { Some("BUY") }
            else if s.ema.ema9 < s.ema.ema25 && s.rsi.rsi14 < 45.0 { Some("SELL") }
            else { None };

            if let Some(side) = maybe_side {
                let d = Decision{
                    decision_id: uuid::Uuid::new_v4().to_string(),
                    ts_event_ms: s.ts_event_ms,
                    strategy_id: "ema_cross_v1".into(),
                    variant: "A".into(),
                    exchange: s.exchange, symbol: s.symbol,
                    side: side.into(), confidence: 0.7, time_in_force_ms: 60_000
                };
                send_json(&prod, TOPIC_SIGNAL_DECISION, &d.decision_id, &d).await?;
            }
            cons.commit_message(&m, rdkafka::consumer::CommitMode::Async)?;
        }
    }
    Ok(())
}
