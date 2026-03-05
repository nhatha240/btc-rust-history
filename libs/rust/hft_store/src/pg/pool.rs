use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use anyhow::{Context, Result};
use std::time::Duration;

pub async fn create_pool(url: &str, max_conns: u32) -> Result<Pool<Postgres>> {
    PgPoolOptions::new()
        .max_connections(max_conns)
        .acquire_timeout(Duration::from_secs(5))
        .connect(url)
        .await
        .with_context(|| format!("Failed to connect to Postgres at {}", url))
}
