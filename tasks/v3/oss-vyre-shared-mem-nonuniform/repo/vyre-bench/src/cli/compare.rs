use crate::report::json::ReportSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use super::cli_report_io::read_report_bounded;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(super) struct ComparisonArtifact {
    pub(super) schema: String,
    pub(super) baseline: ComparisonSide,
    pub(super) candidate: ComparisonSide,
    pub(super) cases: Vec<ComparisonCase>,
    pub(super) regressed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct ComparisonSide {
    pub(super) run_id: String,
    pub(super) suite: String,
    pub(super) selected_backend: String,
    pub(super) profile_backend: String,
    pub(super) timing_quality: String,
    pub(super) source_fingerprint: String,
    pub(super) source_tree_fingerprint: String,
    pub(super) total_cases: usize,
    pub(super) failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(super) struct ComparisonCase {
    pub(super) id: String,
    pub(super) baseline_p50_ns: u64,
    pub(super) candidate_p50_ns: u64,
    pub(super) baseline_mean_ns: f64,
    pub(super) candidate_mean_ns: f64,
    pub(super) delta_fraction: Option<f64>,
    pub(super) delta_percent: Option<f64>,
    pub(super) p_value: Option<f64>,
    pub(super) verdict: String,
    pub(super) regressed: bool,
}

pub(super) fn compare_reports(
    baseline: &ReportSchema,
    candidate: &ReportSchema,
    output: Option<&str>,
) -> anyhow::Result<()> {
    let comparison = build_comparison_artifact(baseline, candidate)?;
    print_comparison_artifact(&comparison);
    if let Some(output) = output {
        write_comparison_artifact(&comparison, output)?;
    }
    if comparison.regressed {
        anyhow::bail!("One or more cases regressed by >1σ");
    }
    Ok(())
}

pub(super) fn build_comparison_artifact(
    baseline: &ReportSchema,
    candidate: &ReportSchema,
) -> anyhow::Result<ComparisonArtifact> {
    let baseline_cases: BTreeMap<_, _> = baseline
        .cases
        .iter()
        .map(|case| (case.id.as_str(), case))
        .collect();
    let mut cases = Vec::with_capacity(candidate.cases.len());
    for case in &candidate.cases {
        let baseline_case = baseline_cases
            .get(case.id.as_str())
            .ok_or_else(|| anyhow::anyhow!("candidate case `{}` has no baseline", case.id))?;
        let baseline_stats = baseline_case
            .metrics
            .get("wall_ns")
            .ok_or_else(|| anyhow::anyhow!("baseline case `{}` lacks wall_ns", case.id))?;
        let candidate_stats = case
            .metrics
            .get("wall_ns")
            .ok_or_else(|| anyhow::anyhow!("candidate case `{}` lacks wall_ns", case.id))?;
        let baseline_p50 = baseline_stats.p50;
        let candidate_p50 = candidate_stats.p50;
        let delta_fraction = if baseline_p50 == 0 {
            None
        } else {
            Some((candidate_p50 as f64 - baseline_p50 as f64) / baseline_p50 as f64)
        };
        let p_value = welch_p_value(baseline_stats, candidate_stats);
        let verdict = compare_verdict(delta_fraction, p_value);
        let regressed = candidate_stats.mean > baseline_stats.mean + baseline_stats.stddev;
        cases.push(ComparisonCase {
            id: case.id.clone(),
            baseline_p50_ns: baseline_p50,
            candidate_p50_ns: candidate_p50,
            baseline_mean_ns: baseline_stats.mean,
            candidate_mean_ns: candidate_stats.mean,
            delta_fraction,
            delta_percent: delta_fraction.map(|delta| delta * 100.0),
            p_value,
            verdict: verdict.to_string(),
            regressed,
        });
    }
    let regressed = cases.iter().any(|case| case.regressed);
    Ok(ComparisonArtifact {
        schema: "vyre-bench.compare.v1".to_string(),
        baseline: comparison_side(baseline),
        candidate: comparison_side(candidate),
        cases,
        regressed,
    })
}

fn comparison_side(report: &ReportSchema) -> ComparisonSide {
    let (profile_backend, timing_quality) = report
        .backend_profile
        .as_ref()
        .map(|profile| (profile.backend.as_str(), profile.timing_quality.as_str()))
        .unwrap_or(("unknown", "unknown"));
    ComparisonSide {
        run_id: report.run_id.clone(),
        suite: report.suite.clone(),
        selected_backend: report
            .selected_backend
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        profile_backend: profile_backend.to_string(),
        timing_quality: timing_quality.to_string(),
        source_fingerprint: report.source_fingerprint.clone(),
        source_tree_fingerprint: report.source_tree_fingerprint.clone(),
        total_cases: report.summary.total_cases,
        failed: report.summary.failed,
    }
}

fn print_comparison_artifact(comparison: &ComparisonArtifact) {
    print_compare_profile("baseline", &comparison.baseline);
    print_compare_profile("candidate", &comparison.candidate);
    println!(
        "{:<30} | {:<12} | {:<12} | {:<10} | {:<12} | {:<10}",
        "Benchmark", "Baseline", "Candidate", "Delta", "p-value", "Verdict"
    );
    println!(
        "------------------------------------------------------------------------------------------------"
    );
    for case in &comparison.cases {
        let delta = case
            .delta_percent
            .map(|value| format!("{value:+.2}%"))
            .unwrap_or_else(|| "n/a".to_string());
        let p_value = case
            .p_value
            .map(|value| format!("{value:.4}"))
            .unwrap_or_else(|| "n/a".to_string());
        println!(
            "{:<30} | {:<12} | {:<12} | {:<10} | {:<12} | {:<10}",
            case.id, case.baseline_p50_ns, case.candidate_p50_ns, delta, p_value, case.verdict
        );
    }
}

fn print_compare_profile(label: &str, side: &ComparisonSide) {
    println!(
        "{label}_selected_backend={} {label}_profile_backend={} {label}_timing_quality={}",
        side.selected_backend, side.profile_backend, side.timing_quality
    );
}

pub(super) fn write_comparison_artifact(
    comparison: &ComparisonArtifact,
    path: &str,
) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)?;
    serde_json::to_writer_pretty(file, comparison)?;
    Ok(())
}

