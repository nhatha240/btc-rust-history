use std::collections::HashMap;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub kafka_topic_orders: String,
    pub kafka_topic_orders_approved: String,
    pub kafka_topic_execution_reports: String,
    pub kafka_group_id: String,

    pub redis_url: String,
    #[allow(dead_code)]
    pub kill_switch_key: String,

    /// Global notional cap — used when no per-symbol override exists.
    pub max_notional_per_order: f64,
    pub max_leverage: f64,

    /// Per-symbol overrides: `BTCUSDT=50000,ETHUSDT=20000`
    /// Parsed from env var `LIMIT_NOTIONAL_PER_SYMBOL_MAP`.
    #[serde(skip)]
    pub notional_limits: HashMap<String, f64>,

    pub database_url: String,
    pub health_addr: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let kafka_brokers = std::env::var("KAFKA_BROKERS")
            .unwrap_or_else(|_| "redpanda:9092".to_string());
        let kafka_topic_orders = std::env::var("KAFKA_TOPIC_ORDERS")
            .unwrap_or_else(|_| "TOPIC_ORDERS".to_string());
        let kafka_topic_orders_approved = std::env::var("KAFKA_TOPIC_ORDERS_APPROVED")
            .unwrap_or_else(|_| "TOPIC_ORDERS_APPROVED".to_string());
        let kafka_topic_execution_reports = std::env::var("KAFKA_TOPIC_EXECUTION_REPORTS")
            .unwrap_or_else(|_| "TOPIC_EXECUTION_REPORTS".to_string());
        let kafka_group_id = std::env::var("KAFKA_GROUP_ID")
            .unwrap_or_else(|_| "risk-guard-group".to_string());

        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://redis:6379/0".to_string());
        let kill_switch_key = std::env::var("REDIS_KILL_SWITCH_KEY")
            .unwrap_or_else(|_| "risk:kill".to_string());

        let max_notional_per_order = std::env::var("LIMIT_NOTIONAL_PER_ORDER")
            .unwrap_or_else(|_| "5000".to_string())
            .parse()?;
        let max_leverage = std::env::var("LIMIT_LEVERAGE")
            .unwrap_or_else(|_| "5.0".to_string())
            .parse()?;

        // Parse "BTCUSDT=50000,ETHUSDT=20000" into a HashMap
        let notional_limits = std::env::var("LIMIT_NOTIONAL_PER_SYMBOL_MAP")
            .unwrap_or_default()
            .split(',')
            .filter(|s| s.contains('='))
            .filter_map(|pair| {
                let mut it = pair.splitn(2, '=');
                let sym = it.next()?.trim().to_uppercase();
                let val: f64 = it.next()?.trim().parse().ok()?;
                Some((sym, val))
            })
            .collect();

        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://trader:traderpw@postgres:5432/db_trading".to_string());
        let health_addr = std::env::var("HEALTH_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:9090".to_string());

        Ok(Self {
            kafka_brokers,
            kafka_topic_orders,
            kafka_topic_orders_approved,
            kafka_topic_execution_reports,
            kafka_group_id,
            redis_url,
            kill_switch_key,
            max_notional_per_order,
            max_leverage,
            notional_limits,
            database_url,
            health_addr,
        })
    }
}
