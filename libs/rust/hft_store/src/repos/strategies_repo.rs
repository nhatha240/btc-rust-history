use crate::pg::models::{StratConfigAudit, StratDefinition, StratInstance};
use crate::pg::types::{DbStratMode, DbStratStatus};
use anyhow::Result;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

pub async fn list_strategies(pool: &Pool<Postgres>) -> Result<Vec<StratDefinition>> {
    let strats = sqlx::query_as::<_, StratDefinition>(
        "SELECT * FROM strat_definitions ORDER BY strategy_name"
    )
    .fetch_all(pool)
    .await?;
    Ok(strats)
}

pub async fn get_strategy_by_id(pool: &Pool<Postgres>, id: Uuid) -> Result<Option<StratDefinition>> {
    let strat = sqlx::query_as::<_, StratDefinition>(
        "SELECT * FROM strat_definitions WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(strat)
}

pub async fn update_strategy_status(
    pool: &Pool<Postgres>,
    id: Uuid,
    status: DbStratStatus,
) -> Result<()> {
    sqlx::query("UPDATE strat_definitions SET status = $1, updated_at = NOW() WHERE id = $2")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_strategy_mode(
    pool: &Pool<Postgres>,
    id: Uuid,
    mode: DbStratMode,
) -> Result<()> {
    sqlx::query("UPDATE strat_definitions SET mode = $1, updated_at = NOW() WHERE id = $2")
        .bind(mode)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_strategy_config(
    pool: &Pool<Postgres>,
    id: Uuid,
    new_config: serde_json::Value,
    changed_by: String,
    reason: Option<String>,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    // 1. Get old config
    let old_config: serde_json::Value = sqlx::query_scalar("SELECT config_json FROM strat_definitions WHERE id = $1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

    // 2. Update strat_definitions
    sqlx::query("UPDATE strat_definitions SET config_json = $1, updated_at = NOW() WHERE id = $2")
        .bind(&new_config)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    // 3. Log to audit
    sqlx::query(
        "INSERT INTO strat_config_audit (strategy_id, changed_by, change_reason, old_config, new_config)
         VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(id)
    .bind(changed_by)
    .bind(reason)
    .bind(old_config)
    .bind(new_config)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn list_strategy_instances(
    pool: &Pool<Postgres>,
    strategy_id: Uuid,
) -> Result<Vec<StratInstance>> {
    let instances = sqlx::query_as::<_, StratInstance>(
        "SELECT * FROM strat_instances WHERE strategy_id = $1 ORDER BY last_heartbeat DESC"
    )
    .bind(strategy_id)
    .fetch_all(pool)
    .await?;
    Ok(instances)
}

pub async fn list_strategy_audit_logs(
    pool: &Pool<Postgres>,
    strategy_id: Uuid,
) -> Result<Vec<StratConfigAudit>> {
    let logs = sqlx::query_as::<_, StratConfigAudit>(
        "SELECT * FROM strat_config_audit WHERE strategy_id = $1 ORDER BY created_at DESC"
    )
    .bind(strategy_id)
    .fetch_all(pool)
    .await?;
    Ok(logs)
}
