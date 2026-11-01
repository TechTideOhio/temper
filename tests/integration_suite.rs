#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use temper::lockfile::{generate_lockfile, verify_lockfile};
use temper::metrics::{
    compute_suite_metrics_summary, projected_pass_at_k, projected_pass_hat_k, unbiased_pass_at_k,
    unbiased_pass_hat_k, CaseMetrics,
};
use temper::repo_audit::audit_repository;
use temper::spec::SuiteSpec;
use temper::temperature::{
    classify_reliability_regime, verify_trial_temperatures, ReliabilityRegime, TemperatureError,
};
use temper::trial::{compute_trial_chain, verify_trial_chain, ChainError, TrialRecord};

#[test]
fn test_80_percent_cliff_identity() {
    let p = 0.80;
    let hat_5 = projected_pass_hat_k(p, 5);
    let at_5 = projected_pass_at_k(p, 5);

    // 0.80^5 = 0.32768 (roughly 33%)
    assert!((hat_5 - 0.32768).abs() < 1e-4);
    // 1 - 0.20^5 = 0.99968 (demo success rate)
    assert!((at_5 - 0.99968).abs() < 1e-4);

    let regime = classify_reliability_regime(at_5, hat_5, 0.90, 0.80);
    assert_eq!(regime, ReliabilityRegime::DemoTrapFlaky);
}

#[test]
fn test_algebraic_ordering_invariant() {
    let p_values = [0.10, 0.30, 0.50, 0.75, 0.80, 0.95, 1.0];
    let k_values = [1, 2, 3, 5, 10];

    for &p in &p_values {
        for &k in &k_values {
            let pass_at_1 = projected_pass_at_k(p, 1);
            let pass_at_k = projected_pass_at_k(p, k);
            let pass_hat_k = projected_pass_hat_k(p, k);

            assert!(pass_hat_k <= pass_at_1 + 1e-9);
            assert!(pass_at_1 <= pass_at_k + 1e-9);
            assert!(pass_hat_k <= pass_at_k + 1e-9);
            assert!(pass_at_k <= 1.0 + 1e-9);
            assert!(pass_hat_k >= 0.0);
        }
    }
}

#[test]
fn test_unbiased_combinatorial_calculations() {
    // n=5, c=5: pass^5 = 1.0, pass@5 = 1.0
    assert_eq!(unbiased_pass_hat_k(5, 5, 5), Some(1.0));
    assert_eq!(unbiased_pass_at_k(5, 5, 5), Some(1.0));

    // n=5, c=4: pass^5 = 0.0, pass@5 = 1.0
    assert_eq!(unbiased_pass_hat_k(5, 4, 5), Some(0.0));
    assert_eq!(unbiased_pass_at_k(5, 4, 5), Some(1.0));

    // n=10, c=9, k=5:
    // pass^5 = (9/10)*(8/9)*(7/8)*(6/7)*(5/6) = 5/10 = 0.50
    let hat_5 = unbiased_pass_hat_k(10, 9, 5).unwrap();
    assert!((hat_5 - 0.50).abs() < 1e-5);
}

#[test]
fn test_merkle_provenance_chain_integrity() {
    let mut trials = vec![
        TrialRecord {
            case_id: "case-alpha".to_string(),
            trial_index: 0,
            passed: true,
            score: 1.0,
            temperature: 0.7,
            model: "claude-3-5-sonnet".to_string(),
            agent: "claude".to_string(),
            duration_ms: 1000,
            cost_usd: 0.01,
            tokens_total: 400,
            error_category: None,
            output_digest: Some("hash1".to_string()),
            prev_hash: None,
            trial_hash: None,
        },
        TrialRecord {
            case_id: "case-alpha".to_string(),
            trial_index: 1,
            passed: false,
            score: 0.0,
            temperature: 0.7,
            model: "claude-3-5-sonnet".to_string(),
            agent: "claude".to_string(),
            duration_ms: 800,
            cost_usd: 0.008,
            tokens_total: 300,
            error_category: Some("timeout".to_string()),
            output_digest: None,
            prev_hash: None,
            trial_hash: None,
        },
    ];

    compute_trial_chain(&mut trials);
    assert!(verify_trial_chain(&trials).is_ok());

    // Tampering with score must fail verification
    trials[0].score = 0.5;
    assert!(matches!(
        verify_trial_chain(&trials),
        Err(ChainError::TamperedHash { index: 0, .. })
    ));
}

#[test]
fn test_temperature_spoof_detection() {
    let greedy_trial = TrialRecord {
        case_id: "case-spoof".to_string(),
        trial_index: 0,
        passed: true,
        score: 1.0,
        temperature: 0.0, // greedy 0.0
        model: "claude-3-5-sonnet".to_string(),
        agent: "claude".to_string(),
        duration_ms: 900,
        cost_usd: 0.01,
        tokens_total: 400,
        error_category: None,
        output_digest: None,
        prev_hash: None,
        trial_hash: None,
    };

    let res = verify_trial_temperatures(&[greedy_trial], 0.7, 0.05);
    assert!(matches!(
        res,
        Err(TemperatureError::GreedySamplingForbidden { .. })
    ));
}

#[test]
fn test_jensen_inequality_pooling_distortion() {
    let cm_a = CaseMetrics::compute(5, 5, &[2], 2);
    let cm_b = CaseMetrics::compute(5, 0, &[2], 2);

    let summary = compute_suite_metrics_summary(&[cm_a, cm_b], &[1.0, 1.0], 2);
    assert!((summary.macro_avg_pass_hat_k - 0.50).abs() < 1e-4);
    assert!((summary.naive_pooled_pass_hat_k - 0.25).abs() < 1e-4);
    assert!((summary.jensen_pooling_distortion - (-0.25)).abs() < 1e-4);
}

#[test]
fn test_softfactory_suite_lock_and_verify() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let suite_path = manifest_dir.join("packs/softfactory.toml");
    let suite = SuiteSpec::from_file(&suite_path).unwrap();

    let lock = generate_lockfile(&suite, &manifest_dir).unwrap();
    assert_eq!(lock.suite_id, "softfactory-reliability");
    assert_eq!(lock.cases.len(), 3);

    // Verify lockfile produces zero drift
    assert!(verify_lockfile(&suite, &lock, &manifest_dir).is_ok());
}

#[test]
fn test_repo_audit_on_temper_crate() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let report = audit_repository(&manifest_dir);

    for c in &report.checks {
        if !c.passed {
            eprintln!("Failing check: {} ({}) - {}", c.name, c.severity, c.message);
        }
    }

    assert!(report.passed, "Repository audit must pass on Tools/temper");
    assert_eq!(report.passed_checks, 8);
}
