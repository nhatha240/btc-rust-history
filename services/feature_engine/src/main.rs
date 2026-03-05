use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use hft_mq::{KafkaConfig, KafkaProducer};
use tracing::{error, info};

mod config;
mod consumer;
mod health;
mod indicators;
mod producer;
mod state;

use config::Config;
use producer::FeatureProducer;
use state::registry::Registry;

#[tokio::main]
async fn main() -> Result<()> {
    // robust rustls init
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let cfg = Arc::new(Config::from_env().context("load feature_engine config failed")?);
    info!(
        topic_in = %cfg.topic_candles,
        topic_out = %cfg.topic_features,
        "feature_engine starting"
    );

    let readiness = Arc::new(AtomicBool::new(false));
    let health_port = cfg.health_port;
    let readiness_for_health = Arc::clone(&readiness);
    tokio::spawn(async move {
        if let Err(e) = health::serve(health_port, readiness_for_health).await {
            error!("health server failed: {e:#}");
        }
    });

    let producer_cfg = KafkaConfig::low_latency(cfg.kafka_brokers.clone(), cfg.kafka_group_id.clone());
    let kafka_producer = KafkaProducer::new(&producer_cfg).context("create feature kafka producer failed")?;
    let producer = Arc::new(FeatureProducer::new(kafka_producer, cfg.topic_features.clone()));
    let registry = Arc::new(Registry::new((*cfg).clone()));

    readiness.store(true, Ordering::Relaxed);
    consumer::run(Arc::clone(&cfg), registry, producer).await
}
