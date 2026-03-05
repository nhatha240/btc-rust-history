#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct GateResult {
    pub pass: bool,
    pub reason: String,
    pub note: Option<String>,
}

impl GateResult {
    pub fn pass() -> Self {
        Self {
            pass: true,
            reason: "Passed".to_string(),
            note: None,
        }
    }

    pub fn fail(reason: impl Into<String>) -> Self {
        Self {
            pass: false,
            reason: reason.into(),
            note: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::{check_leverage_limit, check_notional_limit};
    use crate::sizing::compute_qty_by_risk;

    #[test]
    fn test_notional_pass() {
        let r = check_notional_limit(9_000.0, 10_000.0);
        assert!(r.pass);
    }

    #[test]
    fn test_notional_fail() {
        let r = check_notional_limit(11_000.0, 10_000.0);
        assert!(!r.pass);
        assert!(r.reason.contains("10000"));
    }

    #[test]
    fn test_leverage_pass() {
        let r = check_leverage_limit(3.0, 5.0);
        assert!(r.pass);
    }

    #[test]
    fn test_leverage_fail() {
        let r = check_leverage_limit(6.5, 5.0);
        assert!(!r.pass);
    }

    #[test]
    fn test_qty_by_risk_basic() {
        // $10,000 account, 1% risk, entry 50_000, stop 49_500 → risk/unit = 500
        // Risk amount = $100, qty = 100 / 500 = 0.2
        let plan = compute_qty_by_risk(10_000.0, 50_000.0, 49_500.0, 1.0);
        let expected = 0.2_f64;
        assert!((plan.qty - expected).abs() < 1e-9, "qty={}", plan.qty);
    }

    #[test]
    fn test_qty_by_risk_zero_entry() {
        let plan = compute_qty_by_risk(10_000.0, 0.0, 49_500.0, 1.0);
        assert_eq!(plan.qty, 0.0);
    }

    #[test]
    fn test_gate_result_with_note() {
        let r = GateResult::fail("Too large").with_note("Consider splitting");
        assert!(!r.pass);
        assert_eq!(r.note.as_deref(), Some("Consider splitting"));
    }
}
