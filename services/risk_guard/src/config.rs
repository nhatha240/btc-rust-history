use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub kafka_topic_orders: String,
    pub kafka_topic_orders_approved: String,
    pub kafka_group_id: String,
    
    pub redis_url: String,
    pub kill_switch_key: String,
    
    pub max_notional_per_order: f64,
    pub max_leverage: f64,
    
    pub database_url: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();
        
        let kafka_brokers = std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "redpanda:9092".to_string());
        let kafka_topic_orders = std::env::var("KAFKA_TOPIC_ORDERS").unwrap_or_else(|_| "TOPIC_ORDERS".to_string());
        let kafka_topic_orders_approved = std::env::var("KAFKA_TOPIC_ORDERS_APPROVED").unwrap_or_else(|_| "TOPIC_ORDERS_APPROVED".to_string());
        let kafka_group_id = std::env::var("KAFKA_GROUP_ID").unwrap_or_else(|_| "risk-guard-group".to_string());
        
        let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://redis:6379/0".to_string());
        let kill_switch_key = std::env::var("REDIS_KILL_SWITCH_KEY").unwrap_or_else(|_| "trading:kill-switch".to_string());
        
        let max_notional_per_order = std::env::var("LIMIT_NOTIONAL_PER_SYMBOL")
            .unwrap_or_else(|_| "5000".to_string())
            .parse()?;
            
        let max_leverage = std::env::var("LIMIT_LEVERAGE")
            .unwrap_or_else(|_| "5.0".to_string())
            .parse()?;
            
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://trader:traderpw@postgres:5432/db_trading".to_string());

        Ok(Self {
            kafka_brokers,
            kafka_topic_orders,
            kafka_topic_orders_approved,
            kafka_group_id,
            redis_url,
            kill_switch_key,
            max_notional_per_order,
            max_leverage,
            database_url,
        })
    }
}
