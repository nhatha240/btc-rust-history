use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use hft_proto::oms::{ExecutionReport, ExecutionStatus, OrderCommand, OrderSide, OrderType, TimeInForce};
use sqlx::{Pool, Postgres};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use uuid::Uuid;

#[derive(Clone)]
pub struct DbWriter {
    pool: Pool<Postgres>,
}

impl DbWriter {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn persist_execution(&self, order: &OrderCommand, report: &ExecutionReport) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        self.upsert_order(&mut tx, order, report).await?;
        self.append_order_event(&mut tx, order, report).await?;

        if is_fill_like(report) && report.last_filled_qty > 0.0 {
            self.insert_fill_idempotent(&mut tx, order, report).await?;
            self.update_position(&mut tx, order, report).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn upsert_order(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        order: &OrderCommand,
        report: &ExecutionReport,
    ) -> Result<()> {
        let client_order_id = parse_uuid(&order.client_order_id)?;
        let trace_id = parse_optional_uuid(&order.trace_id);

        sqlx::query(
            r#"
            INSERT INTO orders (
              client_order_id, exchange_order_id, account_id, symbol, side, type, tif,
              qty, price, stop_price, status, filled_qty, avg_price, reduce_only, trace_id
            ) VALUES (
              $1, NULLIF($2, ''), $3, $4, $5::order_side, $6::order_type, $7::time_in_force,
              $8, NULLIF($9, 0), NULLIF($10, 0), $11::order_status, $12, NULLIF($13, 0), $14, $15
            )
            ON CONFLICT (client_order_id) DO UPDATE SET
              exchange_order_id = NULLIF(EXCLUDED.exchange_order_id, ''),
              status = EXCLUDED.status,
              filled_qty = EXCLUDED.filled_qty,
              avg_price = EXCLUDED.avg_price,
              updated_at = now()
            "#,
        )
        .bind(client_order_id)
        .bind(&report.exchange_order_id)
        .bind(&order.account_id)
        .bind(&order.symbol)
        .bind(map_side(order.side))
        .bind(map_order_type(order.r#type))
        .bind(map_tif(order.tif))
        .bind(order.qty)
        .bind(order.price)
        .bind(order.stop_price)
        .bind(map_status(report.status))
        .bind(report.cumulative_filled_qty)
        .bind(report.avg_price)
        .bind(order.reduce_only)
        .bind(trace_id)
        .execute(tx.as_mut())
        .await
        .context("upsert orders failed")?;
        Ok(())
    }

    async fn append_order_event(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        order: &OrderCommand,
        report: &ExecutionReport,
    ) -> Result<()> {
        let client_order_id = parse_uuid(&order.client_order_id)?;
        let event_time = ts_from_ns(report.event_time_ns);

        sqlx::query(
            r#"
            INSERT INTO order_events (
              order_id, client_order_id, event_type, filled_qty, price, commission, commission_asset, event_time, raw
            )
            VALUES (
              (SELECT id FROM orders WHERE client_order_id = $1),
              $1,
              $2::order_event_type,
              NULLIF($3, 0),
              NULLIF($4, 0),
              NULLIF($5, 0),
              NULLIF($6, ''),
              $7,
              $8::jsonb
            )
            "#,
        )
        .bind(client_order_id)
        .bind(map_event_type(report.status))
        .bind(report.last_filled_qty)
        .bind(report.last_filled_price)
        .bind(report.commission)
        .bind(&report.commission_asset)
        .bind(event_time)
        .bind(serde_json::to_value(report)?)
        .execute(tx.as_mut())
        .await
        .context("append order_events failed")?;
        Ok(())
    }

    async fn insert_fill_idempotent(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        order: &OrderCommand,
        report: &ExecutionReport,
    ) -> Result<()> {
        let client_order_id = parse_uuid(&order.client_order_id)?;
        let trade_time = ts_from_ns(report.event_time_ns);
        let trade_id = deterministic_trade_id(report);
        let quote_qty = report.last_filled_qty * report.last_filled_price;
        let side = map_side(order.side);

        sqlx::query(
            r#"
            INSERT INTO trades (
              trade_id, order_id, client_order_id, account_id, symbol, side, qty, price, quote_qty,
              commission, commission_asset, realized_pnl, is_maker, trade_time, fill_id
            )
            VALUES (
              $1,
              (SELECT id FROM orders WHERE client_order_id = $2),
              $2, $3, $4, $5::order_side, $6, $7, $8,
              $9, NULLIF($10, ''), NULL, FALSE, $11, $12
            )
            ON CONFLICT (fill_id, trade_time) DO NOTHING
            "#,
        )
        .bind(trade_id)
        .bind(client_order_id)
        .bind(&order.account_id)
        .bind(&order.symbol)
        .bind(side)
        .bind(report.last_filled_qty)
        .bind(report.last_filled_price)
        .bind(quote_qty)
        .bind(report.commission)
        .bind(&report.commission_asset)
        .bind(trade_time)
        .bind(&report.fill_id)
        .execute(tx.as_mut())
        .await
        .context("insert fills(trades) failed")?;
        Ok(())
    }

    async fn update_position(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        order: &OrderCommand,
        report: &ExecutionReport,
    ) -> Result<()> {
        let qty_delta = if order.side == OrderSide::Buy as i32 {
            report.last_filled_qty
        } else {
            -report.last_filled_qty
        };

        sqlx::query(
            r#"
            INSERT INTO positions (
              account_id, symbol, side, qty, entry_price, unrealized_pnl, realized_pnl, leverage, margin_type, snapshot_time
            )
            VALUES ($1, $2, 'BOTH'::position_side, $3, NULLIF($4, 0), 0, 0, 1, 'isolated', now())
            ON CONFLICT (account_id, symbol, side) DO UPDATE SET
              qty = positions.qty + EXCLUDED.qty,
              entry_price = COALESCE(NULLIF(EXCLUDED.entry_price, 0), positions.entry_price),
              snapshot_time = now()
            "#,
        )
        .bind(&order.account_id)
        .bind(&order.symbol)
        .bind(qty_delta)
        .bind(report.avg_price)
        .execute(tx.as_mut())
        .await
        .context("update positions failed")?;
        Ok(())
    }
}

fn parse_uuid(value: &str) -> Result<Uuid> {
    Uuid::parse_str(value).with_context(|| format!("invalid UUID: {}", value))
}

fn parse_optional_uuid(value: &str) -> Option<Uuid> {
    if value.is_empty() {
        None
    } else {
        Uuid::parse_str(value).ok()
    }
}

fn ts_from_ns(ns: i64) -> DateTime<Utc> {
    let secs = ns.div_euclid(1_000_000_000);
    let nsecs = ns.rem_euclid(1_000_000_000) as u32;
    DateTime::<Utc>::from_timestamp(secs, nsecs).unwrap_or_else(Utc::now)
}

fn deterministic_trade_id(report: &ExecutionReport) -> i64 {
    let mut hasher = DefaultHasher::new();
    report.client_order_id.hash(&mut hasher);
    report.symbol.hash(&mut hasher);
    report.event_time_ns.hash(&mut hasher);
    report.last_filled_qty.to_bits().hash(&mut hasher);
    report.last_filled_price.to_bits().hash(&mut hasher);
    (hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF) as i64
}

fn map_side(side: i32) -> &'static str {
    if side == OrderSide::Sell as i32 {
        "SELL"
    } else {
        "BUY"
    }
}

fn map_order_type(t: i32) -> &'static str {
    if t == OrderType::Limit as i32 {
        "LIMIT"
    } else if t == OrderType::StopMarket as i32 {
        "STOP_MARKET"
    } else if t == OrderType::StopLimit as i32 {
        "STOP_LIMIT"
    } else {
        "MARKET"
    }
}

fn map_tif(tif: i32) -> &'static str {
    if tif == TimeInForce::Ioc as i32 {
        "IOC"
    } else if tif == TimeInForce::Fok as i32 {
        "FOK"
    } else if tif == TimeInForce::Gtx as i32 {
        "GTX"
    } else {
        "GTC"
    }
}

