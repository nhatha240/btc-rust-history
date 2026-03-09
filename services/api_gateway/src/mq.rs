use hft_mq::{KafkaConfig, KafkaProducer};
use anyhow::Result;
use serde::Serialize;
use prost::Message;
use uuid::Uuid;

#[derive(Serialize)]
pub struct ConfigUpdateSignal {
    pub strategy_id: Uuid,
    pub event_time: chrono::DateTime<chrono::Utc>,
    pub action: String, // "RELOAD_CONFIG", "STOP", "START", "PAUSE"
}

pub struct ControlProducer {
    producer: Option<KafkaProducer>,
    topic: String,
}

impl ControlProducer {
    pub fn new() -> Self {
        let topic = std::env::var("TOPIC_CONTROL_CONFIG")
            .unwrap_or_else(|_| "control.config_updates".to_string());
        
        let producer = match KafkaConfig::from_env() {
            Ok(cfg) => KafkaProducer::new(&cfg).ok(),
            Err(_) => None,
        };

        if producer.is_none() {
            tracing::warn!("Kafka producer not initialized for ControlProducer (check KAFKA_BROKERS)");
        }

        Self { producer, topic }
    }

    pub async fn publish_update(&self, strategy_id: Uuid, action: &str) -> Result<()> {
        if let Some(producer) = &self.producer {
            let signal = ConfigUpdateSignal {
                strategy_id,
                event_time: chrono::Utc::now(),
                action: action.to_string(),
            };
            
            let payload = serde_json::to_vec(&signal)?;
            producer.send(&self.topic, &strategy_id.to_string(), &payload, None).await?;
            tracing::info!("Sent control signal to {}: {}", self.topic, action);
        } else {
            tracing::warn!("Skipping Kafka publish (producer not initialized)");
        }
        Ok(())
    }
}

pub struct OrderProducer {
    producer: Option<KafkaProducer>,
    topic: String,
}

impl OrderProducer {
    pub fn new() -> Self {
        let topic = std::env::var("TOPIC_OMS_ORDERS")
            .unwrap_or_else(|_| "oms.orders.v1".to_string());
        
        // Use reliable preset for OMS if KAFKA_BROKERS is set
        let producer = if let Ok(brokers) = std::env::var("KAFKA_BROKERS") {
            let cfg = KafkaConfig::reliable(&brokers, "api-gateway-orders");
            KafkaProducer::new(&cfg).ok()
        } else {
            match KafkaConfig::from_env() {
                Ok(cfg) => KafkaProducer::new(&cfg).ok(),
                Err(_) => None,
            }
        };

        if producer.is_none() {
            tracing::warn!("Kafka producer not initialized for OrderProducer (check KAFKA_BROKERS)");
        }

        Self { producer, topic }
    }

    pub async fn submit_order(&self, order: hft_proto::oms::OrderCommand) -> Result<()> {
        if let Some(producer) = &self.producer {
            let mut payload = Vec::new();
            order.encode(&mut payload)?;
            producer.send(&self.topic, &order.client_order_id, &payload, None).await?;
            tracing::info!("Sent OrderCommand to {}: {}", self.topic, order.client_order_id);
        } else {
            tracing::warn!("Skipping Kafka publish (OrderProducer not initialized). Order: {:?}", order);
        }
        Ok(())
    }
}
