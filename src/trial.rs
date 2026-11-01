use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write;
use thiserror::Error;

pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrialRecord {
    pub case_id: String,
    pub trial_index: usize,
    pub passed: bool,
    #[serde(default)]
    pub score: f64,
    pub temperature: f64,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_agent")]
    pub agent: String,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub tokens_total: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trial_hash: Option<String>,
}

fn default_model() -> String {
    "unknown".to_string()
}

fn default_agent() -> String {
    "agent".to_string()
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChainError {
    #[error("empty trial list cannot be chained or verified")]
    EmptyList,
    #[error("genesis hash mismatch at index 0: expected {expected}, found {found}")]
    GenesisMismatch { expected: String, found: String },
    #[error("broken hash chain at index {index}: expected prev_hash {expected}, found {found}")]
    BrokenChain {
        index: usize,
        expected: String,
        found: String,
    },
    #[error("tampered record hash at index {index}: computed {computed}, found {found}")]
    TamperedHash {
        index: usize,
        computed: String,
        found: String,
    },
    #[error("missing hash at index {index}")]
    MissingHash { index: usize },
}

pub fn calculate_single_trial_hash(prev_hash: &str, trial: &TrialRecord) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(trial.case_id.as_bytes());
    hasher.update(&trial.trial_index.to_be_bytes());
    hasher.update(if trial.passed { &[1u8] } else { &[0u8] });
    hasher.update(&trial.score.to_bits().to_be_bytes());
    hasher.update(&trial.temperature.to_bits().to_be_bytes());
    hasher.update(trial.model.as_bytes());
    hasher.update(trial.agent.as_bytes());
    hasher.update(&trial.duration_ms.to_be_bytes());
    hasher.update(&trial.cost_usd.to_bits().to_be_bytes());
    hasher.update(&trial.tokens_total.to_be_bytes());

    if let Some(ref err) = trial.error_category {
        hasher.update(err.as_bytes());
    }
    if let Some(ref digest) = trial.output_digest {
        hasher.update(digest.as_bytes());
    }

    let result = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in result {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

pub fn compute_trial_chain(trials: &mut [TrialRecord]) {
    if trials.is_empty() {
        return;
    }
    let mut current_prev = GENESIS_HASH.to_string();
    for trial in trials.iter_mut() {
        trial.prev_hash = Some(current_prev.clone());
        let hash = calculate_single_trial_hash(&current_prev, trial);
        trial.trial_hash = Some(hash.clone());
        current_prev = hash;
    }
}

pub fn verify_trial_chain(trials: &[TrialRecord]) -> Result<(), ChainError> {
    if trials.is_empty() {
        return Err(ChainError::EmptyList);
    }

    let mut current_prev = GENESIS_HASH.to_string();
    for (i, trial) in trials.iter().enumerate() {
        let prev = trial
            .prev_hash
            .as_deref()
            .ok_or(ChainError::MissingHash { index: i })?;
        let recorded_hash = trial
            .trial_hash
            .as_deref()
            .ok_or(ChainError::MissingHash { index: i })?;

        if i == 0 && prev != GENESIS_HASH {
            return Err(ChainError::GenesisMismatch {
                expected: GENESIS_HASH.to_string(),
                found: prev.to_string(),
            });
        }

        if prev != current_prev {
            return Err(ChainError::BrokenChain {
                index: i,
                expected: current_prev,
                found: prev.to_string(),
            });
        }

        let computed = calculate_single_trial_hash(prev, trial);
        if computed != recorded_hash {
            return Err(ChainError::TamperedHash {
                index: i,
                computed,
                found: recorded_hash.to_string(),
            });
        }

        current_prev = computed;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_trial(idx: usize, passed: bool, temp: f64) -> TrialRecord {
        TrialRecord {
            case_id: "test-case-1".to_string(),
            trial_index: idx,
            passed,
            score: if passed { 1.0 } else { 0.0 },
            temperature: temp,
            model: "claude-3-5-sonnet".to_string(),
            agent: "claude".to_string(),
            duration_ms: 1200,
            cost_usd: 0.012,
            tokens_total: 450,
            error_category: if passed {
                None
            } else {
                Some("assertion_failure".to_string())
            },
            output_digest: Some("abc123456789".to_string()),
            prev_hash: None,
            trial_hash: None,
        }
    }

    #[test]
    fn test_chain_computation_and_verification() {
        let mut trials = vec![
            sample_trial(0, true, 0.7),
            sample_trial(1, true, 0.7),
            sample_trial(2, false, 0.7),
        ];

        compute_trial_chain(&mut trials);
        assert!(verify_trial_chain(&trials).is_ok());

        // Tampering with a trial score breaks verification
        trials[1].score = 0.5;
        assert!(matches!(
            verify_trial_chain(&trials),
            Err(ChainError::TamperedHash { index: 1, .. })
        ));
    }

    #[test]
    fn test_detects_removed_trial() {
        let mut trials = vec![
            sample_trial(0, true, 0.7),
            sample_trial(1, false, 0.7), // Failed trial
            sample_trial(2, true, 0.7),
        ];
        compute_trial_chain(&mut trials);

        // Someone removes the failed trial to pretend 100% pass rate
        trials.remove(1);
        assert!(matches!(
            verify_trial_chain(&trials),
            Err(ChainError::BrokenChain { index: 1, .. })
        ));
    }
}
