use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub fn unbiased_pass_at_k(n: usize, c: usize, k: u32) -> Option<f64> {
    let k_usize = k as usize;
    if k_usize == 0 || n < k_usize {
        return None;
    }
    if c > n {
        return None;
    }
    // If the number of failures (n - c) is less than k, any draw of k items
    // must contain at least one success.
    if n - c < k_usize {
        return Some(1.0);
    }

    let mut prod = 1.0;
    for j in 0..k_usize {
        let num = (n - c - j) as f64;
        let den = (n - j) as f64;
        prod *= num / den;
    }
    Some((1.0 - prod).clamp(0.0, 1.0))
}

pub fn unbiased_pass_hat_k(n: usize, c: usize, k: u32) -> Option<f64> {
    let k_usize = k as usize;
    if k_usize == 0 || n < k_usize {
        return None;
    }
    if c > n {
        return None;
    }
    // If the number of successes is less than k, we cannot draw k successes.
    if c < k_usize {
        return Some(0.0);
    }

    let mut prod = 1.0;
    for j in 0..k_usize {
        let num = (c - j) as f64;
        let den = (n - j) as f64;
        prod *= num / den;
    }
    Some(prod.clamp(0.0, 1.0))
}

pub fn projected_pass_at_k(p: f64, k: u32) -> f64 {
    let p_clamped = p.clamp(0.0, 1.0);
    if k == 0 {
        return 0.0;
    }
    let fail_rate = 1.0 - p_clamped;
    (1.0 - fail_rate.powi(k as i32)).clamp(0.0, 1.0)
}

pub fn projected_pass_hat_k(p: f64, k: u32) -> f64 {
    let p_clamped = p.clamp(0.0, 1.0);
    if k == 0 {
        return 1.0;
    }
    p_clamped.powi(k as i32).clamp(0.0, 1.0)
}

pub fn bayesian_pass_hat_k(n: usize, c: usize, k: u32, alpha: f64, beta: f64) -> f64 {
    let k_usize = k as usize;
    if k_usize == 0 {
        return 1.0;
    }
    let mut prod = 1.0;
    for j in 0..k_usize {
        let num = alpha + (c as f64) + (j as f64);
        let den = alpha + beta + (n as f64) + (j as f64);
        prod *= num / den;
    }
    prod.clamp(0.0, 1.0)
}

pub fn wilson_interval_95(n: usize, c: usize) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let n_f = n as f64;
    let p_hat = (c as f64) / n_f;
    let z = 1.959_963_984_540_054; // 95% two-sided normal quantile
    let z2 = z * z;

    let denom = 1.0 + (z2 / n_f);
    let center = (p_hat + (z2 / (2.0 * n_f))) / denom;
    let margin = (z / denom) * ((p_hat * (1.0 - p_hat) / n_f) + (z2 / (4.0 * n_f * n_f))).sqrt();

    let low = (center - margin).clamp(0.0, 1.0);
    let high = (center + margin).clamp(0.0, 1.0);
    (low, high)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaseMetrics {
    pub n_trials: usize,
    pub n_passed: usize,
    pub empirical_p: f64,
    pub unbiased_pass_at_k: BTreeMap<u32, Option<f64>>,
    pub unbiased_pass_hat_k: BTreeMap<u32, Option<f64>>,
    pub projected_pass_at_k: BTreeMap<u32, f64>,
    pub projected_pass_hat_k: BTreeMap<u32, f64>,
    pub bayesian_pass_hat_k: BTreeMap<u32, f64>,
    pub wilson_ci_95: (f64, f64),
    pub target_k: u32,
    pub target_pass_at_k: f64,
    pub target_pass_hat_k: f64,
    pub flake_gap: f64,
}

