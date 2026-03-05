
//! Normalised rejection reason codes.
//!
//! Every place in the system that can refuse an order MUST use one of these
//! variants so the web dashboard displays a consistent, human-readable reason.
//!
//! # String representation
//! [`RejectReason::as_str`] returns a SCREAMING_SNAKE_CASE code that is stable
//! across versions and safe to persist in PostgreSQL / ClickHouse.

use std::fmt;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    /// Global or per-account kill switch is active.
    KillSwitch,
    /// Per-account kill switch is active.
    KillSwitchAccount,
    /// Order notional (qty × price) exceeds the configured limit.
    NotionalLimitExceeded,
    /// Requested leverage exceeds the configured ceiling.
    LeverageLimitExceeded,
    /// Symbol is not in the approved list.
    SymbolNotAllowed,
    /// Order rate-limit exceeded for this account.
    RateLimitExceeded,
    /// Order could not be decoded from the wire format.
    MalformedOrder,
    /// Catch-all for unexpected internal failures.
    InternalError,
}

impl RejectReason {
    /// Stable SCREAMING_SNAKE_CASE code — safe to store in DB.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::KillSwitch            => "KILL_SWITCH",
            Self::KillSwitchAccount     => "KILL_SWITCH_ACCOUNT",
            Self::NotionalLimitExceeded => "NOTIONAL_LIMIT_EXCEEDED",
            Self::LeverageLimitExceeded => "LEVERAGE_LIMIT_EXCEEDED",
            Self::SymbolNotAllowed      => "SYMBOL_NOT_ALLOWED",
            Self::RateLimitExceeded     => "RATE_LIMIT_EXCEEDED",
            Self::MalformedOrder        => "MALFORMED_ORDER",
            Self::InternalError         => "INTERNAL_ERROR",
        }
    }

    /// Human-readable description shown in the web UI.
    pub fn description(&self) -> &'static str {
        match self {
            Self::KillSwitch            => "Global kill switch is active — all order submission halted",
            Self::KillSwitchAccount     => "Per-account kill switch is active",
            Self::NotionalLimitExceeded => "Order notional (qty × price) exceeds the per-symbol limit",
            Self::LeverageLimitExceeded => "Requested leverage exceeds the configured ceiling",
            Self::SymbolNotAllowed      => "Symbol is not in the approved trading list",
            Self::RateLimitExceeded     => "Order submission rate limit exceeded for this account",
            Self::MalformedOrder        => "Order payload could not be decoded",
            Self::InternalError         => "Unexpected internal error in risk_guard",
        }
    }

    /// Try to parse from the stable string code (e.g. from PostgreSQL).
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "KILL_SWITCH"             => Some(Self::KillSwitch),
            "KILL_SWITCH_ACCOUNT"     => Some(Self::KillSwitchAccount),
            "NOTIONAL_LIMIT_EXCEEDED" => Some(Self::NotionalLimitExceeded),
            "LEVERAGE_LIMIT_EXCEEDED" => Some(Self::LeverageLimitExceeded),
            "SYMBOL_NOT_ALLOWED"      => Some(Self::SymbolNotAllowed),
            "RATE_LIMIT_EXCEEDED"     => Some(Self::RateLimitExceeded),
            "MALFORMED_ORDER"         => Some(Self::MalformedOrder),
            "INTERNAL_ERROR"          => Some(Self::InternalError),
            _                         => None,
        }
    }
}

impl fmt::Display for RejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} — {}", self.as_str(), self.description())
    }
}
