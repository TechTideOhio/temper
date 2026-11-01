#![forbid(unsafe_code)]

pub mod economics;
pub mod envelope;
pub mod ingest;
pub mod lockfile;
pub mod metrics;
pub mod repo_audit;
pub mod report;
pub mod runner;
pub mod spec;
pub mod temperature;
pub mod trial;

pub use economics::{compute_case_economics, CaseEconomics, DistributionStats};
pub use envelope::{EnvelopeStatus, HarnessEnvelope};
pub use ingest::{ingest_agent_harness_runs, load_trials_from_jsonl};
pub use lockfile::{generate_lockfile, verify_lockfile, CaseLockEntry, TemperLockfile};
pub use metrics::{
    compute_suite_metrics_summary, projected_pass_at_k, projected_pass_hat_k, unbiased_pass_at_k,
    unbiased_pass_hat_k, CaseMetrics, SuiteMetricsSummary,
};
pub use repo_audit::{audit_repository, RepoAuditReport, RepoCheckResult};
pub use report::{export_otel_spans, export_reval_ratchet, generate_mermaid_xychart};
pub use runner::{run_multi_trials, RunTrialOptions};
pub use spec::{CaseSeverity, CaseSpec, EvalKind, SuiteMeta, SuiteSpec};
pub use temperature::{
    classify_reliability_regime, compute_output_entropy, verify_trial_temperatures, OutputEntropy,
    ReliabilityRegime, TemperatureError,
};
pub use trial::{compute_trial_chain, verify_trial_chain, ChainError, TrialRecord};
