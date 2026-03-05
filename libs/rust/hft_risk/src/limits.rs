use crate::gates::GateResult;

pub fn check_notional_limit(notional: f64, limit: f64) -> GateResult {
    if notional <= limit {
        GateResult::pass()
    } else {
        GateResult::fail(format!(
            "Notional limit exceeded: current={}, limit={}",
            notional, limit
        ))
    }
}

pub fn check_leverage_limit(leverage: f64, limit: f64) -> GateResult {
    if leverage <= limit {
        GateResult::pass()
    } else {
        GateResult::fail(format!(
            "Leverage limit exceeded: current={}, limit={}",
            leverage, limit
        ))
    }
}
