//! Scoring: the quality and security analyzers for Python changes, the
//! weighted scorer, and the evaluation orchestrator
//! (`harness/evaluator/{quality,security,scorer,evaluate}.py`).
//!
//! The Python harness shells out to ruff and bandit and calls radon in
//! process. The spike shells out to the same ruff and bandit binaries and asks
//! the venv's Python to run radon's two API calls, because radon is a Python
//! library with no CLI mode that matches the harness's exact call.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::deadline::{budget_exhausted, clamp_timeout, Deadline};
use crate::error::EngineError;
use crate::json::{self, obj, Value};
use crate::process;
use crate::pyfmt::{py_round, py_splitlines, py_str};
use crate::tasks::walk_files;
use crate::tracer::TraceCollector;

const LINT_VIOLATIONS_PER_LOC_FLOOR: f64 = 0.5;
const CC_GOOD: f64 = 5.0;
const CC_BAD: f64 = 25.0;
const EFFICIENCY_TOKEN_BUDGET: f64 = 50_000.0;

const WEIGHTS: [(&str, f64); 5] = [
    ("functional", 0.50),
    ("quality", 0.15),
    ("security", 0.15),
    ("efficiency", 0.10),
    ("human_like", 0.10),
];

const EXT_TO_LANG: [(&str, &str); 17] = [
    ("py", "python"),
    ("pyi", "python"),
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("js", "javascript"),
    ("jsx", "javascript"),
    ("mjs", "javascript"),
    ("cjs", "javascript"),
    ("go", "go"),
    ("java", "java"),
    ("rs", "rust"),
    ("c", "c"),
    ("cc", "cpp"),
    ("cpp", "cpp"),
    ("cxx", "cpp"),
    ("hh", "cpp"),
    ("hpp", "cpp"),
];

const SKIP_DIRS: [&str; 5] = [".git", "node_modules", "target", "build", "zz_hidden_build"];

/// One metric's score (or `None` when it could not be computed) and its breakdown.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricResult {
    pub score: Option<f64>,
    pub details: Value,
}

impl MetricResult {
    fn unscored(details: Value) -> Self {
        Self {
            score: None,
            details,
        }
    }

    fn reason(text: &str) -> Self {
        Self::unscored(obj! { "reason" => text })
    }
}

fn extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn detect_language(path: &str) -> Option<&'static str> {
    let ext = extension(path);
    if ext == "hxx" {
        return Some("cpp");
    }
    EXT_TO_LANG
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, lang)| *lang)
}

fn has_cpp_sources(workspace: &Path) -> bool {
    walk_files(workspace).unwrap_or_default().iter().any(|rel| {
        let skipped = rel
            .components()
            .any(|c| SKIP_DIRS.contains(&c.as_os_str().to_string_lossy().as_ref()));
        !skipped && matches!(extension(&rel.to_string_lossy()).as_str(), "cc" | "cpp" | "cxx")
    })
}

/// Groups changed files by language in first-seen order, dropping unknown extensions.
pub fn group_by_language(files: &[String], workspace: &Path) -> Vec<(String, Vec<String>)> {
    fn push(grouped: &mut Vec<(String, Vec<String>)>, lang: &str, file: &str) {
        match grouped.iter_mut().find(|(l, _)| l == lang) {
            Some((_, list)) => list.push(file.to_owned()),
            None => grouped.push((lang.to_owned(), vec![file.to_owned()])),
        }
    }
    let mut grouped: Vec<(String, Vec<String>)> = Vec::new();
    let mut headers = Vec::new();
    for file in files {
        if extension(file) == "h" {
            headers.push(file.clone());
            continue;
        }
        if let Some(lang) = detect_language(file) {
            push(&mut grouped, lang, file);
        }
    }
    if !headers.is_empty() {
        let is_cpp = grouped.iter().any(|(l, _)| l == "cpp") || has_cpp_sources(workspace);
        let lang = if is_cpp { "cpp" } else { "c" };
        for header in &headers {
            push(&mut grouped, lang, header);
        }
    }
    grouped
}

fn clamp(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}

fn existing_abs_files(workspace: &Path, files: &[String]) -> Vec<PathBuf> {
    files
        .iter()
        .map(|f| workspace.join(f))
        .filter(|p| p.exists())
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .collect()
}

