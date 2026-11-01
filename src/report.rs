use crate::metrics::{CaseMetrics, SuiteMetricsSummary};
use crate::temperature::ReliabilityRegime;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

pub fn generate_mermaid_xychart(
    case_id: &str,
    pass_at_k: &[(u32, f64)],
    pass_hat_k: &[(u32, f64)],
) -> String {
    let mut chart = String::new();
    let _ = writeln!(
        chart,
        "%% Mermaid xychart-beta: pass@k (demo) vs pass^k (production)"
    );
    let _ = writeln!(chart, "xychart-beta");
    let _ = writeln!(chart, "    title \"Reliability Decay Curve: {case_id}\"");

    let x_labels: Vec<String> = pass_at_k
        .iter()
        .map(|(k, _)| format!("\"k={k}\""))
        .collect();
    let _ = writeln!(chart, "    x-axis [{}]", x_labels.join(", "));
    let _ = writeln!(chart, "    y-axis \"Probability (%)\" 0 --> 100");

    let at_pcts: Vec<String> = pass_at_k
        .iter()
        .map(|(_, v)| format!("{:.1}", v * 100.0))
        .collect();
    let _ = writeln!(chart, "    line [{}]", at_pcts.join(", "));

    let hat_pcts: Vec<String> = pass_hat_k
        .iter()
        .map(|(_, v)| format!("{:.1}", v * 100.0))
        .collect();
    let _ = writeln!(chart, "    line [{}]", hat_pcts.join(", "));

    chart
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtelSpanExport {
    pub name: String,
    pub trace_id: String,
    pub span_id: String,
    pub attributes: Vec<OtelAttribute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtelAttribute {
    pub key: String,
    pub value: String,
}

pub fn export_otel_spans(
    suite_id: &str,
    case_results: &[(&str, &CaseMetrics, ReliabilityRegime, f64)],
) -> Vec<OtelSpanExport> {
    let mut spans = Vec::new();

    for (case_id, metrics, regime, temp) in case_results {
        let span = OtelSpanExport {
            name: format!("eval.case.reliability:{case_id}"),
            trace_id: format!("trace-{suite_id}-{case_id}"),
            span_id: format!("span-{case_id}"),
            attributes: vec![
                OtelAttribute {
                    key: "eval.tool".to_string(),
                    value: "temper".to_string(),
                },
                OtelAttribute {
                    key: "eval.suite".to_string(),
                    value: suite_id.to_string(),
                },
                OtelAttribute {
                    key: "eval.case_id".to_string(),
                    value: (*case_id).to_string(),
                },
                OtelAttribute {
                    key: "eval.target_k".to_string(),
                    value: metrics.target_k.to_string(),
                },
                OtelAttribute {
                    key: "eval.pass_at_k".to_string(),
                    value: format!("{:.4}", metrics.target_pass_at_k),
                },
                OtelAttribute {
                    key: "eval.pass_hat_k".to_string(),
                    value: format!("{:.4}", metrics.target_pass_hat_k),
                },
                OtelAttribute {
                    key: "eval.flake_gap".to_string(),
                    value: format!("{:.4}", metrics.flake_gap),
                },
                OtelAttribute {
                    key: "eval.temperature".to_string(),
                    value: format!("{temp:.2}"),
                },
                OtelAttribute {
                    key: "eval.regime".to_string(),
                    value: format!("{regime:?}"),
                },
            ],
        };
        spans.push(span);
    }

    spans
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevalRatchetExport {
    pub suite_id: String,
    pub target_k: u32,
    pub macro_avg_pass_at_k: f64,
    pub macro_avg_pass_hat_k: f64,
    pub naive_pooled_pass_hat_k: f64,
    pub jensen_pooling_distortion: f64,
    pub weighted_pass_hat_k: f64,
}

pub fn export_reval_ratchet(suite_id: &str, summary: &SuiteMetricsSummary) -> RevalRatchetExport {
    RevalRatchetExport {
        suite_id: suite_id.to_string(),
        target_k: summary.target_k,
        macro_avg_pass_at_k: summary.macro_avg_pass_at_k,
        macro_avg_pass_hat_k: summary.macro_avg_pass_hat_k,
        naive_pooled_pass_hat_k: summary.naive_pooled_pass_hat_k,
        jensen_pooling_distortion: summary.jensen_pooling_distortion,
        weighted_pass_hat_k: summary.weighted_pass_hat_k,
    }
}