pub(super) fn load_comparison_artifact(path: &str) -> anyhow::Result<ComparisonArtifact> {
    let bytes = read_report_bounded(std::path::Path::new(path))?;
    parse_comparison_artifact(&bytes)
}

pub(super) fn parse_comparison_artifact(bytes: &[u8]) -> anyhow::Result<ComparisonArtifact> {
    Ok(serde_json::from_slice(bytes)?)
}

pub(super) fn validate_comparison_expectations(
    comparison: &ComparisonArtifact,
    baseline_backend: &str,
    candidate_backend: &str,
    case_ids: &[String],
) -> anyhow::Result<()> {
    if comparison.schema != "vyre-bench.compare.v1" {
        anyhow::bail!(
            "comparison schema `{}` is not `vyre-bench.compare.v1`. Fix: regenerate comparison with current vyre-bench compare.",
            comparison.schema
        );
    }
    if comparison.baseline.profile_backend != baseline_backend {
        anyhow::bail!(
            "comparison baseline profile backend `{}` does not match expected `{baseline_backend}`. Fix: compare the intended baseline report.",
            comparison.baseline.profile_backend
        );
    }
    if comparison.candidate.profile_backend != candidate_backend {
        anyhow::bail!(
            "comparison candidate profile backend `{}` does not match expected `{candidate_backend}`. Fix: compare the intended candidate report.",
            comparison.candidate.profile_backend
        );
    }
    for (label, side) in [
        ("baseline", &comparison.baseline),
        ("candidate", &comparison.candidate),
    ] {
        if !matches!(
            side.timing_quality.as_str(),
            "host_only" | "host_enqueue_wait" | "device_timestamps" | "hardware_counters"
        ) {
            anyhow::bail!(
                "{label} timing quality `{}` is invalid. Fix: regenerate comparison from reports with DeviceTimingQuality::as_str() values.",
                side.timing_quality
            );
        }
    }
    if comparison.cases.is_empty() {
        anyhow::bail!("comparison contains zero cases. Fix: compare reports with overlapping benchmark cases.");
    }
    for case_id in case_ids {
        if !comparison.cases.iter().any(|case| case.id == *case_id) {
            anyhow::bail!(
                "comparison artifact lacks case `{case_id}`. Fix: compare reports generated with the intended --case selection."
            );
        }
    }
    let derived_regressed = comparison.cases.iter().any(|case| case.regressed);
    if comparison.regressed != derived_regressed {
        anyhow::bail!(
            "comparison regressed={} contradicts case-derived regressed={derived_regressed}. Fix: regenerate comparison from case evidence.",
            comparison.regressed
        );
    }
    Ok(())
}

fn compare_verdict(delta_fraction: Option<f64>, p_value: Option<f64>) -> &'static str {
    match (delta_fraction, p_value) {
        (Some(delta), Some(p)) if delta > 0.05 && p < 0.05 => "regress",
        (Some(delta), Some(p)) if delta < -0.05 && p < 0.05 => "improve",
        (Some(delta), _) if delta.abs() <= 0.05 => "flat",
        _ => "noisy",
    }
}

fn welch_p_value(
    baseline: &crate::api::metric::MetricStats,
    candidate: &crate::api::metric::MetricStats,
) -> Option<f64> {
    if baseline.samples < 2 || candidate.samples < 2 {
        return None;
    }
    let n1 = f64::from(baseline.samples);
    let n2 = f64::from(candidate.samples);
    let variance = baseline.stddev.powi(2) / n1 + candidate.stddev.powi(2) / n2;
    if variance <= f64::EPSILON {
        return (baseline.mean != candidate.mean)
            .then_some(0.0)
            .or(Some(1.0));
    }
    let t = (candidate.mean - baseline.mean).abs() / variance.sqrt();
    Some((2.0 * (1.0 - normal_cdf(t))).clamp(0.0, 1.0))
}

fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    sign * y
}
