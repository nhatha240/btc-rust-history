use sqlx::{Postgres, Executor};
use uuid::Uuid;
use anyhow::Result;
use hft_proto::oms::ExecutionReport;

pub async fn upsert_order<'e, E>(
    executor: E,
    client_order_id: Uuid,
    report: &ExecutionReport,
) -> Result<()> 
where E: Executor<'e, Database = Postgres>
{
    sqlx::query(
        r#"
        INSERT INTO orders (
            id, client_order_id, account_id, symbol, side, type, qty, price, status, exchange_order_id
        ) VALUES (
            DEFAULT, $1, $2, $3, $4, $5, $6, $7, $8, $9
        )
        ON CONFLICT (client_order_id) DO UPDATE SET
            status = $8,
            exchange_order_id = $9,
            updated_at = now()
        "#
    )
    .bind(client_order_id)
    .bind(&report.account_id)
    .bind(&report.symbol)
    .bind(if report.side == 1 { "BUY" } else { "SELL" })
    .bind("MARKET")
    .bind(report.cumulative_filled_qty)
    .bind(report.avg_price)
    .bind(format!("{:?}", report.status))
    .bind(&report.exchange_order_id)
    .execute(executor)
    .await?;

    Ok(())
}
