use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub kafka_group_id: String,
    pub kafka_topic_order_commands: String,
    pub kafka_topic_execution_reports: String,
    pub database_url: String,
    pub execution_mode: String, // PAPER or REAL
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let execution_mode = std::env::var("EXECUTION_MODE").unwrap_or_else(|_| "PAPER".to_string());

        let kafka_brokers =
            std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());
        let kafka_group_id = std::env::var("KAFKA_GROUP_ID")
            .unwrap_or_else(|_| "execution-router-group".to_string());

        // Support both stack variants: orders.commands or TOPIC_ORDERS_APPROVED.
        let kafka_topic_order_commands = std::env::var("KAFKA_TOPIC_ORDER_COMMANDS")
            .or_else(|_| std::env::var("KAFKA_TOPIC_ORDERS_APPROVED"))
            .unwrap_or_else(|_| "TOPIC_ORDERS_APPROVED".to_string());

        let kafka_topic_execution_reports = std::env::var("KAFKA_TOPIC_EXECUTION_REPORTS")
            .or_else(|_| std::env::var("KAFKA_TOPIC_FILLS"))
            .unwrap_or_else(|_| "TOPIC_FILLS".to_string());

        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://trader:traderpw@localhost:5432/db_trading".to_string());

        if kafka_topic_order_commands.is_empty() {
            anyhow::bail!("KAFKA_TOPIC_ORDER_COMMANDS/KAFKA_TOPIC_ORDERS_APPROVED is empty");
        }
        if kafka_topic_execution_reports.is_empty() {
            anyhow::bail!("KAFKA_TOPIC_EXECUTION_REPORTS/KAFKA_TOPIC_FILLS is empty");
        }

        Ok(Self {
            kafka_brokers,
            kafka_group_id,
            kafka_topic_order_commands,
            kafka_topic_execution_reports,
            database_url: database_url
                .parse::<String>()
                .context("DATABASE_URL is invalid")?,
            execution_mode,
        })
    }
}
