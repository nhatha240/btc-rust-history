use sqlx::{Postgres, Executor, Pool};
use uuid::Uuid;
use anyhow::Result;
use chrono::{DateTime, Utc};
use hft_proto::oms::ExecutionReport;
use crate::pg::models::EventRow;

pub async fn insert_order_event<'e, E>(
    executor: E,
    client_order_id: Uuid,
    event_type: &str,
    report: &ExecutionReport,
) -> Result<()>
where E: Executor<'e, Database = Postgres>
{
    sqlx::query(
        r#"
        INSERT INTO order_events (
            id, client_order_id, event_type, payload, event_time
        ) VALUES (
            DEFAULT, $1, $2, $3, $4
        )
        "#,
    )
    .bind(client_order_id)
    .bind(event_type)
    .bind(serde_json::to_value(report)?)
    .bind(DateTime::<Utc>::from_timestamp_nanos(report.event_time_ns))
    .execute(executor)
    .await?;

    Ok(())
}

pub async fn list_events_for_order(
    pool: &Pool<Postgres>,
    client_order_id: Uuid,
) -> Result<Vec<EventRow>> {
    let rows = sqlx::query_as::<_, EventRow>(
        "SELECT * FROM order_events WHERE client_order_id = $1 ORDER BY event_time ASC"
    )
    .bind(client_order_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
