use sqlx::{Postgres, Executor};
use anyhow::Result;
use hft_proto::oms::ExecutionReport;

pub async fn update_position<'e, E>(
    executor: E,
    report: &ExecutionReport,
) -> Result<()> 
where E: Executor<'e, Database = Postgres>
{
    // ... existing logic ...
    let side_multiplier = if report.side == 1 { 1.0 } else { -1.0 };
    let qty_delta = report.last_filled_qty * side_multiplier;

    sqlx::query(
        r#"
        INSERT INTO positions (
            id, account_id, symbol, qty, avg_price, realized_pnl, updated_at
        ) VALUES (
            DEFAULT, $1, $2, $3, $4, 0, now()
        )
        ON CONFLICT (account_id, symbol) DO UPDATE SET
            qty = positions.qty + $3,
            avg_price = (positions.qty * positions.avg_price + $3 * $4) / NULLIF(positions.qty + $3, 0),
            updated_at = now()
        "#
    )
    .bind(&report.account_id)
    .bind(&report.symbol)
    .bind(qty_delta)
    .bind(report.last_filled_price)
    .execute(executor)
    .await?;

    Ok(())
}

pub async fn list_positions<'e, E>(
    executor: E,
    account_id: Option<String>,
) -> Result<Vec<crate::pg::models::PositionRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query_as::<_, crate::pg::models::PositionRow>(
        "SELECT * FROM positions 
         WHERE ($1::TEXT IS NULL OR account_id = $1)
         ORDER BY symbol ASC"
    )
    .bind(account_id)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}