fn map_status(status: i32) -> &'static str {
    if status == ExecutionStatus::PartiallyFilled as i32 {
        "PARTIALLY_FILLED"
    } else if status == ExecutionStatus::Filled as i32 {
        "FILLED"
    } else if status == ExecutionStatus::Rejected as i32 {
        "REJECTED"
    } else if status == ExecutionStatus::Canceled as i32 {
        "CANCELED"
    } else if status == ExecutionStatus::Expired as i32 {
        "EXPIRED"
    } else {
        "NEW"
    }
}

fn map_event_type(status: i32) -> &'static str {
    if status == ExecutionStatus::PartiallyFilled as i32 {
        "PARTIALLY_FILLED"
    } else if status == ExecutionStatus::Filled as i32 {
        "FILLED"
    } else if status == ExecutionStatus::Rejected as i32 {
        "REJECTED"
    } else if status == ExecutionStatus::Canceled as i32 {
        "CANCELED"
    } else if status == ExecutionStatus::Expired as i32 {
        "EXPIRED"
    } else {
        "ACKNOWLEDGED"
    }
}

fn is_fill_like(report: &ExecutionReport) -> bool {
    report.status == ExecutionStatus::PartiallyFilled as i32
        || report.status == ExecutionStatus::Filled as i32
}
