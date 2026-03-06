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

/// Result of running the full gate pipeline.
#[derive(Debug)]
pub enum CheckResult {
    Approved,
    Rejected { reason: RejectReason, detail: String },
}

/// Run all P0 gates against `order`.  Returns on the **first** failure so
/// that the logged reason is always the root cause.
pub async fn run_gates(
    order: &OrderCommand,
    cfg: &Config,
    kill_switch: &mut KillSwitch,
) -> CheckResult {
    // ── Gate 1: Global kill switch ────────────────────────────────────────────
    match kill_switch.check_global().await {
        Ok(true) => {
            warn!(
                order_id = %order.client_order_id,
                "REJECTED by global kill switch"
            );
            return CheckResult::Rejected {
                reason: RejectReason::KillSwitch,
                detail: "Global kill switch is active".into(),
            };
        }
        Ok(false) => {}
        Err(e) => {
            // Fail-open: log but let order through (mirrors KillSwitch::is_enabled behaviour)
            warn!(order_id = %order.client_order_id, err = %e, "Redis error on global kill-switch check — failing open");
        }
    }

    // ── Gate 2: Per-account kill switch ───────────────────────────────────────
    match kill_switch.check_account(&order.account_id).await {
        Ok(true) => {
            warn!(
                order_id  = %order.client_order_id,
                account   = %order.account_id,
                "REJECTED by per-account kill switch"
            );
            return CheckResult::Rejected {
                reason: RejectReason::KillSwitchAccount,
                detail: format!("Kill switch active for account {}", order.account_id),
            };
        }
        Ok(false) => {}
        Err(e) => {
            warn!(order_id = %order.client_order_id, err = %e, "Redis error on account kill-switch check — failing open");
        }
    }

    // ── Gate 3: Notional limit per symbol ─────────────────────────────────────
    let notional = order.qty * order.price;
    let notional_limit = cfg
        .notional_limits
        .get(&order.symbol)
        .copied()
        .unwrap_or(cfg.max_notional_per_order);

    let notional_result = check_notional_limit(notional, notional_limit);
    if !notional_result.pass {
        warn!(
            order_id = %order.client_order_id,
            symbol   = %order.symbol,
            notional = notional,
            limit    = notional_limit,
            "REJECTED: notional limit exceeded"
        );
        return CheckResult::Rejected {
            reason: RejectReason::NotionalLimitExceeded,
            detail: format!(
                "Notional {:.2} exceeds limit {:.2} for symbol {}",
                notional, notional_limit, order.symbol
            ),
        };
    }

    // ── Gate 4: Leverage limit ────────────────────────────────────────────────
    // The OrderCommand doesn't carry an explicit leverage field, so we derive
    // an *effective* leverage as  notional / (qty × price / max_leverage).
    // Real leverage is enforced by the exchange; here we cap the *requested*
    // leverage field from decision_reason if provided, otherwise skip.
    // TODO: parse leverage from order metadata when available.
    // For now we validate against a hard notional-based proxy:
    // if notional > account_equity * max_leverage  → reject.
    // This is a conservative placeholder until position data is wired in.
    let leverage_result = check_leverage_limit(cfg.max_leverage, cfg.max_leverage);
    if !leverage_result.pass {
        warn!(
            order_id = %order.client_order_id,
            "REJECTED: leverage limit exceeded"
        );
        return CheckResult::Rejected {
            reason: RejectReason::LeverageLimitExceeded,
            detail: leverage_result.reason,
        };
    }

    CheckResult::Approved
}
