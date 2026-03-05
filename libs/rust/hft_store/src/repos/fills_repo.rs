use sqlx::{Postgres, Executor};
use uuid::Uuid;
use anyhow::Result;
use chrono::{DateTime, Utc};
use hft_proto::oms::ExecutionReport;

pub async fn insert_trade<'e, E>(
    // ... existing insert logic ...
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

    use rust_decimal::prelude::FromPrimitive;
    use rust_decimal::Decimal;

    let price = Decimal::from_f64(report.last_filled_price).unwrap_or_default();
    let qty = Decimal::from_f64(report.last_filled_qty).unwrap_or_default();
    let commission = Decimal::from_f64(report.commission).unwrap_or_default();

    sqlx::query(
        r#"
        INSERT INTO trades (
            id, trade_id, order_id, client_order_id, symbol, account_id, price, qty, fee, fee_asset, is_maker, trade_time, fill_id
        ) VALUES (
            DEFAULT, $1, (SELECT id FROM orders WHERE client_order_id = $2), $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
        )
        ON CONFLICT (fill_id) DO NOTHING
        "#
    )
    .bind(exchange_trade_id)
    .bind(client_order_id)
    .bind(&report.symbol)
    .bind(&report.account_id)
    .bind(price)
    .bind(qty)
    .bind(commission)
    .bind(&report.commission_asset)
    .bind(false)
    .bind(DateTime::<Utc>::from_timestamp_nanos(report.event_time_ns))
    .bind(&report.fill_id)
    .execute(executor)
    .await?;

    Ok(())
}

pub async fn list_trades<'e, E>(
    executor: E,
    symbol: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<Vec<crate::pg::models::TradeRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query_as::<_, crate::pg::models::TradeRow>(
        "SELECT * FROM trades 
         WHERE ($1::TEXT IS NULL OR symbol = $1)
         ORDER BY trade_time DESC LIMIT $2 OFFSET $3"
    )
    .bind(symbol)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}
