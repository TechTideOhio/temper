use crate::trial::{compute_trial_chain, TrialRecord};
use sha2::{Digest, Sha256};
use std::fmt::Write;
use std::process::Command;
use std::time::Instant;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("failed to spawn trial command: {source}")]
    Spawn {
        #[source]
        source: std::io::Error,
    },
    #[error("trial execution error: {message}")]
    Execution { message: String },
}

pub struct RunTrialOptions<'a> {
    pub case_id: &'a str,
    pub command: &'a str,
    pub num_trials: usize,
    pub temperature: f64,
    pub model: &'a str,
    pub agent: &'a str,
    pub fail_fast: bool,
}

pub fn run_multi_trials(opts: &RunTrialOptions) -> Result<Vec<TrialRecord>, RunnerError> {
    let mut trials = Vec::with_capacity(opts.num_trials);

    for idx in 0..opts.num_trials {
        let start = Instant::now();

        // On Windows run via powershell or cmd, on Unix run via sh
        #[cfg(target_os = "windows")]
        let output_res = Command::new("powershell")
            .args(["-NoProfile", "-Command", opts.command])
            .output();

        #[cfg(not(target_os = "windows"))]
        let output_res = Command::new("sh").args(["-c", opts.command]).output();

        let duration_ms = start.elapsed().as_millis() as u64;

        let (passed, output_bytes, error_category) = match output_res {
            Ok(out) => {
                let passed = out.status.success();
                let err_cat = if passed {
                    None
                } else {
                    Some(format!("exit_code_{}", out.status.code().unwrap_or(1)))
                };
                (passed, out.stdout, err_cat)
            }
            Err(e) => (false, Vec::new(), Some(format!("spawn_error: {e}"))),
        };

        let output_digest = if !output_bytes.is_empty() {
            let mut hasher = Sha256::new();
            hasher.update(&output_bytes);
            let result = hasher.finalize();
            let mut hex = String::with_capacity(64);
            for byte in result {
                let _ = write!(hex, "{byte:02x}");
            }
            Some(hex)
        } else {
            None
        };

        let record = TrialRecord {
            case_id: opts.case_id.to_string(),
            trial_index: idx,
            passed,
            score: if passed { 1.0 } else { 0.0 },
            temperature: opts.temperature,
            model: opts.model.to_string(),
            agent: opts.agent.to_string(),
            duration_ms,
            cost_usd: 0.0,
            tokens_total: 0,
            error_category,
            output_digest,
            prev_hash: None,
            trial_hash: None,
        };

        trials.push(record);

        if opts.fail_fast && !passed {
            break;
        }
    }

    compute_trial_chain(&mut trials);
    Ok(trials)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_executes_simple_echo() {
        let opts = RunTrialOptions {
            case_id: "test-echo",
            command: "echo hello-world",
            num_trials: 2,
            temperature: 0.7,
            model: "mock-model",
            agent: "mock-agent",
            fail_fast: false,
        };

        let trials = run_multi_trials(&opts).expect("execution succeeded");
        assert_eq!(trials.len(), 2);
        assert!(trials[0].passed);
        assert!(trials[0].prev_hash.is_some());
        assert!(trials[0].trial_hash.is_some());
    }
}
