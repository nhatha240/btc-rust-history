//! Repository for `risk_rejections` — queried by api_gateway.

use anyhow::Result;
use sqlx::{Executor, Postgres};

use crate::pg::models::RiskRejectionRow;

/// List recent risk rejections with optional filters.
pub async fn list_risk_rejections<'e, E>(
    executor: E,
    symbol: Option<&str>,
    reason: Option<&str>,
    account_id: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<RiskRejectionRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query_as::<_, RiskRejectionRow>(
        r#"
        SELECT id, client_order_id, account_id, symbol,
               qty, price, notional,
               reject_reason, reject_detail, trace_id, rejected_at
        FROM   risk_rejections
        WHERE  ($1::TEXT IS NULL OR symbol     = $1)
          AND  ($2::TEXT IS NULL OR reject_reason = $2)
          AND  ($3::TEXT IS NULL OR account_id = $3)
        ORDER  BY rejected_at DESC
        LIMIT  $4 OFFSET $5
        "#,
    )
    .bind(symbol)
    .bind(reason)
    .bind(account_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

/// Count rejections grouped by reason — used for the summary widget.
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct RejectionSummary {
    pub reject_reason: String,
    pub count: i64,
}

pub async fn rejection_summary<'e, E>(
    executor: E,
    hours: i32,
) -> Result<Vec<RejectionSummary>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query_as::<_, RejectionSummary>(
        r#"
        SELECT reject_reason, COUNT(*) AS count
        FROM   risk_rejections
        WHERE  rejected_at >= now() - ($1 || ' hours')::INTERVAL
        GROUP  BY reject_reason
        ORDER  BY count DESC
        "#,
    )
    .bind(hours)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}
