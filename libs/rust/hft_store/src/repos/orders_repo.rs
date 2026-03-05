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
    use rust_decimal::prelude::FromPrimitive;
    use rust_decimal::Decimal;

    let qty = Decimal::from_f64(report.cumulative_filled_qty).unwrap_or_default();
    let price = Decimal::from_f64(report.avg_price).unwrap_or_default();

    // ... existing bind logic ...
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
    .bind(qty)
    .bind(price)
    .bind(format!("{:?}", report.status))
    .bind(&report.exchange_order_id)
    .execute(executor)
    .await?;

    Ok(())
}

pub async fn list_orders<'e, E>(
    executor: E,
    symbol: Option<String>,
    status: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<Vec<crate::pg::models::OrderRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let mut query = String::from("SELECT * FROM orders WHERE 1=1");
    if symbol.is_some() {
        query.push_str(" AND symbol = $1");
    }
    if status.is_some() {
        if symbol.is_some() {
            query.push_str(" AND status = $2");
        } else {
            query.push_str(" AND status = $1");
        }
    }
    query.push_str(" ORDER BY created_at DESC LIMIT $3 OFFSET $4");

    // Dynamic binding is easier with QueryBuilder in sqlx 0.7+, 
    // but for simplicity here we assume both are often provided.
    // Let's use a simpler static query if symbol/status are none for now to avoid complexity.
    
    let rows = sqlx::query_as::<_, crate::pg::models::OrderRow>(
        "SELECT * FROM orders 
         WHERE ($1::TEXT IS NULL OR symbol = $1)
           AND ($2::TEXT IS NULL OR status::TEXT = $2)
         ORDER BY created_at DESC LIMIT $3 OFFSET $4"
    )
    .bind(symbol)
    .bind(status)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

pub async fn get_order_by_id<'e, E>(
    executor: E,
    order_id: Uuid,
) -> Result<Option<crate::pg::models::OrderRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let row = sqlx::query_as::<_, crate::pg::models::OrderRow>(
        "SELECT * FROM orders WHERE client_order_id = $1"
    )
    .bind(order_id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}
