#[derive(Debug, Clone)]
pub struct QtyPlan {
    pub qty: f64,
    pub risk_pct: f64,
}

pub fn compute_qty_by_risk(
    account_value: f64,
    entry_price: f64,
    stop_loss_price: f64,
    risk_pct_of_account: f64,
) -> QtyPlan {
    if entry_price <= 0.0 || stop_loss_price <= 0.0 || entry_price == stop_loss_price {
        return QtyPlan {
            qty: 0.0,
            risk_pct: 0.0,
        };
    }

    let risk_amount = account_value * (risk_pct_of_account / 100.0);
    let risk_per_unit = (entry_price - stop_loss_price).abs();
    let qty = risk_amount / risk_per_unit;

    QtyPlan {
        qty,
        risk_pct: risk_pct_of_account,
    }
}
