use sqlx::{Postgres, Executor};
use uuid::Uuid;
use anyhow::Result;
use hft_proto::oms::ExecutionReport;
use crate::pg::models::{OrderExitTagSnapshot, OrderTrainingEventRow};

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

    let status_str = format!("{:?}", report.status);
    let now = chrono::Utc::now();
    
    let (ack_at, done_at) = match report.status {
        x if x == hft_proto::oms::ExecutionStatus::New as i32
            || x == hft_proto::oms::ExecutionStatus::PartiallyFilled as i32 =>
        {
            (Some(now), None)
        }
        x if x == hft_proto::oms::ExecutionStatus::Filled as i32
            || x == hft_proto::oms::ExecutionStatus::Canceled as i32
            || x == hft_proto::oms::ExecutionStatus::Rejected as i32
            || x == hft_proto::oms::ExecutionStatus::Expired as i32 =>
        {
            (None, Some(now))
        }
        _ => (None, None),
    };

    sqlx::query(
        r#"
        INSERT INTO orders (
            id, client_order_id, account_id, symbol, side, type, qty, price, status, exchange_order_id, strategy_version, ack_at, done_at
        ) VALUES (
            DEFAULT, $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12
        )
        ON CONFLICT (client_order_id) DO UPDATE SET
            status = $8,
            exchange_order_id = $9,
            ack_at = COALESCE(orders.ack_at, $11),
            done_at = COALESCE(orders.done_at, $12),
            updated_at = now()
        "#
    )
    .bind(client_order_id)
    .bind(&report.account_id)
    .bind(&report.symbol)
    .bind(if report.side == 1 { "BUY" } else { "SELL" })
    .bind("MARKET") // TODO: fix type
    .bind(qty)
    .bind(price)
    .bind(status_str)
    .bind(&report.exchange_order_id)
    .bind(&report.strategy_id) // Assuming report has strategy_id
    .bind(ack_at)
    .bind(done_at)
    .execute(executor)
    .await?;

    Ok(())
}

pub async fn list_orders<'e, E>(
    executor: E,
    symbol: Option<String>,
    status: Option<String>,
    strategy_version: Option<String>,
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
           AND ($5::TEXT IS NULL OR strategy_version = $5)
         ORDER BY created_at DESC LIMIT $3 OFFSET $4"
    )
    .bind(symbol)
    .bind(status)
    .bind(limit)
    .bind(offset)
    .bind(strategy_version)
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

pub async fn list_order_exit_tag_snapshots<'e, E>(
    executor: E,
    client_order_id: Uuid,
) -> Result<Vec<OrderExitTagSnapshot>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query_as::<_, OrderExitTagSnapshot>(
        r#"
        SELECT *
        FROM order_exit_tag_snapshots
        WHERE client_order_id = $1
        ORDER BY event_time DESC
        "#,
    )
    .bind(client_order_id)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

pub async fn list_order_training_events<'e, E>(
    executor: E,
    symbol: Option<String>,
    execution_mode: Option<String>,
    outcome_label: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<Vec<OrderTrainingEventRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query_as::<_, OrderTrainingEventRow>(
        r#"
        SELECT *
        FROM order_training_events
        WHERE ($1::TEXT IS NULL OR symbol = $1)
          AND ($2::TEXT IS NULL OR execution_mode = $2)
          AND ($3::TEXT IS NULL OR outcome_label = $3)
        ORDER BY event_time DESC
        LIMIT $4 OFFSET $5
        "#,
    )
    .bind(symbol)
    .bind(execution_mode)
    .bind(outcome_label)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;

    Ok(rows)
}
