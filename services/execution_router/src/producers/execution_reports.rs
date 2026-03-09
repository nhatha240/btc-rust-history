use hft_common::time::now_ns;
use hft_proto::oms::{ExecutionReport, ExecutionStatus, OrderCommand, OrderSide};
use hft_exchange::binance::types::BinanceOrderAck;

fn base_report(order: &OrderCommand) -> ExecutionReport {
    let now = now_ns();
    ExecutionReport {
        account_id: order.account_id.clone(),
        symbol: order.symbol.clone(),
        client_order_id: order.client_order_id.clone(),
        exchange_order_id: format!("sim-{}", order.client_order_id),
        status: ExecutionStatus::New as i32,
        side: order.side,
        last_filled_qty: 0.0,
        last_filled_price: 0.0,
        cumulative_filled_qty: 0.0,
        avg_price: 0.0,
        commission: 0.0,
        commission_asset: "USDT".to_string(),
        reject_reason: String::new(),
        event_time_ns: now,
        recv_time_ns: now,
        trace_id: order.trace_id.clone(),
        fill_id: String::new(),
        fill_seq: 0,
        schema_version: 1,
        strategy_id: order.strategy_id.clone(),
        signal_id: order.signal_id.clone(),
    }
}

pub fn build_exchange_ack(order: &OrderCommand, ack: &BinanceOrderAck) -> ExecutionReport {
    let mut report = base_report(order);
    report.status = ExecutionStatus::New as i32;
    report.exchange_order_id = ack.order_id.to_string();
    report.event_time_ns = ack.transact_time * 1_000_000;
    report.recv_time_ns = now_ns();
    report.fill_id = format!("ack-{}", ack.order_id);
    report
}

pub fn build_reports(order: &OrderCommand) -> Vec<ExecutionReport> {
    if order.qty <= 0.0 {
        return vec![build_reject(order, "qty must be > 0")];
    }

    let mut ack = base_report(order);
    ack.status = ExecutionStatus::New as i32;
    ack.fill_id = format!("ack-{}", order.client_order_id);

    let partial_qty = (order.qty * 0.4).max(0.0001);
    if partial_qty < order.qty {
        let mut partial = base_report(order);
        partial.status = ExecutionStatus::PartiallyFilled as i32;
        partial.last_filled_qty = partial_qty;
        partial.last_filled_price = choose_price(order);
        partial.cumulative_filled_qty = partial_qty;
        partial.avg_price = partial.last_filled_price;
        partial.commission = partial.last_filled_qty * partial.last_filled_price * 0.0001;
        partial.fill_id = format!("{}-p1", order.client_order_id);
        partial.fill_seq = 1;

        let mut filled = base_report(order);
        filled.status = ExecutionStatus::Filled as i32;
        filled.last_filled_qty = order.qty - partial_qty;
        filled.last_filled_price = choose_price(order);
        filled.cumulative_filled_qty = order.qty;
        filled.avg_price = filled.last_filled_price;
        filled.commission = filled.last_filled_qty * filled.last_filled_price * 0.0001;
        filled.fill_id = format!("{}-p2", order.client_order_id);
        filled.fill_seq = 2;

        return vec![ack, partial, filled];
    }

    let mut filled = base_report(order);
    filled.status = ExecutionStatus::Filled as i32;
    filled.last_filled_qty = order.qty;
    filled.last_filled_price = choose_price(order);
    filled.cumulative_filled_qty = order.qty;
    filled.avg_price = filled.last_filled_price;
    filled.commission = filled.last_filled_qty * filled.last_filled_price * 0.0001;
    filled.fill_id = format!("{}-f1", order.client_order_id);
    filled.fill_seq = 1;
    vec![ack, filled]
}

pub fn build_reject(order: &OrderCommand, reason: &str) -> ExecutionReport {
    let mut reject = base_report(order);
    reject.status = ExecutionStatus::Rejected as i32;
    reject.reject_reason = reason.to_string();
    reject
}

pub fn build_cancel(order: &OrderCommand) -> ExecutionReport {
    let mut cancel = base_report(order);
    cancel.status = ExecutionStatus::Canceled as i32;
    cancel
}

fn choose_price(order: &OrderCommand) -> f64 {
    if order.price > 0.0 {
        order.price
    } else {
        // Fallback simulation price for MARKET-like commands.
        match order.side {
            x if x == OrderSide::Buy as i32 => 62_000.0,
            _ => 62_000.0,
        }
    }
}
