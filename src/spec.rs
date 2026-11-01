use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpecError {
    #[error("failed to read suite specification file at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse TOML suite at {path}: {source}")]
    ParseToml {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid suite configuration: {message}")]
    Validation { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaseSeverity {
    Blocker,
    Graded,
}

impl Default for CaseSeverity {
    fn default() -> Self {
        Self::Graded
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvalKind {
    Capability,
    Regression,
}

impl Default for EvalKind {
    fn default() -> Self {
        Self::Capability
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteMeta {
    pub id: String,
    pub version: String,
    pub description: String,
    #[serde(default = "default_k_values")]
    pub k_values: Vec<u32>,
    #[serde(default = "default_target_k")]
    pub target_k: u32,
    #[serde(default = "default_temperature")]
    pub required_temperature: f64,
    #[serde(default = "default_temperature_tolerance")]
    pub temperature_tolerance: f64,
    #[serde(default = "default_min_suite_pass_hat_k")]
    pub min_suite_pass_hat_k: f64,
    #[serde(default = "default_min_suite_pass_at_k")]
    pub min_suite_pass_at_k: f64,
    #[serde(default = "default_true")]
    pub fail_on_temperature_spoof: bool,
    #[serde(default)]
    pub fail_on_flaky_trap: bool,
}

fn default_k_values() -> Vec<u32> {
    vec![1, 3, 5, 10]
}

fn default_target_k() -> u32 {
    5
}

fn default_temperature() -> f64 {
    0.7
}

fn default_temperature_tolerance() -> f64 {
    0.05
}

fn default_min_suite_pass_hat_k() -> f64 {
    0.80
}

fn default_min_suite_pass_at_k() -> f64 {
    0.95
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseSpec {
    pub id: String,
    pub title: String,
    #[serde(default = "default_stratum")]
    pub stratum: String,
    #[serde(default)]
    pub severity: CaseSeverity,
    #[serde(default)]
    pub eval_kind: EvalKind,
    #[serde(default = "default_min_trials")]
    pub min_trials: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_k: Option<u32>,
    #[serde(default = "default_min_pass_at_k")]
    pub min_pass_at_k: f64,
    #[serde(default = "default_min_pass_hat_k")]
    pub min_pass_hat_k: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_flake_gap: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_temperature: Option<f64>,
    pub trials_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

fn default_stratum() -> String {
    "core".to_string()
}

fn default_min_trials() -> usize {
    5
}

fn default_min_pass_at_k() -> f64 {
    0.90
}

fn default_min_pass_hat_k() -> f64 {
    0.80
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteSpec {
    pub suite: SuiteMeta,
    #[serde(default)]
    pub strata: BTreeMap<String, f64>,
    pub cases: Vec<CaseSpec>,
}

impl SuiteSpec {
    pub fn from_toml_str(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, SpecError> {
        let path_ref = path.as_ref();
        let content = fs::read_to_string(path_ref).map_err(|source| SpecError::Io {
            path: path_ref.display().to_string(),
            source,
        })?;
        Self::from_toml_str(&content).map_err(|source| SpecError::ParseToml {
            path: path_ref.display().to_string(),
            source,
        })
    }

    pub fn validate(&self) -> Result<(), SpecError> {
        if self.cases.is_empty() {
            return Err(SpecError::Validation {
                message: "suite must contain at least one case".to_string(),
            });
        }
        if self.suite.target_k == 0 {
            return Err(SpecError::Validation {
                message: "suite target_k must be at least 1".to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_suite_spec() {
        let toml_data = r#"
[suite]
id = "test-reliability"
version = "1.0.0"
description = "Test reliability suite"
target_k = 5
required_temperature = 0.7

[strata]
critical = 0.60
core = 0.40

[[cases]]
id = "auth-flow"
title = "Auth flow multi-trial test"
stratum = "critical"
severity = "blocker"
eval_kind = "regression"
min_trials = 5
min_pass_hat_k = 1.0
min_pass_at_k = 1.0
trials_path = "fixtures/auth.jsonl"
"#;

        let suite = SuiteSpec::from_toml_str(toml_data).expect("failed to parse suite");
        assert_eq!(suite.suite.id, "test-reliability");
        assert_eq!(suite.suite.target_k, 5);
        assert_eq!(suite.cases.len(), 1);
        assert_eq!(suite.cases[0].severity, CaseSeverity::Blocker);
        assert_eq!(suite.cases[0].eval_kind, EvalKind::Regression);
        assert!((suite.cases[0].min_pass_hat_k - 1.0).abs() < f64::EPSILON);
    }
}
