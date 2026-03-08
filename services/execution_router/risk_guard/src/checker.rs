//! P0 gate pipeline — stateless, pure functions.
//!
//! Each gate takes the order + config and returns `Ok(())` or
//! `Err(RejectReason)`.  Gates run in priority order:
//! 1. Kill switch (global)
//! 2. Kill switch (per-account)
//! 3. Notional limit per symbol
//! 4. Leverage limit

use hft_proto::oms::OrderCommand;
use hft_redis::KillSwitch;
use hft_risk::{check_leverage_limit, check_notional_limit, RejectReason};
use tracing::warn;

use crate::config::Config;

use std::collections::HashMap;

/// Result of running the full gate pipeline.
#[derive(Debug)]
pub enum CheckResult {
    Approved,
    Rejected { reason: RejectReason, detail: String },
}

/// Dynamic overrides loaded from Postgres
#[derive(Default, Clone)]
pub struct DynamicLimits {
    pub symbol_notional: HashMap<String, f64>,
    pub max_leverage: Option<f64>,
}

/// Run all P0 gates against `order`.  Returns on the **first** failure so
/// that the logged reason is always the root cause.
pub async fn run_gates(
    order: &OrderCommand,
    cfg: &Config,
    pool: &sqlx::Pool<sqlx::Postgres>,
    kill_switch: &mut KillSwitch,
    dynamic: &DynamicLimits,
) -> CheckResult {
    // ── Gate 1: Global kill switch ────────────────────────────────────────────
    match kill_switch.check_global().await {
        Ok(true) => {
            crate::audit::log_risk_event_to_db(pool, order, "GLOBAL_KILL", false, Some(1.0), Some(0.0), "REJECTED", "CRITICAL").await;
            return CheckResult::Rejected {
                reason: RejectReason::KillSwitch,
                detail: "Global kill switch is active".into(),
            };
        }
        Ok(false) => {
             crate::audit::log_risk_event_to_db(pool, order, "GLOBAL_KILL", true, Some(0.0), Some(0.0), "PASSED", "INFO").await;
        }
        Err(e) => {
            warn!(order_id = %order.client_order_id, err = %e, "Redis error on global kill-switch check — failing open");
        }
    }

    // ── Gate 2: Per-account kill switch ───────────────────────────────────────
    match kill_switch.check_account(&order.account_id).await {
        Ok(true) => {
            crate::audit::log_risk_event_to_db(pool, order, "ACCOUNT_KILL", false, Some(1.0), Some(0.0), "REJECTED", "CRITICAL").await;
            return CheckResult::Rejected {
                reason: RejectReason::KillSwitchAccount,
                detail: format!("Kill switch active for account {}", order.account_id),
            };
        }
        Ok(false) => {
            crate::audit::log_risk_event_to_db(pool, order, "ACCOUNT_KILL", true, Some(0.0), Some(0.0), "PASSED", "INFO").await;
        }
        Err(e) => {
            warn!(order_id = %order.client_order_id, err = %e, "Redis error on account kill-switch check — failing open");
        }
    }

    // ── Gate 3: Notional limit per symbol ─────────────────────────────────────
    let notional = order.qty * order.price;
    // Dynamic override first
    let notional_limit = dynamic.symbol_notional.get(&order.symbol).copied()
        .or_else(|| cfg.notional_limits.get(&order.symbol).copied())
        .unwrap_or(cfg.max_notional_per_order);

    let notional_result = check_notional_limit(notional, notional_limit);
    if !notional_result.pass {
        crate::audit::log_risk_event_to_db(pool, order, "NOTIONAL_LIMIT", false, Some(notional), Some(notional_limit), "REJECTED", "CRITICAL").await;
        return CheckResult::Rejected {
            reason: RejectReason::NotionalLimitExceeded,
            detail: format!(
                "Notional {:.2} exceeds limit {:.2} for symbol {}",
                notional, notional_limit, order.symbol
            ),
        };
    }
    crate::audit::log_risk_event_to_db(pool, order, "NOTIONAL_LIMIT", true, Some(notional), Some(notional_limit), "PASSED", "INFO").await;

    // ── Gate 4: Leverage limit ────────────────────────────────────────────────
    let leverage_limit = dynamic.max_leverage.unwrap_or(cfg.max_leverage);
    let leverage_result = check_leverage_limit(leverage_limit, leverage_limit);
    if !leverage_result.pass {
        crate::audit::log_risk_event_to_db(pool, order, "LEVERAGE_LIMIT", false, None, Some(leverage_limit), "REJECTED", "CRITICAL").await;
        return CheckResult::Rejected {
            reason: RejectReason::LeverageLimitExceeded,
            detail: leverage_result.reason,
        };
    }
    crate::audit::log_risk_event_to_db(pool, order, "LEVERAGE_LIMIT", true, None, Some(leverage_limit), "PASSED", "INFO").await;

    CheckResult::Approved
}
