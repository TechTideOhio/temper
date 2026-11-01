#![forbid(unsafe_code)]

use clap::{Args, Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use temper::economics::compute_case_economics;
use temper::envelope::{EnvelopeStatus, HarnessEnvelope};
use temper::ingest::{ingest_agent_harness_runs, load_trials_from_jsonl};
use temper::lockfile::{generate_lockfile, verify_lockfile, TemperLockfile};
use temper::metrics::{
    compute_suite_metrics_summary, projected_pass_at_k, projected_pass_hat_k, CaseMetrics,
};
use temper::repo_audit::audit_repository;
use temper::report::{export_otel_spans, export_reval_ratchet, generate_mermaid_xychart};
use temper::runner::{run_multi_trials, RunTrialOptions};
use temper::spec::SuiteSpec;
use temper::temperature::{
    classify_reliability_regime, compute_output_entropy, verify_trial_temperatures,
    ReliabilityRegime,
};

const SOFTFACTORY_PACK_STR: &str = include_str!("../packs/softfactory.toml");

#[derive(Parser, Debug)]
#[command(
    name = "temper",
    author = "TechTide Ohio",
    version = "0.1.0",
    about = "TideOS Eval Kit (Tool 7): pass@k vs pass^k multi-trial reliability evaluation engine at real sampling temperatures",
    long_about = None
)]
pub struct Cli {
    #[arg(long, global = true, help = "Emit canonical HarnessEnvelope<T> JSON")]
    pub json: bool,

    #[arg(
        long,
        global = true,
        default_value_t = true,
        help = "Ring terminal audio alert (\\x07) on reliability failure or anomaly"
    )]
    pub bell: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Initialize a declarative reliability evaluation suite")]
    Init(InitArgs),

    #[command(about = "Display the pass@k vs pass^k cliff table for a given per-run pass rate")]
    Table(TableArgs),

    #[command(about = "Score a single JSONL trial evidence file")]
    Score(ScoreArgs),

    #[command(
        about = "Evaluate and gate a multi-case reliability suite against real temperatures"
    )]
    Check(CheckArgs),

    #[command(about = "Execute bounded multi-trial runs of a command at real temperature")]
    Run(RunArgs),

    #[command(about = "Ingest multi-agent harness execution runs from agent-harness/runs")]
    Ingest(IngestArgs),

    #[command(about = "Generate a sha256-framed-v1 cryptographic lockfile")]
    Lock(LockArgs),

    #[command(about = "Verify suite evidence and digests against a cryptographic lockfile")]
    Verify(VerifyArgs),

    #[command(about = "Audit a repository for Soft Factory hygiene and zero em-dash compliance")]
    Repo(RepoArgs),

    #[command(
        about = "Export reliability metrics as Mermaid chart, OpenTelemetry spans, or reval ratchet"
    )]
    Export(ExportArgs),
}

#[derive(Args, Debug)]
pub struct InitArgs {
    #[arg(long, default_value = "softfactory", help = "Preset template pack")]
    pub pack: String,

    #[arg(
        long,
        default_value = "temper.toml",
        help = "Output path for suite file"
    )]
    pub out: PathBuf,
}

#[derive(Args, Debug)]
pub struct TableArgs {
    #[arg(
        long,
        default_value_t = 0.80,
        help = "Per-run single-trial pass probability (0.0 to 1.0)"
    )]
    pub p: f64,

    #[arg(
        long,
        default_value = "1,2,3,4,5,8,10",
        help = "Comma-separated list of k values"
    )]
    pub k: String,
}

#[derive(Args, Debug)]
pub struct ScoreArgs {
    #[arg(long, help = "Path to multi-trial JSONL evidence file")]
    pub trials: PathBuf,

    #[arg(long, default_value_t = 5, help = "Target k for evaluation")]
    pub target_k: u32,

    #[arg(long, default_value_t = 0.7, help = "Required sampling temperature")]
    pub temperature: f64,
}

#[derive(Args, Debug)]
pub struct CheckArgs {
    #[arg(long, default_value = "temper.toml", help = "Path to suite TOML file")]
    pub suite: PathBuf,

    #[arg(long, default_value = ".", help = "Root directory for relative paths")]
    pub root: PathBuf,

