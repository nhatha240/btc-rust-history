use sqlx::{Postgres, Executor};
use uuid::Uuid;
use anyhow::Result;
use chrono::{DateTime, Utc};
use hft_proto::oms::ExecutionReport;

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
