use std::cmp::Ordering;

const EPSILON: f64 = 1e-12;

#[derive(Debug, Clone)]
pub struct FactorInput {
    pub name: String,
    pub series: Vec<f64>,
    pub return_series: Vec<f64>,
    pub weight: f64,
    pub rolling_window: Option<usize>,
    pub current_value: Option<f64>,
    pub direction_hint: Option<i8>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RegimeFilterConfig {
    pub enabled: bool,
    pub current_regime: Option<String>,
    pub allowed_regimes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct VolatilityFilterConfig {
    pub enabled: bool,
    pub historical_volatility_series: Vec<f64>,
    pub current_volatility: Option<f64>,
    pub min_zscore: Option<f64>,
    pub max_zscore: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum WeightMode {
    #[default]
    Equal,
    Manual,
    Correlation,
    RankedCorrelation,
}

impl WeightMode {
    pub fn from_env(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "manual" => Self::Manual,
            "correlation" => Self::Correlation,
            "ranked-correlation" | "ranked_correlation" | "ranked" => Self::RankedCorrelation,
            _ => Self::Equal,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SignalBuildConfig {
    pub rolling_window: usize,
    pub min_history: usize,
    pub min_abs_correlation: f64,
    pub rank_decay: f64,
    pub weight_mode: WeightMode,
    pub winsorize_zscore_at: f64,
    pub mean_return: Option<f64>,
    pub std_return: Option<f64>,
    pub regime_filter: Option<RegimeFilterConfig>,
    pub volatility_filter: Option<VolatilityFilterConfig>,
}

impl Default for SignalBuildConfig {
    fn default() -> Self {
        Self {
            rolling_window: 120,
            min_history: 30,
            min_abs_correlation: 0.03,
            rank_decay: 0.85,
            weight_mode: WeightMode::Equal,
            winsorize_zscore_at: 4.0,
            mean_return: None,
            std_return: None,
            regime_filter: None,
            volatility_filter: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FactorContribution {
    pub name: String,
    pub correlation: f64,
    pub sign: i8,
    pub zscore: f64,
    pub rank: Option<usize>,
    pub raw_weight: f64,
    pub normalized_weight: f64,
    pub contribution: f64,
    pub passed: bool,
    pub reject_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SignalResult {
    pub signal: f64,
    pub predicted_return: f64,
    pub mean_return: f64,
    pub std_return: f64,
    pub suppressed: bool,
    pub suppression_reasons: Vec<String>,
    pub regime_passed: bool,
    pub volatility_passed: bool,
    pub factors_used: usize,
    pub total_factors: usize,
    pub contributions: Vec<FactorContribution>,
}

#[derive(Debug, Clone)]
struct PreparedFactor {
    factor_index: usize,
    factor: FactorInput,
    passed: bool,
    reject_reason: Option<String>,
    correlation: f64,
    sign: i8,
    zscore: f64,
    raw_weight: f64,
    contribution: f64,
}

pub fn calculate_mean(values: &[f64]) -> Option<f64> {
    let data = sanitize_series(values);
    if data.is_empty() {
        return None;
    }
    Some(data.iter().sum::<f64>() / data.len() as f64)
}

pub fn calculate_std(values: &[f64]) -> Option<f64> {
    let data = sanitize_series(values);
    if data.is_empty() {
        return None;
    }
    if data.len() == 1 {
        return Some(0.0);
    }

    let mean = calculate_mean(&data)?;
    let variance = data
        .iter()
        .map(|value| {
            let delta = value - mean;
            delta * delta
        })
        .sum::<f64>()
        / data.len() as f64;

    Some(variance.max(0.0).sqrt())
}

pub fn calculate_zscore(value: f64, mean: f64, std: f64) -> f64 {
    if !value.is_finite() || !mean.is_finite() || !std.is_finite() {
        return 0.0;
    }
    if std.abs() <= EPSILON {
        return 0.0;
    }
    (value - mean) / std
}

#[allow(dead_code)]
pub fn normalize_factor_series(series: &[f64]) -> Vec<f64> {
    let data = sanitize_series(series);
    if data.is_empty() {
        return vec![];
    }

    let Some(mean) = calculate_mean(&data) else {
        return vec![];
    };
    let Some(std) = calculate_std(&data) else {
        return vec![];
    };

    data.iter()
        .map(|value| calculate_zscore(*value, mean, std))
        .collect()
}

pub fn detect_factor_correlation_sign(factor_series: &[f64], return_series: &[f64]) -> i8 {
    let (factor, returns) = sanitize_aligned_pair(factor_series, return_series);
    let correlation = pearson_correlation(&factor, &returns);

    if correlation.abs() <= EPSILON {
        0
    } else if correlation > 0.0 {
        1
    } else {
        -1
    }
}

pub fn build_zscore_signal(factors: &[FactorInput], config: &SignalBuildConfig) -> SignalResult {
    if factors.is_empty() {
        return SignalResult {
            signal: 0.0,
            predicted_return: 0.0,
            mean_return: 0.0,
            std_return: 0.0,
            suppressed: true,
            suppression_reasons: vec!["no_factors".to_string()],
            regime_passed: true,
            volatility_passed: true,
            factors_used: 0,
            total_factors: 0,
            contributions: vec![],
        };
    }

    let mut suppression_reasons = Vec::<String>::new();
    let regime_gate = evaluate_regime_filter(config.regime_filter.as_ref());
    let vol_gate = evaluate_volatility_filter(config.volatility_filter.as_ref());

    if !regime_gate.passed {
        if let Some(reason) = regime_gate.reason {
            suppression_reasons.push(reason);
        }
    }
    if !vol_gate.passed {
        if let Some(reason) = vol_gate.reason {
            suppression_reasons.push(reason);
        }
    }

    let mut prepared: Vec<PreparedFactor> = factors
        .iter()
        .cloned()
        .enumerate()
        .map(|(factor_index, factor)| prepare_factor(factor_index, factor, config))
        .collect();

    let mut ranked: Vec<PreparedFactor> =
        prepared.iter().filter(|row| row.passed).cloned().collect();

    ranked.sort_by(|a, b| {
        b.correlation
            .abs()
            .partial_cmp(&a.correlation.abs())
            .unwrap_or(Ordering::Equal)
    });

    for (index, row) in ranked.iter_mut().enumerate() {
        let abs_corr = row.correlation.abs();
        let manual_weight = row.factor.weight.max(0.0);
        let rank_weight = config.rank_decay.powf(index as f64);

        row.raw_weight = match config.weight_mode {
            WeightMode::Equal => 1.0,
            WeightMode::Manual => manual_weight,
            WeightMode::Correlation => manual_weight * abs_corr,
            WeightMode::RankedCorrelation => manual_weight * abs_corr * rank_weight,
        };

        if !row.raw_weight.is_finite() {
            row.raw_weight = 0.0;
        }

        row.contribution = f64::from(row.sign) * row.zscore;
    }

    let total_weight = ranked.iter().map(|row| row.raw_weight).sum::<f64>();
    if !ranked.is_empty() && total_weight <= EPSILON {
        suppression_reasons.push("all_factor_weights_zero".to_string());
    }
    if ranked.is_empty() {
        suppression_reasons.push("no_valid_factors_after_filters".to_string());
    }

    let normalized_denominator = if total_weight > EPSILON {
        total_weight
    } else {
        1.0
    };

    let weighted_signal = ranked
        .iter()
        .map(|row| {
            let normalized_weight = row.raw_weight / normalized_denominator;
            normalized_weight * row.contribution
        })
        .sum::<f64>();

    let reference_returns = ranked
        .first()
        .map(|row| {
            tail(
                &sanitize_series(&row.factor.return_series),
                config
                    .min_history
                    .max(row.factor.rolling_window.unwrap_or(config.rolling_window)),
            )
        })
        .unwrap_or_default();

    let mean_return = config
        .mean_return
        .or_else(|| calculate_mean(&reference_returns))
        .unwrap_or(0.0);

    let std_return = config
        .std_return
        .or_else(|| calculate_std(&reference_returns))
        .unwrap_or(0.0)
        .max(0.0);

    let model_invalid = ranked.is_empty() || total_weight <= EPSILON;
    let suppressed = !regime_gate.passed || !vol_gate.passed || model_invalid;
    let signal = if suppressed { 0.0 } else { weighted_signal };

    let mut rank_lookup = std::collections::HashMap::<usize, usize>::new();
    let mut row_lookup = std::collections::HashMap::<usize, PreparedFactor>::new();
    for (idx, row) in ranked.iter().cloned().enumerate() {
        rank_lookup.insert(row.factor_index, idx + 1);
        row_lookup.insert(row.factor_index, row);
    }

    let contributions = prepared
        .iter_mut()
        .map(|row| {
            if let Some(ranked_row) = row_lookup.get(&row.factor_index) {
                let normalized_weight = ranked_row.raw_weight / normalized_denominator;
                FactorContribution {
                    name: ranked_row.factor.name.clone(),
                    correlation: ranked_row.correlation,
                    sign: ranked_row.sign,
                    zscore: ranked_row.zscore,
                    rank: rank_lookup.get(&ranked_row.factor_index).copied(),
                    raw_weight: ranked_row.raw_weight,
                    normalized_weight,
                    contribution: normalized_weight * ranked_row.contribution,
                    passed: true,
                    reject_reason: None,
                }
            } else {
                FactorContribution {
                    name: row.factor.name.clone(),
                    correlation: row.correlation,
                    sign: row.sign,
                    zscore: row.zscore,
                    rank: None,
                    raw_weight: 0.0,
                    normalized_weight: 0.0,
                    contribution: 0.0,
                    passed: false,
                    reject_reason: Some(
                        row.reject_reason
                            .clone()
                            .unwrap_or_else(|| "filtered_out".to_string()),
                    ),
                }
            }
        })
        .collect::<Vec<_>>();

    SignalResult {
        signal,
        predicted_return: mean_return + std_return * signal,
        mean_return,
        std_return,
        suppressed,
        suppression_reasons,
        regime_passed: regime_gate.passed,
        volatility_passed: vol_gate.passed,
        factors_used: ranked.len(),
        total_factors: factors.len(),
        contributions,
    }
}

fn prepare_factor(
    factor_index: usize,
    factor: FactorInput,
    cfg: &SignalBuildConfig,
) -> PreparedFactor {
    let factor_series = sanitize_series(&factor.series);
    let return_series = sanitize_series(&factor.return_series);
    let rolling_window = factor.rolling_window.unwrap_or(cfg.rolling_window).max(2);

    if !factor.enabled {
        return rejected_factor(factor_index, factor, "factor_disabled", 0.0, 0, 0.0);
    }

    if factor_series.len() < cfg.min_history {
        return rejected_factor(
            factor_index,
            factor,
            "insufficient_factor_history",
            0.0,
            0,
            0.0,
        );
    }

    if return_series.len() < cfg.min_history {
        return rejected_factor(
            factor_index,
            factor,
            "insufficient_return_history",
            0.0,
            0,
            0.0,
        );
    }

    let aligned_len = factor_series
        .len()
        .min(return_series.len())
        .min(rolling_window)
        .max(0);

    if aligned_len < 2 {
        return rejected_factor(factor_index, factor, "insufficient_overlap", 0.0, 0, 0.0);
    }

    let factor_tail = tail(&factor_series, aligned_len);
    let return_tail = tail(&return_series, aligned_len);
    let (aligned_factor, aligned_return) = sanitize_aligned_pair(&factor_tail, &return_tail);

    if aligned_factor.len() < 2 {
        return rejected_factor(
            factor_index,
            factor,
            "insufficient_finite_overlap",
            0.0,
            0,
            0.0,
        );
    }

    let correlation = pearson_correlation(&aligned_factor, &aligned_return);
    let sign = factor
        .direction_hint
        .unwrap_or_else(|| detect_factor_correlation_sign(&aligned_factor, &aligned_return));

    if correlation.abs() < cfg.min_abs_correlation {
        return rejected_factor(
            factor_index,
            factor,
            "low_predictive_correlation",
            correlation,
            sign,
            0.0,
        );
    }

    let current_value = factor
        .current_value
        .or_else(|| factor_series.last().copied())
        .unwrap_or(f64::NAN);

    if !current_value.is_finite() {
        return rejected_factor(
            factor_index,
            factor,
            "missing_current_factor_value",
            correlation,
            sign,
            0.0,
        );
    }

    let segment = tail(&factor_series, rolling_window);
    let mean = calculate_mean(&segment).unwrap_or(0.0);
    let std = calculate_std(&segment).unwrap_or(0.0);
    let raw_zscore = calculate_zscore(current_value, mean, std);
    let bounded_zscore = clamp(
        raw_zscore,
        -cfg.winsorize_zscore_at,
        cfg.winsorize_zscore_at,
    );

    PreparedFactor {
        factor_index,
        factor,
        passed: true,
        reject_reason: None,
        correlation,
        sign,
        zscore: bounded_zscore,
        raw_weight: 0.0,
        contribution: 0.0,
    }
}

fn rejected_factor(
    factor_index: usize,
    factor: FactorInput,
    reason: &str,
    correlation: f64,
    sign: i8,
    zscore: f64,
) -> PreparedFactor {
    PreparedFactor {
        factor_index,
        factor,
        passed: false,
        reject_reason: Some(reason.to_string()),
        correlation,
        sign,
        zscore,
        raw_weight: 0.0,
        contribution: 0.0,
    }
}

#[derive(Debug)]
struct GateResult {
    passed: bool,
    reason: Option<String>,
}

fn evaluate_regime_filter(config: Option<&RegimeFilterConfig>) -> GateResult {
    let Some(cfg) = config else {
        return GateResult {
            passed: true,
            reason: None,
        };
    };

    if !cfg.enabled {
        return GateResult {
            passed: true,
            reason: None,
        };
    }

    let Some(current) = cfg
        .current_regime
        .as_ref()
        .map(|value| value.trim().to_string())
    else {
        return GateResult {
            passed: false,
            reason: Some("regime_filter_missing_current".to_string()),
        };
    };

    if current.is_empty() {
        return GateResult {
            passed: false,
            reason: Some("regime_filter_missing_current".to_string()),
        };
    }

    if cfg.allowed_regimes.is_empty() {
        return GateResult {
            passed: false,
            reason: Some("regime_filter_empty_allowed_set".to_string()),
        };
    }

    if !cfg
        .allowed_regimes
        .iter()
        .any(|allowed| allowed == &current)
    {
        return GateResult {
            passed: false,
            reason: Some(format!("regime_not_allowed:{current}")),
        };
    }

    GateResult {
        passed: true,
        reason: None,
    }
}

fn evaluate_volatility_filter(config: Option<&VolatilityFilterConfig>) -> GateResult {
    let Some(cfg) = config else {
        return GateResult {
            passed: true,
            reason: None,
        };
    };

    if !cfg.enabled {
        return GateResult {
            passed: true,
            reason: None,
        };
    }

    let history = sanitize_series(&cfg.historical_volatility_series);
    if history.len() < 2 {
        return GateResult {
            passed: false,
            reason: Some("vol_filter_insufficient_history".to_string()),
        };
    }

    let current_vol = cfg
        .current_volatility
        .or_else(|| history.last().copied())
        .unwrap_or(f64::NAN);

    if !current_vol.is_finite() {
        return GateResult {
            passed: false,
            reason: Some("vol_filter_missing_current".to_string()),
        };
    }

    let mean = calculate_mean(&history).unwrap_or(0.0);
    let std = calculate_std(&history).unwrap_or(0.0);
    let zscore = calculate_zscore(current_vol, mean, std);
    let min_z = cfg.min_zscore.unwrap_or(f64::NEG_INFINITY);
    let max_z = cfg.max_zscore.unwrap_or(f64::INFINITY);

    if zscore < min_z || zscore > max_z {
        return GateResult {
            passed: false,
            reason: Some(format!("vol_z_out_of_range:{zscore:.4}")),
        };
    }

    GateResult {
        passed: true,
        reason: None,
    }
}

fn sanitize_series(values: &[f64]) -> Vec<f64> {
    values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect()
}

fn sanitize_aligned_pair(series_a: &[f64], series_b: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let len = series_a.len().min(series_b.len());
    let a_offset = series_a.len().saturating_sub(len);
    let b_offset = series_b.len().saturating_sub(len);

    let mut aligned_a = Vec::with_capacity(len);
    let mut aligned_b = Vec::with_capacity(len);

    for i in 0..len {
        let a = series_a[a_offset + i];
        let b = series_b[b_offset + i];
        if a.is_finite() && b.is_finite() {
            aligned_a.push(a);
            aligned_b.push(b);
        }
    }

    (aligned_a, aligned_b)
}

fn pearson_correlation(series_a: &[f64], series_b: &[f64]) -> f64 {
    let len = series_a.len().min(series_b.len());
    if len < 2 {
        return 0.0;
    }

    let a = &series_a[series_a.len() - len..];
    let b = &series_b[series_b.len() - len..];
    let Some(mean_a) = calculate_mean(a) else {
        return 0.0;
    };
    let Some(mean_b) = calculate_mean(b) else {
        return 0.0;
    };

    let mut covariance = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;

    for i in 0..len {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        covariance += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    if var_a <= EPSILON || var_b <= EPSILON {
        return 0.0;
    }

    covariance / (var_a * var_b).sqrt()
}

fn tail(values: &[f64], n: usize) -> Vec<f64> {
    if n == 0 {
        return vec![];
    }
    if values.len() <= n {
        return values.to_vec();
    }
    values[values.len() - n..].to_vec()
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_std_zscore_work() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mean = calculate_mean(&values).unwrap();
        let std = calculate_std(&values).unwrap();
        let z = calculate_zscore(5.0, mean, std);

        assert!((mean - 3.0).abs() < 1e-9);
        assert!(std > 0.0);
        assert!(z > 1.0);
    }

    #[test]
    fn normalize_series_centers_data() {
        let normalized = normalize_factor_series(&[1.0, 2.0, 3.0, 4.0]);
        let mean = calculate_mean(&normalized).unwrap();
        assert!(mean.abs() < 1e-9);
    }

    #[test]
    fn detect_sign_positive() {
        let factor = vec![1.0, 2.0, 3.0, 4.0];
        let returns = vec![0.1, 0.2, 0.3, 0.4];
        assert_eq!(detect_factor_correlation_sign(&factor, &returns), 1);
    }

    #[test]
    fn build_signal_includes_weighted_contribution() {
        let returns = (0..100).map(|i| (i as f64) * 0.001).collect::<Vec<_>>();
        let factor_series = returns.clone();

        let factor = FactorInput {
            name: "trend".to_string(),
            series: factor_series,
            return_series: returns,
            weight: 1.0,
            rolling_window: Some(50),
            current_value: Some(0.2),
            direction_hint: None,
            enabled: true,
        };

        let config = SignalBuildConfig::default();
        let result = build_zscore_signal(&[factor], &config);

        assert_eq!(result.total_factors, 1);
        assert_eq!(result.factors_used, 1);
        assert!(!result.contributions.is_empty());
    }
}