    #[arg(
        long,
        help = "Optional path to cryptographic lockfile to verify against"
    )]
    pub lock: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    #[arg(long, help = "Case identifier")]
    pub case: String,

    #[arg(long, help = "Command to execute for each trial")]
    pub cmd: String,

    #[arg(long, default_value_t = 5, help = "Number of trials to execute")]
    pub trials: usize,

    #[arg(long, default_value_t = 0.7, help = "Sampling temperature recorded")]
    pub temperature: f64,

    #[arg(long, default_value = "model", help = "Model identifier")]
    pub model: String,

    #[arg(long, default_value = "agent", help = "Agent identifier")]
    pub agent: String,

    #[arg(long, help = "Optional output JSONL file to write trials into")]
    pub out: Option<PathBuf>,

    #[arg(long, help = "Stop immediately on first trial failure (saves tokens)")]
    pub fail_fast: bool,
}

#[derive(Args, Debug)]
pub struct IngestArgs {
    #[arg(
        long,
        default_value = "Tools/agent-harness/runs",
        help = "Path to agent-harness runs directory"
    )]
    pub runs_dir: PathBuf,

    #[arg(long, default_value_t = 0.7, help = "Default sampling temperature")]
    pub temperature: f64,
}

#[derive(Args, Debug)]
pub struct LockArgs {
    #[arg(long, default_value = "temper.toml", help = "Path to suite TOML file")]
    pub suite: PathBuf,

    #[arg(long, default_value = ".", help = "Root directory for relative paths")]
    pub root: PathBuf,

    #[arg(
        long,
        default_value = "temper.lock.json",
        help = "Output path for lockfile"
    )]
    pub out: PathBuf,
}

#[derive(Args, Debug)]
pub struct VerifyArgs {
    #[arg(long, default_value = "temper.toml", help = "Path to suite TOML file")]
    pub suite: PathBuf,

    #[arg(long, default_value = ".", help = "Root directory for relative paths")]
    pub root: PathBuf,

    #[arg(
        long,
        default_value = "temper.lock.json",
        help = "Path to lockfile to verify"
    )]
    pub lock: PathBuf,
}

#[derive(Args, Debug)]
pub struct RepoArgs {
    #[arg(default_value = ".", help = "Path to repository to audit")]
    pub path: PathBuf,
}

#[derive(Args, Debug)]
pub struct ExportArgs {
    #[arg(long, default_value = "temper.toml", help = "Path to suite TOML file")]
    pub suite: PathBuf,

    #[arg(long, default_value = ".", help = "Root directory for relative paths")]
    pub root: PathBuf,

    #[arg(
        long,
        default_value = "mermaid",
        help = "Export format: mermaid | otel | reval"
    )]
    pub format: String,

    #[arg(long, help = "Optional output file path")]
    pub out: Option<PathBuf>,
}

fn ring_bell() {
    print!("\x07");
}

fn handle_init(args: &InitArgs, json: bool) -> Result<ExitCode, anyhow::Error> {
    if args.pack != "softfactory" {
        eprintln!("Unknown pack '{}', defaulting to softfactory", args.pack);
    }
    fs::write(&args.out, SOFTFACTORY_PACK_STR)?;
    if json {
        let env = HarnessEnvelope::success(
            format!("Initialized suite at {}", args.out.display()),
            serde_json::json!({ "path": args.out.display().to_string(), "pack": args.pack }),
        )
        .with_artifact(args.out.display().to_string())
        .with_next_action(format!("temper check --suite {}", args.out.display()));
        env.print_json()?;
    } else {
        println!("Initialized suite specification at {}", args.out.display());
    }
    Ok(ExitCode::SUCCESS)
}

