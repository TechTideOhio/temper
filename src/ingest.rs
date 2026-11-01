use crate::trial::{compute_trial_chain, verify_trial_chain, TrialRecord};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("failed to read file at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse JSON at {path} line {line}: {source}")]
    ParseJson {
        path: String,
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("trial chain verification failed for {path}: {source}")]
    ChainVerification {
        path: String,
        #[source]
        source: crate::trial::ChainError,
    },
}

pub fn load_trials_from_jsonl<P: AsRef<Path>>(
    path: P,
    verify_chain: bool,
) -> Result<Vec<TrialRecord>, IngestError> {
    let p = path.as_ref();
    let file = File::open(p).map_err(|e| IngestError::Io {
        path: p.display().to_string(),
        source: e,
    })?;

    let reader = BufReader::new(file);
    let mut trials = Vec::new();

    for (idx, line_res) in reader.lines().enumerate() {
        let line = line_res.map_err(|e| IngestError::Io {
            path: p.display().to_string(),
            source: e,
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let record: TrialRecord =
            serde_json::from_str(trimmed).map_err(|e| IngestError::ParseJson {
                path: p.display().to_string(),
                line: idx + 1,
                source: e,
            })?;
        trials.push(record);
    }

    if verify_chain && !trials.is_empty() && trials[0].trial_hash.is_some() {
        verify_trial_chain(&trials).map_err(|e| IngestError::ChainVerification {
            path: p.display().to_string(),
            source: e,
        })?;
    }

    Ok(trials)
}

#[derive(Debug, Deserialize)]
struct HarnessResult {
    #[serde(default)]
    status: String,
    #[serde(default)]
    agent: String,
    #[serde(rename = "taskHash", default)]
    task_hash: String,
    #[serde(rename = "durationMs", default)]
    duration_ms: u64,
    #[serde(rename = "costUsd", default)]
    cost_usd: Option<f64>,
    #[serde(rename = "numTurns", default)]
    num_turns: Option<u64>,
}

pub fn ingest_agent_harness_runs<P: AsRef<Path>>(
    runs_dir: P,
    default_temp: f64,
) -> Result<BTreeMap<String, Vec<TrialRecord>>, IngestError> {
    let dir = runs_dir.as_ref();
    let entries = fs::read_dir(dir).map_err(|e| IngestError::Io {
        path: dir.display().to_string(),
        source: e,
    })?;

    let mut groups: BTreeMap<String, Vec<TrialRecord>> = BTreeMap::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let result_file = path.join("result.json");
            if result_file.is_file() {
                if let Ok(content) = fs::read_to_string(&result_file) {
                    if let Ok(res) = serde_json::from_str::<HarnessResult>(&content) {
                        let case_id = if res.task_hash.is_empty() {
                            path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string()
                        } else {
                            res.task_hash
                        };

                        let passed = res.status == "success";
                        let group = groups.entry(case_id.clone()).or_default();
                        let trial_index = group.len();

                        let trial = TrialRecord {
                            case_id,
                            trial_index,
                            passed,
                            score: if passed { 1.0 } else { 0.0 },
                            temperature: default_temp,
                            model: "harness-model".to_string(),
                            agent: if res.agent.is_empty() {
                                "agent".to_string()
                            } else {
                                res.agent
                            },
                            duration_ms: res.duration_ms,
                            cost_usd: res.cost_usd.unwrap_or(0.0),
                            tokens_total: res.num_turns.unwrap_or(0) * 1000,
                            error_category: if passed { None } else { Some(res.status) },
                            output_digest: None,
                            prev_hash: None,
                            trial_hash: None,
                        };

                        group.push(trial);
                    }
                }
            }
        }
    }

    for group in groups.values_mut() {
        compute_trial_chain(group);
    }

    Ok(groups)
}