fn nonblank_loc(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .map(|text| {
            py_splitlines(&text, false)
                .iter()
                .filter(|line| !line.trim().is_empty())
                .count()
        })
        .unwrap_or(0)
}

fn python_executable() -> Option<PathBuf> {
    process::which("python").or_else(|| process::which("python3"))
}

fn ruff_violations(workspace: &Path, abs_files: &[PathBuf], remaining: Option<f64>) -> Option<i64> {
    let timeout = clamp_timeout(120, remaining)?;
    let mut command = Command::new("ruff");
    command
        .args(["check", "--output-format=json", "--no-cache"])
        .args(abs_files)
        .current_dir(workspace);
    let done = process::run(&mut command, Some(Duration::from_secs(timeout as u64))).ok()?;
    if done.timed_out {
        return None;
    }
    let text = if done.stdout.is_empty() { "[]" } else { done.stdout.as_str() };
    match json::parse(text) {
        Ok(Value::Array(items)) => Some(i64::try_from(items.len()).unwrap_or(i64::MAX)),
        Ok(Value::Object(entries)) => Some(i64::try_from(entries.len()).unwrap_or(i64::MAX)),
        Ok(_) | Err(_) => Some(0),
    }
}

const RADON_SNIPPET: &str = r#"
import json, sys
from radon.complexity import cc_visit
from radon.metrics import mi_visit
ccs, mis = [], []
for f in sys.argv[1:]:
    try:
        src = open(f, encoding="utf-8").read()
    except OSError:
        continue
    try:
        ccs.extend(block.complexity for block in cc_visit(src))
        mis.append(mi_visit(src, multi=True))
    except Exception:
        continue
print(json.dumps({"cc": ccs, "mi": mis}))
"#;

/// Average cyclomatic complexity and maintainability index via radon's API,
/// run in the venv's Python. `None` when radon cannot be reached at all.
fn radon_metrics(abs_files: &[PathBuf]) -> Option<(f64, Option<f64>)> {
    let python = python_executable()?;
    let mut command = Command::new(python);
    command.arg("-c").arg(RADON_SNIPPET).args(abs_files);
    let done = process::run(&mut command, Some(Duration::from_secs(120))).ok()?;
    if done.timed_out || done.exit_code != 0 {
        return None;
    }
    let parsed = json::parse(done.stdout.trim()).ok()?;
    let ccs: Vec<f64> = parsed
        .get("cc")?
        .as_array()?
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    let mis: Vec<f64> = parsed
        .get("mi")?
        .as_array()?
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    let avg_cc = if ccs.is_empty() {
        0.0
    } else {
        ccs.iter().sum::<f64>() / ccs.len() as f64
    };
    let mi = (!mis.is_empty()).then(|| mis.iter().sum::<f64>() / mis.len() as f64);
    Some((avg_cc, mi))
}

fn python_quality(workspace: &Path, files: &[String], remaining: Option<f64>) -> MetricResult {
    if process::which("ruff").is_none() {
        return MetricResult::unscored(obj! { "tool" => "ruff", "reason" => "ruff not on PATH" });
    }
    let abs_files = existing_abs_files(workspace, files);
    if abs_files.is_empty() {
        return MetricResult::reason("changed files no longer exist");
    }
    let loc = abs_files.iter().map(|p| nonblank_loc(p.as_path())).sum::<usize>().max(1);
    let Some(violations) = ruff_violations(workspace, &abs_files, remaining) else {
        return MetricResult::unscored(obj! {
            "tool" => "ruff+radon",
            "reason" => "ruff timed out or budget expired",
        });
    };
    let lint_score = clamp(1.0 - (violations as f64 / loc as f64) / LINT_VIOLATIONS_PER_LOC_FLOOR);
    let Some((avg_cc, mi)) = radon_metrics(&abs_files) else {
        return MetricResult::unscored(obj! {
            "tool" => "ruff+radon",
            "reason" => "radon unavailable: the spike runs radon through the venv's python and that call failed",
        });
    };
    let complexity_score = clamp((CC_BAD - avg_cc) / (CC_BAD - CC_GOOD));
    let mi_score = mi.map(|m| clamp(m / 100.0));
    let parts: [(&str, Option<f64>, f64); 3] = [
        ("lint", Some(lint_score), 0.4),
        ("complexity", Some(complexity_score), 0.3),
        ("maintainability", mi_score, 0.3),
    ];
    let present: Vec<(&str, f64, f64)> = parts
        .iter()
        .filter_map(|(name, score, weight)| score.map(|s| (*name, s, *weight)))
        .collect();
    let weight_sum: f64 = present.iter().map(|(_, _, w)| w).sum::<f64>();
    let weight_sum = if weight_sum == 0.0 { 1.0 } else { weight_sum };
    let score = py_round(
        present.iter().map(|(_, s, w)| s * w).sum::<f64>() / weight_sum,
        4,
    );
    let mut sub_scores = Value::object();
    for (name, value, _) in &present {
        sub_scores.insert(*name, py_round(*value, 4));
    }
    MetricResult {
        score: Some(score),
        details: obj! {
            "tool" => "ruff+radon",
            "loc" => loc,
            "violations" => violations,
            "avg_complexity" => avg_cc,
            "maintainability_index" => mi.map(|m| py_round(m, 2)),
            "sub_scores" => sub_scores,
        },
    }
}

