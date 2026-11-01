use crate::trial::TrialRecord;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DistributionStats {
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
    pub std_dev: f64,
    pub cv: f64, // coefficient of variation (std_dev / mean)
}

fn compute_distribution(mut values: Vec<f64>) -> DistributionStats {
    let count = values.len();
    if count == 0 {
        return DistributionStats {
            count: 0,
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            p50: 0.0,
            p90: 0.0,
            p95: 0.0,
            std_dev: 0.0,
            cv: 0.0,
        };
    }

    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let min = values[0];
    let max = values[count - 1];

    let sum: f64 = values.iter().sum();
    let mean = sum / (count as f64);

    let variance: f64 = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (count as f64);
    let std_dev = variance.sqrt();
    let cv = if mean > 1e-9 { std_dev / mean } else { 0.0 };

    let p50 = percentile(&values, 0.50);
    let p90 = percentile(&values, 0.90);
    let p95 = percentile(&values, 0.95);

    DistributionStats {
        count,
        min,
        max,
        mean,
        p50,
        p90,
        p95,
        std_dev,
        cv,
    }
}

fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64) * pct).floor() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaseEconomics {
    pub total_trials: usize,
    pub total_cost_usd: f64,
    pub mean_cost_usd: f64,
    pub total_tokens: u64,
    pub mean_tokens: f64,
    pub latency_ms: DistributionStats,
    pub expected_cost_per_reliable_k_usd: f64,
}

pub fn compute_case_economics(
    trials: &[TrialRecord],
    pass_hat_k: f64,
    target_k: u32,
) -> CaseEconomics {
    let total_trials = trials.len();
    if total_trials == 0 {
        return CaseEconomics {
            total_trials: 0,
            total_cost_usd: 0.0,
            mean_cost_usd: 0.0,
            total_tokens: 0,
            mean_tokens: 0.0,
            latency_ms: compute_distribution(Vec::new()),
            expected_cost_per_reliable_k_usd: 0.0,
        };
    }

    let mut latencies = Vec::with_capacity(total_trials);
    let mut total_cost = 0.0;
    let mut total_tokens = 0u64;

    for t in trials {
        latencies.push(t.duration_ms as f64);
        total_cost += t.cost_usd;
        total_tokens += t.tokens_total;
    }

    let mean_cost_usd = total_cost / (total_trials as f64);
    let mean_tokens = (total_tokens as f64) / (total_trials as f64);
    let latency_stats = compute_distribution(latencies);

    // If pass^k is low, the workflow must be rerun on average 1 / pass^k times
    let denom = pass_hat_k.max(0.01);
    let expected_cost_per_reliable_k_usd = (mean_cost_usd * (target_k as f64)) / denom;

    CaseEconomics {
        total_trials,
        total_cost_usd: total_cost,
        mean_cost_usd,
        total_tokens,
        mean_tokens,
        latency_ms: latency_stats,
        expected_cost_per_reliable_k_usd,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distribution_and_economics() {
        let values = vec![100.0, 200.0, 300.0, 400.0, 500.0];
        let stats = compute_distribution(values);
        assert_eq!(stats.count, 5);
        assert!((stats.mean - 300.0).abs() < 1e-4);
        assert!((stats.p50 - 300.0).abs() < 1e-4);
        assert!((stats.min - 100.0).abs() < 1e-4);
        assert!((stats.max - 500.0).abs() < 1e-4);
    }
}
