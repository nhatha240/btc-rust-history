use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub kafka_topic_orders_approved: String,
    pub kafka_topic_fills: String,
    pub kafka_group_id: String,

    #[allow(dead_code)]
    pub exchange_name: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let kafka_brokers =
            std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "redpanda:9092".to_string());
        let kafka_topic_orders_approved = std::env::var("KAFKA_TOPIC_ORDERS_APPROVED")
            .unwrap_or_else(|_| "TOPIC_ORDERS_APPROVED".to_string());
        let kafka_topic_fills =
            std::env::var("KAFKA_TOPIC_FILLS").unwrap_or_else(|_| "TOPIC_FILLS".to_string());
        let kafka_group_id =
            std::env::var("KAFKA_GROUP_ID").unwrap_or_else(|_| "paper-trader-group".to_string());

        let exchange_name =
            std::env::var("EXCHANGE").unwrap_or_else(|_| "binance-paper".to_string());

        Ok(Self {
            kafka_brokers,
            kafka_topic_orders_approved,
            kafka_topic_fills,
            kafka_group_id,
            exchange_name,
        })
    }
}