fn is_test_file(filename: &str) -> bool {
    let path = Path::new(filename);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dirs: Vec<String> = path
        .parent()
        .map(|p| {
            p.components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    dirs.iter().any(|d| d == "tests" || d == "test")
        || name.starts_with("test_")
        || name.ends_with("_test.py")
        || name == "conftest.py"
}

pub fn score_from_counts(high: i64, medium: i64, low: i64) -> f64 {
    let penalty = 0.4 * high as f64 + 0.15 * medium as f64 + 0.05 * low as f64;
    py_round(clamp(1.0 - penalty), 4)
}

fn python_security(workspace: &Path, files: &[String], remaining: Option<f64>) -> MetricResult {
    if process::which("bandit").is_none() {
        return MetricResult::unscored(obj! { "tool" => "bandit", "reason" => "bandit not on PATH" });
    }
    let abs_files = existing_abs_files(workspace, files);
    if abs_files.is_empty() {
        return MetricResult::reason("changed files no longer exist");
    }
    let Some(timeout) = clamp_timeout(180, remaining) else {
        return MetricResult::unscored(obj! { "tool" => "bandit", "reason" => "run budget exceeded" });
    };
    let mut command = Command::new("bandit");
    command
        .args(["-f", "json", "-q"])
        .args(&abs_files)
        .current_dir(workspace);
    let done = match process::run(&mut command, Some(Duration::from_secs(timeout as u64))) {
        Ok(done) if !done.timed_out => done,
        _ => {
            return MetricResult::unscored(obj! { "tool" => "bandit", "reason" => "timed out" });
        }
    };
    let text = if done.stdout.is_empty() { "{}" } else { done.stdout.as_str() };
    let Ok(parsed) = json::parse(text) else {
        return MetricResult::unscored(obj! {
            "tool" => "bandit",
            "reason" => "could not parse bandit output",
        });
    };
    let results = parsed
        .get("results")
        .and_then(Value::as_array)
        .unwrap_or(&[]);
    let (mut high, mut medium, mut low, mut skipped) = (0i64, 0i64, 0i64, 0i64);
    for finding in results {
        let test_id = finding.get("test_id").and_then(Value::as_str);
        let filename = finding.get("filename").map(py_str).unwrap_or_default();
        if test_id == Some("B101") && is_test_file(&filename) {
            skipped += 1;
            continue;
        }
        match finding
            .get("issue_severity")
            .map(py_str)
            .unwrap_or_default()
            .to_uppercase()
            .as_str()
        {
            "HIGH" => high += 1,
            "MEDIUM" => medium += 1,
            "LOW" => low += 1,
            _ => {}
        }
    }
    let mut details = obj! { "tool" => "bandit", "high" => high, "medium" => medium, "low" => low };
    if skipped > 0 {
        details.insert("skipped_test_asserts", skipped);
    }
    MetricResult {
        score: Some(score_from_counts(high, medium, low)),
        details,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Metric {
    Quality,
    Security,
}

fn assess(metric: Metric, workspace: &Path, changed_files: &[String], remaining: Option<f64>) -> MetricResult {
    let by_lang = group_by_language(changed_files, workspace);
    if by_lang.is_empty() {
        return MetricResult::reason("no recognized source files changed");
    }
    let mut per_lang = Value::object();
    let mut scores = Vec::new();
    for (lang, files) in &by_lang {
        let result = if budget_exhausted(remaining) {
            MetricResult::reason("run budget exceeded")
        } else {
            match (metric, lang.as_str()) {
                (Metric::Quality, "python") => python_quality(workspace, files, remaining),
                (Metric::Security, "python") => python_security(workspace, files, remaining),
                (Metric::Quality, _) => MetricResult::reason("no quality analyzer for this language"),
                (Metric::Security, _) => MetricResult::reason("no security scanner for this language"),
            }
        };
        let mut details = result.details.clone();
        details.insert("score", result.score);
        per_lang.insert(lang.as_str(), details);
        if let Some(score) = result.score {
            scores.push(score);
        }
    }
    let overall = (!scores.is_empty()).then(|| py_round(scores.iter().sum::<f64>() / scores.len() as f64, 4));
    let mut details = obj! { "languages" => per_lang };
    if overall.is_none() {
        let reason = match metric {
            Metric::Quality => "no quality analyzer available for changed languages",
            Metric::Security => "no security scanner available for changed languages",
        };
        details.insert("reason", reason);
    }
    MetricResult {
        score: overall,
        details,
    }
}

pub fn assess_quality(workspace: &Path, changed_files: &[String], remaining: Option<f64>) -> MetricResult {
    assess(Metric::Quality, workspace, changed_files, remaining)
}

pub fn assess_security(workspace: &Path, changed_files: &[String], remaining: Option<f64>) -> MetricResult {
    assess(Metric::Security, workspace, changed_files, remaining)
}

/// Resource usage mapped to a 0 to 1 score (1.0 frugal, toward 0 profligate).
pub fn efficiency_score(total_tokens: i64, steps: i64) -> f64 {
    if total_tokens <= 0 && steps <= 0 {
        return 1.0;
    }
    let token_factor = (1.0 - total_tokens as f64 / EFFICIENCY_TOKEN_BUDGET).max(0.0);
    let step_factor = (1.0 - steps as f64 / 100.0).max(0.0);
    py_round(0.7 * token_factor + 0.3 * step_factor, 4)
}

/// The inputs `score_run` combines.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScoreInputs {
    pub functional: f64,
    pub total_tokens: i64,
    pub steps: i64,
    pub quality: Option<f64>,
    pub security: Option<f64>,
    pub human_like: Option<f64>,
}

/// Combines the five metrics into a score dict with a weighted total that
/// re-normalizes over whichever metrics are present.
pub fn score_run(inputs: ScoreInputs, extra: Option<Value>) -> Value {
    let mut metrics = obj! {
        "functional" => py_round(inputs.functional, 4),
        "quality" => inputs.quality.map(|q| py_round(q, 4)),
        "security" => inputs.security.map(|s| py_round(s, 4)),
        "efficiency" => efficiency_score(inputs.total_tokens, inputs.steps),
        "human_like" => inputs.human_like.map(|h| py_round(h, 4)),
    };
    let present: Vec<(f64, f64)> = WEIGHTS
        .iter()
        .filter_map(|(name, weight)| metrics.get(name).and_then(Value::as_f64).map(|v| (*weight, v)))
        .collect();
    let weight_sum: f64 = present.iter().map(|(w, _)| w).sum();
    let weight_sum = if weight_sum == 0.0 { 1.0 } else { weight_sum };
    let total = present.iter().map(|(w, v)| w * v).sum::<f64>() / weight_sum;
    metrics.insert("total", py_round(total, 4));
    if let Some(extra) = extra {
        metrics.extend(extra);
    }
    metrics
}

fn warn_if_unscored(metric: &str, result: &MetricResult) {
    if result.score.is_some() {
        return;
    }
    let mut reasons: Vec<String> = Vec::new();
    let mut consider = |detail: &Value| {
        if let Some(reason) = detail.get("reason") {
            let text = py_str(reason);
            if !text.is_empty() && !reasons.contains(&text) {
                reasons.push(text);
            }
        }
    };
    consider(&result.details);
    for (_, value) in result.details.as_object().unwrap_or(&[]) {
        consider(value);
    }
    reasons.sort();
    let note = if reasons.is_empty() {
        "no reason recorded".to_owned()
    } else {
        reasons.join("; ")
    };
    eprintln!("[vulcanbench] WARNING: {metric} score is None ({note})");
}

/// What `evaluate_run` needs beyond the collector and the deadline.
#[derive(Debug)]
pub struct EvaluateParams<'a> {
    pub functional: f64,
    pub total_tokens: i64,
    pub steps: i64,
    pub workspace: &'a Path,
    pub changed_files: &'a [String],
    pub judges_enabled: bool,
}

/// Computes every metric for a finished run (tests grader only; judges off).
///
/// # Errors
///
/// When a trace event cannot be written.
pub fn evaluate_run(
    params: &EvaluateParams<'_>,
    collector: &mut TraceCollector,
    deadline: &mut Deadline,
) -> Result<Value, EngineError> {
    let quality = if budget_exhausted(deadline.remaining_s()) {
        deadline.record_exceeded(collector, "quality", None)?;
        MetricResult::reason("run budget exceeded")
    } else {
        assess_quality(params.workspace, params.changed_files, deadline.remaining_s())
    };
    collector.record("quality_detail", quality.details.clone())?;

    let security = if budget_exhausted(deadline.remaining_s()) {
        deadline.record_exceeded(collector, "security", None)?;
        MetricResult::reason("run budget exceeded")
    } else {
        assess_security(params.workspace, params.changed_files, deadline.remaining_s())
    };
    collector.record("security_detail", security.details.clone())?;

    warn_if_unscored("quality", &quality);
    warn_if_unscored("security", &security);

    let human_like_details = if budget_exhausted(deadline.remaining_s()) {
        deadline.record_exceeded(collector, "human_like", None)?;
        obj! { "reason" => "run budget exceeded" }
    } else if !params.judges_enabled {
        obj! { "reason" => "judges disabled" }
    } else {
        obj! { "reason" => "no judge provider available (missing API key or invalid model)" }
    };
    collector.record("human_like_detail", human_like_details.clone())?;

    Ok(score_run(
        ScoreInputs {
            functional: params.functional,
            total_tokens: params.total_tokens,
            steps: params.steps,
            quality: quality.score,
            security: security.score,
            human_like: None,
        },
        Some(obj! {
            "metric_details" => obj! {
                "quality" => quality.details,
                "security" => security.details,
                "human_like" => human_like_details,
            },
        }),
    ))
}

/// `_budget_exceeded_scores`: the score dict when evaluation never started.
pub fn budget_exceeded_scores(functional: f64, total_tokens: i64, steps: i64) -> Value {
    score_run(
        ScoreInputs {
            functional,
            total_tokens,
            steps,
            ..ScoreInputs::default()
        },
        Some(obj! {
            "budget_exceeded" => true,
            "metric_details" => obj! {
                "quality" => obj! { "reason" => "run budget exceeded" },
                "security" => obj! { "reason" => "run budget exceeded" },
                "human_like" => obj! { "reason" => "run budget exceeded" },
            },
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scorer_matches_the_oracle_numbers() {
        assert_eq!(efficiency_score(400, 4), 0.9824);
        assert_eq!(efficiency_score(0, 0), 1.0);
        let scores = score_run(
            ScoreInputs {
                functional: 1.0,
                total_tokens: 400,
                steps: 4,
                quality: Some(1.0),
                security: Some(1.0),
                human_like: None,
            },
            None,
        );
        assert_eq!(
            scores.to_json(),
            r#"{"functional": 1.0, "quality": 1.0, "security": 1.0, "efficiency": 0.9824, "human_like": null, "total": 0.998}"#
        );
        assert_eq!(score_from_counts(1, 1, 1), 0.4);
    }

    #[test]
    fn language_grouping_keeps_first_seen_order() {
        let files = vec!["b.rs".to_owned(), "a.py".to_owned(), "c.py".to_owned(), "x.txt".to_owned()];
        let grouped = group_by_language(&files, Path::new("/nonexistent"));
        assert_eq!(
            grouped,
            vec![
                ("rust".to_owned(), vec!["b.rs".to_owned()]),
                ("python".to_owned(), vec!["a.py".to_owned(), "c.py".to_owned()]),
            ]
        );
        assert!(is_test_file("pkg/tests/test_x.py"));
        assert!(is_test_file("conftest.py"));
        assert!(!is_test_file("pkg/module.py"));
    }
}