fn handle_table(args: &TableArgs, json: bool) -> Result<ExitCode, anyhow::Error> {
    let p = args.p.clamp(0.0, 1.0);
    let mut k_vals = Vec::new();
    for part in args.k.split(',') {
        if let Ok(k) = part.trim().parse::<u32>() {
            k_vals.push(k);
        }
    }
    if k_vals.is_empty() {
        k_vals = vec![1, 2, 3, 4, 5, 8, 10];
    }

    let mut rows = Vec::new();
    for &k in &k_vals {
        let pass_at_k = projected_pass_at_k(p, k);
        let pass_hat_k = projected_pass_hat_k(p, k);
        let flake_gap = pass_at_k - pass_hat_k;
        rows.push(serde_json::json!({
            "k": k,
            "pass_at_k": pass_at_k,
            "pass_hat_k": pass_hat_k,
            "flake_gap": flake_gap
        }));
    }

    if json {
        let env = HarnessEnvelope::success(
            format!(
                "Reliability decay table for per-run pass rate {:.2}% across k={:?}",
                p * 100.0,
                k_vals
            ),
            serde_json::json!({ "p": p, "rows": rows }),
        );
        env.print_json()?;
    } else {
        println!("=========================================================================");
        println!(" The Reliability Cliff: pass@k (Capability Demo) vs pass^k (Production)");
        println!(" Per-run single-trial pass rate: {:.2}%", p * 100.0);
        println!("=========================================================================");
        println!(
            "{:<6} | {:<18} | {:<18} | {:<14}",
            "k", "pass@k (at least 1)", "pass^k (all k pass)", "Flake Wedge"
        );
        println!("-------+--------------------+--------------------+---------------");
        for &k in &k_vals {
            let at_k = projected_pass_at_k(p, k);
            let hat_k = projected_pass_hat_k(p, k);
            let gap = at_k - hat_k;
            println!(
                "{:<6} | {:>16.2}% | {:>16.2}% | {:>12.2}%",
                k,
                at_k * 100.0,
                hat_k * 100.0,
                gap * 100.0
            );
        }
        println!("=========================================================================");
        if (p - 0.80).abs() < 0.01 {
            println!("Note: Notice how at 80% per-run pass rate, pass@5 looks near perfect");
            println!(
                "(99.97%), while pass^5 collapses to 32.77% (~33%). Production lives in pass^k!"
            );
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn handle_score(args: &ScoreArgs, json: bool, bell: bool) -> Result<ExitCode, anyhow::Error> {
    let trials = load_trials_from_jsonl(&args.trials, true)?;
    if trials.is_empty() {
        if bell {
            ring_bell();
        }
        eprintln!("Error: trials file is empty");
        return Ok(ExitCode::from(2));
    }

    let cm = CaseMetrics::compute(
        trials.len(),
        trials.iter().filter(|t| t.passed).count(),
        &[1, 2, 3, 4, 5, 8, 10],
        args.target_k,
    );

    let temp_res = verify_trial_temperatures(&trials, args.temperature, 0.05);
    let entropy = compute_output_entropy(&trials);
    let economics = compute_case_economics(&trials, cm.target_pass_hat_k, args.target_k);
    let regime = classify_reliability_regime(cm.target_pass_at_k, cm.target_pass_hat_k, 0.90, 0.80);

    if temp_res.is_err() && bell {
        ring_bell();
    }

    if json {
        let env = HarnessEnvelope::success(
            format!(
                "Scored {} trials for case {}: pass@{k}={:.2}%, pass^{k}={:.2}%",
                cm.n_trials,
                trials[0].case_id,
                cm.target_pass_at_k * 100.0,
                cm.target_pass_hat_k * 100.0,
                k = args.target_k
            ),
            serde_json::json!({
                "case_id": trials[0].case_id,
                "metrics": cm,
                "regime": regime,
                "temperature_check": temp_res.as_ref().map_err(|e| e.to_string()),
                "entropy": entropy,
                "economics": economics,
            }),
        );
        env.print_json()?;
    } else {
        println!("=========================================================================");
        println!(" Case Reliability Evaluation: {}", trials[0].case_id);
        println!("=========================================================================");
        println!(" Trials count (n):     {}", cm.n_trials);
        println!(" Passed trials (c):    {}", cm.n_passed);
        println!(" Empirical pass rate:  {:.2}%", cm.empirical_p * 100.0);
        println!(
            " Wilson 95% CI:        [{:.2}%, {:.2}%]",
            cm.wilson_ci_95.0 * 100.0,
            cm.wilson_ci_95.1 * 100.0
        );
        println!(" Target k:             {}", args.target_k);
        println!(
            " pass@{k}:              {:.2}%",
            cm.target_pass_at_k * 100.0,
            k = args.target_k
        );
        println!(
            " pass^{k}:              {:.2}%",
            cm.target_pass_hat_k * 100.0,
            k = args.target_k
        );
        println!(" Flake Wedge:          {:.2}%", cm.flake_gap * 100.0);
        println!(
            " Reliability Regime:   {:?} ({})",
            regime,
            regime.description()
        );
        println!(" Temperature Check:    {:?}", temp_res);
        println!(
            " Output Entropy:       {:.2} bits ({} unique digests)",
            entropy.shannon_entropy_bits, entropy.unique_digests
        );
        println!(
            " Mean Duration:        {:.1} ms (p95: {:.1} ms)",
            economics.latency_ms.mean, economics.latency_ms.p95
        );
        println!(
            " Expected Cost / k:    ${:.4}",
            economics.expected_cost_per_reliable_k_usd
        );
        println!("=========================================================================");
    }

    if temp_res.is_err() {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn handle_check(args: &CheckArgs, json: bool, bell: bool) -> Result<ExitCode, anyhow::Error> {
    let suite = SuiteSpec::from_file(&args.suite)?;
    suite.validate()?;

    if let Some(ref lock_path) = args.lock {
        let lock_content = fs::read_to_string(lock_path)?;
        let lock: TemperLockfile = serde_json::from_str(&lock_content)?;
        if let Err(e) = verify_lockfile(&suite, &lock, &args.root) {
            if bell {
                ring_bell();
            }
            if json {
                let env = HarnessEnvelope::error(
                    format!("Lockfile drift detected: {e}"),
                    serde_json::json!({ "error": e.to_string() }),
                );
                env.print_json()?;
            } else {
                eprintln!("Error: lockfile drift detected: {e}");
            }
            return Ok(ExitCode::from(2));
        }
    }

    let mut case_results = Vec::new();
    let mut case_metrics_list = Vec::new();
    let mut weights = Vec::new();
    let mut has_failure = false;

    for case in &suite.cases {
        let trials_path = args.root.join(&case.trials_path);
        let trials = load_trials_from_jsonl(&trials_path, true)?;
        let target_k = case.target_k.unwrap_or(suite.suite.target_k);

        let cm = CaseMetrics::compute(
            trials.len(),
            trials.iter().filter(|t| t.passed).count(),
            &suite.suite.k_values,
            target_k,
        );

        let temp_res = verify_trial_temperatures(
            &trials,
            case.required_temperature
                .unwrap_or(suite.suite.required_temperature),
            suite.suite.temperature_tolerance,
        );

        let regime = classify_reliability_regime(
            cm.target_pass_at_k,
            cm.target_pass_hat_k,
            case.min_pass_at_k,
            case.min_pass_hat_k,
        );

        let weight = suite.strata.get(&case.stratum).copied().unwrap_or(1.0);

        let mut case_passed = true;

        if temp_res.is_err() && suite.suite.fail_on_temperature_spoof {
            case_passed = false;
        }

        if cm.target_pass_at_k < case.min_pass_at_k {
            case_passed = false;
        }

        if cm.target_pass_hat_k < case.min_pass_hat_k {
            case_passed = false;
        }

        if let Some(max_gap) = case.max_flake_gap {
            if cm.flake_gap > max_gap {
                case_passed = false;
            }
        }

        if case.severity == temper::spec::CaseSeverity::Blocker && !case_passed {
            has_failure = true;
        }

        case_results.push(serde_json::json!({
            "case_id": case.id,
            "title": case.title,
            "stratum": case.stratum,
            "severity": case.severity,
            "passed": case_passed,
            "metrics": cm,
            "regime": regime,
            "temperature_ok": temp_res.is_ok(),
        }));

        case_metrics_list.push(cm);
        weights.push(weight);
    }

    let summary = compute_suite_metrics_summary(&case_metrics_list, &weights, suite.suite.target_k);

    if summary.macro_avg_pass_at_k < suite.suite.min_suite_pass_at_k
        || summary.macro_avg_pass_hat_k < suite.suite.min_suite_pass_hat_k
    {
        has_failure = true;
    }

    if has_failure && bell {
        ring_bell();
    }

    if json {
        let status = if has_failure {
            EnvelopeStatus::Error
        } else {
            EnvelopeStatus::Success
        };
        let env = HarnessEnvelope {
            status,
            summary: format!(
                "Suite {}: pass@{k}={:.2}%, pass^{k}={:.2}% (macro avg), pooling distortion={:.2}%",
                suite.suite.id,
                summary.macro_avg_pass_at_k * 100.0,
                summary.macro_avg_pass_hat_k * 100.0,
                summary.jensen_pooling_distortion * 100.0,
                k = suite.suite.target_k
            ),
            next_actions: if has_failure {
                vec!["Review flaky cases and fix non-deterministic trial failures".to_string()]
            } else {
                vec!["Ready for production release gate".to_string()]
            },
            artifacts: vec![args.suite.display().to_string()],
            data: serde_json::json!({
                "suite_id": suite.suite.id,
                "summary": summary,
                "cases": case_results,
            }),
        };
        env.print_json()?;
    } else {
        println!("=========================================================================");
        println!(" TideOS Reliability Gate: {}", suite.suite.id);
        println!("=========================================================================");
        println!(" Target k:                 {}", summary.target_k);
        println!(
            " Macro Avg pass@{k}:        {:.2}% (Min required: {:.2}%)",
            summary.macro_avg_pass_at_k * 100.0,
            suite.suite.min_suite_pass_at_k * 100.0,
            k = summary.target_k
        );
        println!(
            " Macro Avg pass^{k}:        {:.2}% (Min required: {:.2}%)",
            summary.macro_avg_pass_hat_k * 100.0,
            suite.suite.min_suite_pass_hat_k * 100.0,
            k = summary.target_k
        );
        println!(
            " Naive Pooled pass^{k}:     {:.2}%",
            summary.naive_pooled_pass_hat_k * 100.0,
            k = summary.target_k
        );
        println!(
            " Jensen Pooling Distortion:{:>+6.2}%",
            summary.jensen_pooling_distortion * 100.0
        );
        println!("-------------------------------------------------------------------------");
        println!(
            "{:<24} | {:<8} | {:<10} | {:<10} | {:<16}",
            "Case", "Stratum", "pass@k", "pass^k", "Regime"
        );
        println!(
            "-------------------------+----------+------------+------------+------------------"
        );
        for c in &case_results {
            let id = c["case_id"].as_str().unwrap_or("unknown");
            let stratum = c["stratum"].as_str().unwrap_or("core");
            let at_k = c["metrics"]["target_pass_at_k"].as_f64().unwrap_or(0.0);
            let hat_k = c["metrics"]["target_pass_hat_k"].as_f64().unwrap_or(0.0);
            let regime = c["regime"].as_str().unwrap_or("unknown");
            println!(
                "{:<24} | {:<8} | {:>9.2}% | {:>9.2}% | {:<16}",
                id,
                stratum,
                at_k * 100.0,
                hat_k * 100.0,
                regime
            );
        }
        println!("=========================================================================");
        if has_failure {
            println!("GATE FAILED: One or more blocker cases or suite targets did not meet stability thresholds.");
        } else {
            println!("GATE PASSED: All reliability and temperature stability targets verified.");
        }
    }

    if has_failure {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn handle_run(args: &RunArgs, json: bool) -> Result<ExitCode, anyhow::Error> {
    let opts = RunTrialOptions {
        case_id: &args.case,
        command: &args.cmd,
        num_trials: args.trials,
        temperature: args.temperature,
        model: &args.model,
        agent: &args.agent,
        fail_fast: args.fail_fast,
    };

    let trials = run_multi_trials(&opts)?;

    if let Some(ref out_path) = args.out {
        let mut f_content = String::new();
        for t in &trials {
            f_content.push_str(&serde_json::to_string(t)?);
            f_content.push('\n');
        }
        fs::write(out_path, f_content)?;
    }

    let cm = CaseMetrics::compute(
        trials.len(),
        trials.iter().filter(|t| t.passed).count(),
        &[1, 3, 5],
        trials.len().min(5) as u32,
    );

    if json {
        let env = HarnessEnvelope::success(
            format!(
                "Executed {} trials for case {}: {} passed",
                trials.len(),
                args.case,
                cm.n_passed
            ),
            serde_json::json!({ "trials": trials, "metrics": cm }),
        );
        env.print_json()?;
    } else {
        println!(
            "Executed {} trials for case {}: {} passed ({:.1}%)",
            trials.len(),
            args.case,
            cm.n_passed,
            cm.empirical_p * 100.0
        );
    }

    Ok(ExitCode::SUCCESS)
}

fn handle_ingest(args: &IngestArgs, json: bool) -> Result<ExitCode, anyhow::Error> {
    let groups = ingest_agent_harness_runs(&args.runs_dir, args.temperature)?;
    let total_tasks = groups.len();
    let total_trials: usize = groups.values().map(Vec::len).sum();

    if json {
        let env = HarnessEnvelope::success(
            format!(
                "Ingested {} tasks ({} total trials) from {}",
                total_tasks,
                total_trials,
                args.runs_dir.display()
            ),
            serde_json::json!({
                "tasks_count": total_tasks,
                "trials_count": total_trials,
                "cases": groups.keys().collect::<Vec<_>>()
            }),
        );
        env.print_json()?;
    } else {
        println!(
            "Ingested {} tasks ({} total trials) from {}",
            total_tasks,
            total_trials,
            args.runs_dir.display()
        );
        for (case_id, trials) in &groups {
            let passed = trials.iter().filter(|t| t.passed).count();
            println!(
                " - {}: {} trials, {} passed ({:.1}%)",
                case_id,
                trials.len(),
                passed,
                (passed as f64) / (trials.len() as f64) * 100.0
            );
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn handle_lock(args: &LockArgs, json: bool) -> Result<ExitCode, anyhow::Error> {
    let suite = SuiteSpec::from_file(&args.suite)?;
    let lock = generate_lockfile(&suite, &args.root)?;
    let json_bytes = serde_json::to_string_pretty(&lock)?;
    fs::write(&args.out, json_bytes)?;

    if json {
        let env = HarnessEnvelope::success(
            format!("Generated lockfile at {}", args.out.display()),
            serde_json::json!({ "path": args.out.display().to_string(), "digest": lock.overall_lock_digest }),
        )
        .with_artifact(args.out.display().to_string());
        env.print_json()?;
    } else {
        println!("Generated lockfile at {}", args.out.display());
        println!("Overall lock digest: {}", lock.overall_lock_digest);
    }

    Ok(ExitCode::SUCCESS)
}

fn handle_verify(args: &VerifyArgs, json: bool, bell: bool) -> Result<ExitCode, anyhow::Error> {
    let suite = SuiteSpec::from_file(&args.suite)?;
    let lock_content = fs::read_to_string(&args.lock)?;
    let lock: TemperLockfile = serde_json::from_str(&lock_content)?;

    match verify_lockfile(&suite, &lock, &args.root) {
        Ok(()) => {
            if json {
                let env = HarnessEnvelope::success(
                    format!("Lockfile verification passed for {}", args.lock.display()),
                    serde_json::json!({ "status": "verified" }),
                );
                env.print_json()?;
            } else {
                println!("Lockfile verification passed: zero drift in suite and evidence digests.");
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            if bell {
                ring_bell();
            }
            if json {
                let env = HarnessEnvelope::error(
                    format!("Lockfile verification failed: {e}"),
                    serde_json::json!({ "error": e.to_string() }),
                );
                env.print_json()?;
            } else {
                eprintln!("Lockfile verification failed: {e}");
            }
            Ok(ExitCode::from(1))
        }
    }
}

fn handle_repo(args: &RepoArgs, json: bool, bell: bool) -> Result<ExitCode, anyhow::Error> {
    let report = audit_repository(&args.path);

    if !report.passed && bell {
        ring_bell();
    }

    if json {
        let status = if report.passed {
            EnvelopeStatus::Success
        } else {
            EnvelopeStatus::Error
        };
        let env = HarnessEnvelope {
            status,
            summary: format!(
                "Repository audit: {}/{} checks passed",
                report.passed_checks, report.total_checks
            ),
            next_actions: if report.passed {
                vec!["Repository meets Soft Factory standards".to_string()]
            } else {
                vec!["Remediate failing blocker checks".to_string()]
            },
            artifacts: Vec::new(),
            data: serde_json::json!(report),
        };
        env.print_json()?;
    } else {
        println!("=========================================================================");
        println!(" Soft Factory Repository Audit: {}", report.repo_path);
        println!("=========================================================================");
        for c in &report.checks {
            let mark = if c.passed { "PASS" } else { "FAIL" };
            println!("[{mark}] {:<26} ({:<7}): {}", c.name, c.severity, c.message);
        }
        println!("-------------------------------------------------------------------------");
        println!(
            "Total: {}/{} passed",
            report.passed_checks, report.total_checks
        );
        println!("=========================================================================");
    }

    if report.passed {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn handle_export(args: &ExportArgs, json: bool) -> Result<ExitCode, anyhow::Error> {
    let suite = SuiteSpec::from_file(&args.suite)?;
    let mut case_tuples = Vec::new();
    let mut case_metrics_list = Vec::new();
    let mut weights = Vec::new();

    for case in &suite.cases {
        let trials_path = args.root.join(&case.trials_path);
        let trials = load_trials_from_jsonl(&trials_path, false)?;
        let target_k = case.target_k.unwrap_or(suite.suite.target_k);
        let cm = CaseMetrics::compute(
            trials.len(),
            trials.iter().filter(|t| t.passed).count(),
            &suite.suite.k_values,
            target_k,
        );
        let regime = classify_reliability_regime(
            cm.target_pass_at_k,
            cm.target_pass_hat_k,
            case.min_pass_at_k,
            case.min_pass_hat_k,
        );
        let temp = case
            .required_temperature
            .unwrap_or(suite.suite.required_temperature);

        case_metrics_list.push(cm.clone());
        weights.push(suite.strata.get(&case.stratum).copied().unwrap_or(1.0));
        case_tuples.push((case.id.clone(), cm, regime, temp));
    }

    let summary = compute_suite_metrics_summary(&case_metrics_list, &weights, suite.suite.target_k);

    let output_str = match args.format.to_lowercase().as_str() {
        "mermaid" => {
            let mut out = String::new();
            for (id, cm, _, _) in &case_tuples {
                let at_pairs: Vec<(u32, f64)> = cm
                    .projected_pass_at_k
                    .iter()
                    .map(|(&k, &v)| (k, v))
                    .collect();
                let hat_pairs: Vec<(u32, f64)> = cm
                    .projected_pass_hat_k
                    .iter()
                    .map(|(&k, &v)| (k, v))
                    .collect();
                out.push_str(&generate_mermaid_xychart(id, &at_pairs, &hat_pairs));
                out.push('\n');
            }
            out
        }
        "otel" => {
            let slice: Vec<(&str, &CaseMetrics, ReliabilityRegime, f64)> = case_tuples
                .iter()
                .map(|(id, cm, reg, temp)| (id.as_str(), cm, *reg, *temp))
                .collect();
            let spans = export_otel_spans(&suite.suite.id, &slice);
            serde_json::to_string_pretty(&spans)?
        }
        "reval" => {
            let ratchet = export_reval_ratchet(&suite.suite.id, &summary);
            serde_json::to_string_pretty(&ratchet)?
        }
        _ => anyhow::bail!("Unsupported export format '{}'", args.format),
    };

    if let Some(ref out_path) = args.out {
        fs::write(out_path, &output_str)?;
        if json {
            let env = HarnessEnvelope::success(
                format!("Exported {} to {}", args.format, out_path.display()),
                serde_json::json!({ "format": args.format, "path": out_path.display().to_string() }),
            );
            env.print_json()?;
        } else {
            println!("Exported {} to {}", args.format, out_path.display());
        }
    } else {
        println!("{output_str}");
    }

    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let res = match &cli.command {
        Commands::Init(args) => handle_init(args, cli.json),
        Commands::Table(args) => handle_table(args, cli.json),
        Commands::Score(args) => handle_score(args, cli.json, cli.bell),
        Commands::Check(args) => handle_check(args, cli.json, cli.bell),
        Commands::Run(args) => handle_run(args, cli.json),
        Commands::Ingest(args) => handle_ingest(args, cli.json),
        Commands::Lock(args) => handle_lock(args, cli.json),
        Commands::Verify(args) => handle_verify(args, cli.json, cli.bell),
        Commands::Repo(args) => handle_repo(args, cli.json, cli.bell),
        Commands::Export(args) => handle_export(args, cli.json),
    };

    match res {
        Ok(code) => code,
        Err(e) => {
            if cli.bell {
                ring_bell();
            }
            if cli.json {
                let env = HarnessEnvelope::error(
                    format!("Command failed: {e}"),
                    serde_json::json!({ "error": e.to_string() }),
                );
                let _ = env.print_json();
            } else {
                eprintln!("Error: {e}");
            }
            ExitCode::from(2)
        }
    }
}
