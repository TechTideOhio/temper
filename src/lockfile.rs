use crate::ingest::load_trials_from_jsonl;
use crate::metrics::CaseMetrics;
use crate::spec::SuiteSpec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::path::Path;
use thiserror::Error;

const DOMAIN_TAG: &[u8] = b"tideos-temper-lock-v1\0";

#[derive(Debug, Error)]
pub enum LockfileError {
    #[error("failed to read file at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize/deserialize lockfile: {source}")]
    Json {
        #[source]
        source: serde_json::Error,
    },
    #[error("lockfile drift detected: {message}")]
    Drift { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaseLockEntry {
    pub case_digest: String,
    pub trials_digest: String,
    pub n_trials: usize,
    pub n_passed: usize,
    pub target_k: u32,
    pub target_pass_hat_k: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemperLockfile {
    pub format_version: String,
    pub suite_id: String,
    pub suite_version: String,
    pub suite_digest: String,
    pub cases: BTreeMap<String, CaseLockEntry>,
    pub overall_lock_digest: String,
}

fn compute_framed_sha256(entries: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN_TAG);
    for entry in entries {
        hasher.update(&(entry.len() as u64).to_be_bytes());
        hasher.update(entry);
    }
    let res = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in res {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

pub fn generate_lockfile<P: AsRef<Path>>(
    suite: &SuiteSpec,
    root_dir: P,
) -> Result<TemperLockfile, LockfileError> {
    let root = root_dir.as_ref();

    let mut suite_hasher = Sha256::new();
    suite_hasher.update(suite.suite.id.as_bytes());
    suite_hasher.update(suite.suite.version.as_bytes());
    suite_hasher.update(&suite.suite.target_k.to_be_bytes());
    suite_hasher.update(&suite.suite.required_temperature.to_bits().to_be_bytes());

    for (k, v) in &suite.strata {
        suite_hasher.update(k.as_bytes());
        suite_hasher.update(&v.to_bits().to_be_bytes());
    }

    let suite_digest = {
        let res = suite_hasher.finalize();
        let mut hex = String::with_capacity(64);
        for byte in res {
            let _ = write!(hex, "{byte:02x}");
        }
        hex
    };

    let mut cases = BTreeMap::new();
    let mut overall_entries = Vec::new();
    overall_entries.push(suite_digest.as_bytes());

    for case in &suite.cases {
        let trials_path = root.join(&case.trials_path);
        let trials =
            load_trials_from_jsonl(&trials_path, false).map_err(|e| LockfileError::Io {
                path: trials_path.display().to_string(),
                source: std::io::Error::other(e.to_string()),
            })?;

        let trials_bytes = fs::read(&trials_path).map_err(|e| LockfileError::Io {
            path: trials_path.display().to_string(),
            source: e,
        })?;

        let trials_digest = compute_framed_sha256(&[&trials_bytes]);
        let target_k = case.target_k.unwrap_or(suite.suite.target_k);
        let cm = CaseMetrics::compute(
            trials.len(),
            trials.iter().filter(|t| t.passed).count(),
            &suite.suite.k_values,
            target_k,
        );

        let mut case_hasher = Sha256::new();
        case_hasher.update(case.id.as_bytes());
        case_hasher.update(&case.min_trials.to_be_bytes());
        case_hasher.update(&case.min_pass_hat_k.to_bits().to_be_bytes());
        case_hasher.update(trials_digest.as_bytes());

        let case_digest = {
            let res = case_hasher.finalize();
            let mut hex = String::with_capacity(64);
            for byte in res {
                let _ = write!(hex, "{byte:02x}");
            }
            hex
        };

        cases.insert(
            case.id.clone(),
            CaseLockEntry {
                case_digest,
                trials_digest,
                n_trials: cm.n_trials,
                n_passed: cm.n_passed,
                target_k,
                target_pass_hat_k: cm.target_pass_hat_k,
            },
        );
    }

    let cases_json = serde_json::to_vec(&cases).map_err(|source| LockfileError::Json { source })?;
    let overall_lock_digest = compute_framed_sha256(&[suite_digest.as_bytes(), &cases_json]);

    Ok(TemperLockfile {
        format_version: "sha256-framed-v1".to_string(),
        suite_id: suite.suite.id.clone(),
        suite_version: suite.suite.version.clone(),
        suite_digest,
        cases,
        overall_lock_digest,
    })
}

pub fn verify_lockfile<P: AsRef<Path>>(
    suite: &SuiteSpec,
    lock: &TemperLockfile,
    root_dir: P,
) -> Result<(), LockfileError> {
    if lock.format_version != "sha256-framed-v1" {
        return Err(LockfileError::Drift {
            message: format!("unsupported format version: {}", lock.format_version),
        });
    }

    let current = generate_lockfile(suite, root_dir)?;

    if current.suite_digest != lock.suite_digest {
        return Err(LockfileError::Drift {
            message: format!(
                "suite specification digest drift: expected {}, found {}",
                lock.suite_digest, current.suite_digest
            ),
        });
    }

    if current.overall_lock_digest != lock.overall_lock_digest {
        return Err(LockfileError::Drift {
            message: format!(
                "overall lockfile drift: expected {}, found {}",
                lock.overall_lock_digest, current.overall_lock_digest
            ),
        });
    }

    Ok(())
}
