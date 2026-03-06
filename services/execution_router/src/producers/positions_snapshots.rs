use hft_common::time::now_ms;
use hft_proto::oms::{ExecutionReport, OrderSide, Position};

#[allow(dead_code)]
pub fn to_position_snapshot(report: &ExecutionReport) -> Position {
    let signed_qty = if report.side == OrderSide::Sell as i32 {
        -report.last_filled_qty
    } else {
        report.last_filled_qty
    };

    Position {
        account_id: report.account_id.clone(),
        symbol: report.symbol.clone(),
        qty: signed_qty,
        avg_price: report.avg_price,
        realized_pnl: 0.0,
        updated_at_ms: now_ms(),
        schema_version: 1,
    }
}
