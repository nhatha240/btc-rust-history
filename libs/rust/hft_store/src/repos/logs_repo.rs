use crate::pg::models::{ErrorLogRow, StratLogRow, RiskEventRow};
use anyhow::Result;
use sqlx::{Executor, Postgres};

/// List system error logs with filters
pub async fn list_system_logs<'e, E>(
    executor: E,
    service_name: Option<&str>,
    severity: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<ErrorLogRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query_as::<_, ErrorLogRow>(
        r#"
        SELECT * FROM observability.error_logs
        WHERE ($1::TEXT IS NULL OR service_name = $1)
          AND ($2::TEXT IS NULL OR severity     = $2)
        ORDER BY occurred_at DESC
        LIMIT $3 OFFSET $4
        "#
    )
    .bind(service_name)
    .bind(severity)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

/// List strategy decision logs
pub async fn list_strategy_logs<'e, E>(
    executor: E,
    strategy_id: Option<&str>,
    symbol: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<StratLogRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query_as::<_, StratLogRow>(
        r#"
        SELECT * FROM strat_logs
        WHERE ($1::TEXT IS NULL OR strategy_version_id = $1)
          AND ($2::TEXT IS NULL OR symbol = $2)
        ORDER BY event_time DESC
        LIMIT $3 OFFSET $4
        "#
    )
    .bind(strategy_id)
    .bind(symbol)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

/// List risk events from risk schema
pub async fn list_risk_events<'e, E>(
    executor: E,
    account_id: Option<&str>,
    event_type: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<RiskEventRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query_as::<_, RiskEventRow>(
        r#"
        SELECT * FROM risk.risk_events
        WHERE ($1::TEXT IS NULL OR account_id = $1)
          AND ($2::TEXT IS NULL OR event_type = $2)
        ORDER BY created_at DESC
        LIMIT $3 OFFSET $4
        "#
    )
    .bind(account_id)
    .bind(event_type)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}
