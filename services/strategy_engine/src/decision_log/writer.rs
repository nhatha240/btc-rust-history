use anyhow::{Context, Result};
use hft_proto::oms::{OrderCommand, OrderSide};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

#[derive(Clone)]
pub struct DecisionLogWriter {
    pool: Pool<Postgres>,
}

impl DecisionLogWriter {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn write_enter(&self, order: &OrderCommand) -> Result<()> {
        let trace_id = Uuid::parse_str(&order.trace_id).ok();
        let direction = if order.side == OrderSide::Sell as i32 {
            "SHORT"
        } else {
            "LONG"
        };

        sqlx::query(
            r#"
            INSERT INTO decision_logs (
              trace_id, account_id, symbol, direction, action, block_reason, confidence,
              model_version, feature_version, strategy_version, entry_price, qty, tp_price, sl_price, features, decided_at
            ) VALUES (
              $1, $2, $3, $4::signal_direction, 'ENTER'::decision_action, NULL, 1.0,
              'manual', 'manual', 'strategy-p0', NULLIF($5, 0), $6, NULL, NULLIF($7, 0),
              $8::jsonb, now()
            )
            "#,
        )
        .bind(trace_id)
        .bind(&order.account_id)
        .bind(&order.symbol)
        .bind(direction)
        .bind(order.price)
        .bind(order.qty)
        .bind(order.stop_price)
        .bind(serde_json::json!({
            "source": "manual",
            "decision_reason": order.decision_reason,
            "tp_sl": {
                "tp": null,
                "sl": if order.stop_price > 0.0 { Some(order.stop_price) } else { None }
            }
        }))
        .execute(&self.pool)
        .await
        .context("write decision_logs failed")?;
        Ok(())
    }
}