impl CaseMetrics {
    pub fn compute(n: usize, c: usize, k_values: &[u32], target_k: u32) -> Self {
        let empirical_p = if n > 0 { (c as f64) / (n as f64) } else { 0.0 };
        let mut unbiased_at = BTreeMap::new();
        let mut unbiased_hat = BTreeMap::new();
        let mut projected_at = BTreeMap::new();
        let mut projected_hat = BTreeMap::new();
        let mut bayesian_hat = BTreeMap::new();

        for &k in k_values {
            unbiased_at.insert(k, unbiased_pass_at_k(n, c, k));
            unbiased_hat.insert(k, unbiased_pass_hat_k(n, c, k));
            projected_at.insert(k, projected_pass_at_k(empirical_p, k));
            projected_hat.insert(k, projected_pass_hat_k(empirical_p, k));
            bayesian_hat.insert(k, bayesian_pass_hat_k(n, c, k, 1.0, 1.0));
        }

        // For target_k, prefer unbiased if n >= target_k, else projected
        let target_pass_at_k = unbiased_at
            .get(&target_k)
            .copied()
            .flatten()
            .unwrap_or_else(|| projected_pass_at_k(empirical_p, target_k));

        let target_pass_hat_k = unbiased_hat
            .get(&target_k)
            .copied()
            .flatten()
            .unwrap_or_else(|| projected_pass_hat_k(empirical_p, target_k));

        let flake_gap = (target_pass_at_k - target_pass_hat_k).clamp(0.0, 1.0);
        let wilson_ci_95 = wilson_interval_95(n, c);

        Self {
            n_trials: n,
            n_passed: c,
            empirical_p,
            unbiased_pass_at_k: unbiased_at,
            unbiased_pass_hat_k: unbiased_hat,
            projected_pass_at_k: projected_at,
            projected_pass_hat_k: projected_hat,
            bayesian_pass_hat_k: bayesian_hat,
            wilson_ci_95,
            target_k,
            target_pass_at_k,
            target_pass_hat_k,
            flake_gap,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuiteMetricsSummary {
    pub total_cases: usize,
    pub total_trials: usize,
    pub total_passed_trials: usize,
    pub target_k: u32,
    pub macro_avg_pass_at_k: f64,
    pub macro_avg_pass_hat_k: f64,
    pub naive_pooled_pass_at_k: f64,
    pub naive_pooled_pass_hat_k: f64,
    pub jensen_pooling_distortion: f64,
    pub weighted_pass_hat_k: f64,
}

pub fn compute_suite_metrics_summary(
    case_metrics: &[CaseMetrics],
    weights: &[f64],
    target_k: u32,
) -> SuiteMetricsSummary {
    let total_cases = case_metrics.len();
    let mut total_trials = 0;
    let mut total_passed_trials = 0;
    let mut sum_pass_at_k = 0.0;
    let mut sum_pass_hat_k = 0.0;
    let mut weighted_pass_hat_k = 0.0;
    let mut total_weight = 0.0;

    for (i, cm) in case_metrics.iter().enumerate() {
        total_trials += cm.n_trials;
        total_passed_trials += cm.n_passed;
        sum_pass_at_k += cm.target_pass_at_k;
        sum_pass_hat_k += cm.target_pass_hat_k;

        let w = weights.get(i).copied().unwrap_or(1.0);
        weighted_pass_hat_k += cm.target_pass_hat_k * w;
        total_weight += w;
    }

    let macro_avg_pass_at_k = if total_cases > 0 {
        sum_pass_at_k / (total_cases as f64)
    } else {
        0.0
    };

    let macro_avg_pass_hat_k = if total_cases > 0 {
        sum_pass_hat_k / (total_cases as f64)
    } else {
        0.0
    };

    let normalized_weighted_pass_hat_k = if total_weight > 0.0 {
        weighted_pass_hat_k / total_weight
    } else {
        macro_avg_pass_hat_k
    };

    let pooled_p = if total_trials > 0 {
        (total_passed_trials as f64) / (total_trials as f64)
    } else {
        0.0
    };

    let naive_pooled_pass_at_k = projected_pass_at_k(pooled_p, target_k);
    let naive_pooled_pass_hat_k = projected_pass_hat_k(pooled_p, target_k);
    let jensen_pooling_distortion = naive_pooled_pass_hat_k - macro_avg_pass_hat_k;

    SuiteMetricsSummary {
        total_cases,
        total_trials,
        total_passed_trials,
        target_k,
        macro_avg_pass_at_k,
        macro_avg_pass_hat_k,
        naive_pooled_pass_at_k,
        naive_pooled_pass_hat_k,
        jensen_pooling_distortion,
        weighted_pass_hat_k: normalized_weighted_pass_hat_k,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_80_percent_cliff() {
        // Prompt specification:
        // At 80% per-run success, pass^5 drops to roughly 33%.
        // pass@5 is ~99.97%.
        let p = 0.80;
        let pass_hat_5 = projected_pass_hat_k(p, 5);
        let pass_at_5 = projected_pass_at_k(p, 5);

        // 0.80^5 = 0.32768 (roughly 33%)
        assert!((pass_hat_5 - 0.32768).abs() < 1e-4);
        // 1 - 0.20^5 = 0.99968
        assert!((pass_at_5 - 0.99968).abs() < 1e-4);
    }

    #[test]
    fn test_unbiased_estimators_boundary_conditions() {
        // n = 5, c = 5 (all passed)
        assert_eq!(unbiased_pass_at_k(5, 5, 5), Some(1.0));
        assert_eq!(unbiased_pass_hat_k(5, 5, 5), Some(1.0));

        // n = 5, c = 4 (1 failure)
        // pass@5 must be 1.0 because 5 - 4 = 1 < 5 (failures < k)
        assert_eq!(unbiased_pass_at_k(5, 4, 5), Some(1.0));
        // pass^5 must be 0.0 because c = 4 < 5
        assert_eq!(unbiased_pass_hat_k(5, 4, 5), Some(0.0));

        // n = 10, c = 8, k = 2
        // pass^2 = (8/10) * (7/9) = 56 / 90 = 0.622222...
        let hat_2 = unbiased_pass_hat_k(10, 8, 2).expect("unbiased pass^2");
        assert!((hat_2 - (56.0 / 90.0)).abs() < 1e-5);
    }

    #[test]
    fn test_jensen_inequality_pooling_distortion() {
        // Case A: 100% pass (5/5)
        // Case B: 0% pass (0/5)
        // True macro average pass^2 is: 0.5 * 1.0 + 0.5 * 0.0 = 0.50
        // Pooled pass rate: 5 / 10 = 0.50
        // Pooled naive pass^2 is: 0.50^2 = 0.25 (distorts downward by -0.25)
        let cm_a = CaseMetrics::compute(5, 5, &[2], 2);
        let cm_b = CaseMetrics::compute(5, 0, &[2], 2);

        let summary = compute_suite_metrics_summary(&[cm_a, cm_b], &[1.0, 1.0], 2);
        assert!((summary.macro_avg_pass_hat_k - 0.50).abs() < 1e-4);
        assert!((summary.naive_pooled_pass_hat_k - 0.25).abs() < 1e-4);
        assert!((summary.jensen_pooling_distortion - (-0.25)).abs() < 1e-4);
    }
}
