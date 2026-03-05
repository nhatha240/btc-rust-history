use sqlx::{Postgres, Executor};
use uuid::Uuid;
use anyhow::Result;
use chrono::{DateTime, Utc};
use hft_proto::oms::ExecutionReport;

pub async fn insert_trade<'e, E>(
    executor: E,
    client_order_id: Uuid,
    report: &ExecutionReport,
) -> Result<()> 
where E: Executor<'e, Database = Postgres>
{
    let exchange_trade_id: i64 = report
        .exchange_order_id
        .chars()
        .filter(|c| c.is_digit(10))
        .collect::<String>()
        .parse()
        .unwrap_or(0);

    sqlx::query(
        r#"
        INSERT INTO trades (
            id, trade_id, order_id, client_order_id, symbol, account_id, price, qty, fee, fee_asset, is_maker, trade_time
        ) VALUES (
            DEFAULT, $1, (SELECT id FROM orders WHERE client_order_id = $2), $2, $3, $4, $5, $6, $7, $8, $9, $10
        )
        "#
    )
    .bind(exchange_trade_id)
    .bind(client_order_id)
    .bind(&report.symbol)
    .bind(&report.account_id)
    .bind(report.last_filled_price)
    .bind(report.last_filled_qty)
    .bind(report.commission)
    .bind(&report.commission_asset)
    .bind(false)
    .bind(DateTime::<Utc>::from_timestamp_nanos(report.event_time_ns))
    .execute(executor)
    .await?;

    Ok(())
}
