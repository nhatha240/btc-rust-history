use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub kafka_topic_fills: String,
    pub kafka_group_id: String,

    pub database_url: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let kafka_brokers =
            std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "redpanda:9092".to_string());
        let kafka_topic_fills =
            std::env::var("KAFKA_TOPIC_FILLS").unwrap_or_else(|_| "TOPIC_FILLS".to_string());
        let kafka_group_id =
            std::env::var("KAFKA_GROUP_ID").unwrap_or_else(|_| "order-executor-group".to_string());

        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://trader:traderpw@postgres:5432/db_trading".to_string());

        Ok(Self {
            kafka_brokers,
            kafka_topic_fills,
            kafka_group_id,
            database_url,
        })
    }
}
