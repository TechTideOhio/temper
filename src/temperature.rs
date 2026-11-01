use crate::trial::TrialRecord;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum TemperatureError {
    #[error("temperature spoof detected: required {required:.2} (+/- {tolerance:.2}), but trial {trial_index} recorded {recorded:.2}")]
    SpoofDetected {
        trial_index: usize,
        required: f64,
        tolerance: f64,
        recorded: f64,
    },
    #[error("zero temperature (greedy) run detected in production suite requiring non-zero temperature ({required:.2})")]
    GreedySamplingForbidden { trial_index: usize, required: f64 },
}

pub fn verify_trial_temperatures(
    trials: &[TrialRecord],
    required_temp: f64,
    tolerance: f64,
) -> Result<(), TemperatureError> {
    for trial in trials {
        // If production requires temperature > 0.1, forbid exact 0.0 (greedy)
        if required_temp > 0.1 && trial.temperature < 0.01 {
            return Err(TemperatureError::GreedySamplingForbidden {
                trial_index: trial.trial_index,
                required: required_temp,
            });
        }

        let diff = (trial.temperature - required_temp).abs();
        if diff > tolerance {
            return Err(TemperatureError::SpoofDetected {
                trial_index: trial.trial_index,
                required: required_temp,
                tolerance,
                recorded: trial.temperature,
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputEntropy {
    pub total_trials_with_digest: usize,
    pub unique_digests: usize,
    pub shannon_entropy_bits: f64,
    pub max_identical_output_ratio: f64,
}

pub fn compute_output_entropy(trials: &[TrialRecord]) -> OutputEntropy {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut total = 0;

    for trial in trials {
        if let Some(ref digest) = trial.output_digest {
            *counts.entry(digest.as_str()).or_insert(0) += 1;
            total += 1;
        }
    }

    if total == 0 {
        return OutputEntropy {
            total_trials_with_digest: 0,
            unique_digests: 0,
            shannon_entropy_bits: 0.0,
            max_identical_output_ratio: 0.0,
        };
    }

    let mut entropy = 0.0;
    let mut max_count = 0;

    for &cnt in counts.values() {
        if cnt > max_count {
            max_count = cnt;
        }
        let p = (cnt as f64) / (total as f64);
        if p > 0.0 {
            entropy -= p * p.log2();
        }
    }

    let max_ratio = (max_count as f64) / (total as f64);

    OutputEntropy {
        total_trials_with_digest: total,
        unique_digests: counts.len(),
        shannon_entropy_bits: entropy,
        max_identical_output_ratio: max_ratio,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReliabilityRegime {
    DeterministicPass,
    ProductionReliable,
    DemoTrapFlaky,
    CapabilityDeficit,
}

impl ReliabilityRegime {
    pub fn description(&self) -> &'static str {
        match self {
            Self::DeterministicPass => {
                "Rock-solid deterministic stability across all trials (pass^k = 1.0)"
            }
            Self::ProductionReliable => "Meets production stability thresholds for release",
            Self::DemoTrapFlaky => {
                "High pass@k demo rate but broken pass^k production stability (flake wedge trap)"
            }
            Self::CapabilityDeficit => {
                "Fails capability threshold even under repeated trial retries"
            }
        }
    }
}

pub fn classify_reliability_regime(
    pass_at_k: f64,
    pass_hat_k: f64,
    min_pass_at_k: f64,
    min_pass_hat_k: f64,
) -> ReliabilityRegime {
    if (pass_hat_k - 1.0).abs() < 1e-4 {
        return ReliabilityRegime::DeterministicPass;
    }
    if pass_hat_k >= min_pass_hat_k && pass_at_k >= min_pass_at_k {
        return ReliabilityRegime::ProductionReliable;
    }
    if pass_at_k >= 0.80 && pass_hat_k < min_pass_hat_k {
        return ReliabilityRegime::DemoTrapFlaky;
    }
    ReliabilityRegime::CapabilityDeficit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temperature_verification_rejects_greedy() {
        let trial = TrialRecord {
            case_id: "test".to_string(),
            trial_index: 0,
            passed: true,
            score: 1.0,
            temperature: 0.0, // greedy
            model: "model".to_string(),
            agent: "agent".to_string(),
            duration_ms: 100,
            cost_usd: 0.01,
            tokens_total: 100,
            error_category: None,
            output_digest: None,
            prev_hash: None,
            trial_hash: None,
        };

        assert!(matches!(
            verify_trial_temperatures(&[trial], 0.7, 0.05),
            Err(TemperatureError::GreedySamplingForbidden { .. })
        ));
    }

    #[test]
    fn test_regime_classification_demo_trap() {
        // 80% pass rate gives pass@5 ~ 99.97% but pass^5 ~ 32.77%
        let regime = classify_reliability_regime(0.9997, 0.3277, 0.90, 0.80);
        assert_eq!(regime, ReliabilityRegime::DemoTrapFlaky);
    }
}
