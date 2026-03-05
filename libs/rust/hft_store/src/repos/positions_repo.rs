use sqlx::{Postgres, Executor};
use anyhow::Result;
use hft_proto::oms::ExecutionReport;

pub async fn update_position<'e, E>(
    executor: E,
    report: &ExecutionReport,
) -> Result<()> 
where E: Executor<'e, Database = Postgres>
{
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
