use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::artifact_paths::FRONTIER_LEADERBOARD_ARTIFACT;
use crate::hash::sha256_hex;
use crate::research_key::is_research_key;
use crate::research_source_ledger::embedded_research_source_keys;

use super::metrics::write_json;
use super::suite_inspect::{backend_suite_output_path, read_text_bounded};
use super::types::MAX_RELEASE_BENCHMARK_TEXT_BYTES;

pub(crate) const FRONTIER_LEADERBOARD_SCHEMA_VERSION: u32 = 1;
pub(crate) const FRONTIER_LEADERBOARD_SEMANTIC_VALIDATOR: &str =
    "release_benchmarks::validate_frontier_leaderboard_artifact_bytes";
pub(crate) const FRONTIER_LEADERBOARD_BASELINES_PATH: &str =
    "docs/optimization/FRONTIER_LEADERBOARD_BASELINES.toml";
const FRONTIER_LEADERBOARD_BASELINES_TOML: &str =
    include_str!("../../../docs/optimization/FRONTIER_LEADERBOARD_BASELINES.toml");
pub(crate) const FRONTIER_LEADERBOARD_REQUIRED_FIELDS: &[&str] = &[
    "schema_version",
    "selected_backend",
    "source_suite",
    "source_fingerprint",
    "source_tree_fingerprint",
    "source_artifact_count",
    "source_artifacts",
    "required_baseline_count",
    "covered_baseline_count",
    "missing_baselines",
    "row_count",
    "baselines",
    "rows",
    "blockers",
];

const FRONTIER_LEADERBOARD_REQUIRED_ROW_FIELDS: &[&str] = &[
    "baseline_id",
    "research_key",
    "baseline",
    "workload_family",
    "metric_family",
    "source_artifact",
    "family_id",
    "case_id",
    "selected_backend",
    "corpus_digest",
    "baseline_version",
    "output_digest",
    "cpu_digest",
    "gpu_digest",
    "throughput_gb_s_x1000_p50",
    "latency_wall_ns_p50",
    "memory_total_mib_p50",
    "transfer_bytes_p50",
    "unsupported_cases",
    "selected_plan_reason",
    "rejected_plan_reasons",
    "blockers",
];

static FRONTIER_BASELINE_CATALOG: OnceLock<Result<FrontierBaselineCatalog, String>> =
    OnceLock::new();

#[derive(Debug, Deserialize)]
struct FrontierBaselineCatalog {
    schema: FrontierBaselineCatalogSchema,
    baselines: Vec<FrontierBaseline>,
}

#[derive(Debug, Deserialize)]
struct FrontierBaselineCatalogSchema {
    version: u32,
    catalog: String,
}

#[derive(Debug, Deserialize)]
struct FrontierBaseline {
    id: String,
    research_key: String,
    baseline: String,
    workload_family: String,
    match_terms: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FrontierLeaderboard {
    schema_version: u32,
    selected_backend: &'static str,
    source_suite: String,
    source_fingerprint: Option<String>,
    source_tree_fingerprint: Option<String>,
    source_artifact_count: usize,
    source_artifacts: Vec<String>,
    required_baseline_count: usize,
    covered_baseline_count: usize,
    missing_baselines: Vec<String>,
    row_count: usize,
    baselines: Vec<FrontierBaselineEvidence>,
    rows: Vec<FrontierLeaderboardRow>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FrontierBaselineEvidence {
    id: String,
    research_key: String,
    baseline: String,
    workload_family: String,
    match_terms: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FrontierLeaderboardRow {
    baseline_id: String,
    research_key: String,
    baseline: String,
    workload_family: String,
    metric_family: String,
    source_artifact: String,
    family_id: String,
    case_id: String,
    selected_backend: Option<String>,
    corpus_digest: Option<String>,
    baseline_version: Option<String>,
    output_digest: Option<String>,
    cpu_digest: Option<String>,
    gpu_digest: Option<String>,
    throughput_gb_s_x1000_p50: Option<u64>,
    latency_wall_ns_p50: Option<u64>,
    memory_total_mib_p50: Option<u64>,
    transfer_bytes_p50: Option<u64>,
    unsupported_cases: Vec<String>,
    selected_plan_reason: String,
    rejected_plan_reasons: Vec<String>,
    blockers: Vec<String>,
}

fn baseline_catalog_result() -> &'static Result<FrontierBaselineCatalog, String> {
    FRONTIER_BASELINE_CATALOG.get_or_init(read_frontier_baseline_catalog)
}

fn baseline_catalog() -> &'static FrontierBaselineCatalog {
    crate::toml_config::data_or_exit(baseline_catalog_result())
}

fn read_frontier_baseline_catalog() -> Result<FrontierBaselineCatalog, String> {
    let catalog = crate::toml_config::parse_embedded_toml::<FrontierBaselineCatalog>(
        FRONTIER_LEADERBOARD_BASELINES_PATH,
        FRONTIER_LEADERBOARD_BASELINES_TOML,
    )?;
    validate_frontier_baseline_catalog(catalog)
}

fn validate_frontier_baseline_catalog(
    catalog: FrontierBaselineCatalog,
) -> Result<FrontierBaselineCatalog, String> {
    let mut failures = Vec::new();
    if catalog.schema.version != 1 {
        failures.push(format!(
            "schema.version must be 1, got {}",
            catalog.schema.version
        ));
    }
    if catalog.schema.catalog != "vyre-frontier-leaderboard-baselines" {
        failures.push(format!(
            "schema.catalog must be `vyre-frontier-leaderboard-baselines`, got `{}`",
            catalog.schema.catalog
        ));
    }
    if catalog.baselines.is_empty() {
        failures.push("baselines must not be empty".to_string());
    }
    let research_source_keys = match embedded_research_source_keys() {
        Ok(keys) => Some(keys),
        Err(error) => {
            failures.push(format!(
                "frontier baseline catalog cannot load research source ledger: {error}"
            ));
            None
        }
    };
    let mut ids = BTreeSet::new();
    for (index, baseline) in catalog.baselines.iter().enumerate() {
        for (field, value) in [
            ("id", baseline.id.as_str()),
            ("research_key", baseline.research_key.as_str()),
            ("baseline", baseline.baseline.as_str()),
            ("workload_family", baseline.workload_family.as_str()),
        ] {
            if value.trim().is_empty() {
                failures.push(format!("baselines[{index}].{field} must be non-empty"));
            }
        }
        if !ids.insert(baseline.id.as_str()) {
            failures.push(format!("duplicate baseline id `{}`", baseline.id.as_str()));
        }
        if !is_research_key(baseline.research_key.as_str()) {
            failures.push(format!(
                "baselines[{index}].research_key `{}` must use uppercase letters, digits, and underscores",
                baseline.research_key
            ));
        }
        if research_source_keys
            .as_ref()
            .is_some_and(|keys| !keys.contains(baseline.research_key.as_str()))
        {
            failures.push(format!(
                "baselines[{index}].research_key `{}` is not present in docs/optimization/RESEARCH_SOURCE_LEDGER.toml",
                baseline.research_key
            ));
        }
        if baseline.match_terms.is_empty()
            || baseline.match_terms.iter().any(|term| term.trim().is_empty())
        {
            failures.push(format!(
                "baselines[{index}].match_terms must contain non-empty terms"
            ));
        }
        if metric_family_for_workload(baseline.workload_family.as_str()) == "unknown" {
            failures.push(format!(
                "baselines[{index}].workload_family `{}` is not supported",
                baseline.workload_family
            ));
        }
    }
    if failures.is_empty() {
        Ok(catalog)
    } else {
        Err(format!(
            "Fix: {FRONTIER_LEADERBOARD_BASELINES_PATH} is invalid: {}",
            failures.join("; ")
        ))
    }
}

pub(super) fn write_frontier_leaderboard(workspace_root: &Path) {
    let baseline_catalog = baseline_catalog();
    let source_suite = backend_suite_output_path("cuda");
    let suite_path = workspace_root.join(&source_suite);
    let mut blockers = Vec::new();
    let suite = match read_json(&suite_path) {
        Ok(value) => value,
        Err(error) => {
            blockers.push(format!("frontier leaderboard source suite `{source_suite}` is unreadable: {error}"));
            Value::Null
        }
    };
    let mut source_fingerprint = None::<String>;
    let mut source_tree_fingerprint = None::<String>;
    let mut source_artifacts = Vec::new();
    let mut rows = Vec::new();
    let mut seen_rows = BTreeSet::new();
    let statuses = suite
        .get("artifact_statuses")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if statuses.is_empty() {
        blockers.push(format!(
            "frontier leaderboard source suite `{source_suite}` has no artifact_statuses"
        ));
    }
    for status in &statuses {
        remember_fingerprint(
            &mut source_fingerprint,
            status.get("source_fingerprint").and_then(nonblank_str),
            "source_fingerprint",
            &mut blockers,
        );
        remember_fingerprint(
            &mut source_tree_fingerprint,
            status.get("source_tree_fingerprint").and_then(nonblank_str),
            "source_tree_fingerprint",
            &mut blockers,
        );
        let Some(artifact) = status.get("path").and_then(nonblank_str) else {
            blockers.push("frontier leaderboard source suite has artifact_status without path".to_string());
            continue;
        };
        source_artifacts.push(artifact.to_string());
        let report_path = workspace_root.join(artifact);
        let report = match read_json(&report_path) {
            Ok(value) => value,
            Err(error) => {
                blockers.push(format!(
                    "frontier leaderboard source artifact `{artifact}` is unreadable: {error}"
                ));
                continue;
            }
        };
        remember_fingerprint(
            &mut source_fingerprint,
            report.get("source_fingerprint").and_then(nonblank_str),
            "source_fingerprint",
            &mut blockers,
        );
        remember_fingerprint(
            &mut source_tree_fingerprint,
            report.get("source_tree_fingerprint").and_then(nonblank_str),
            "source_tree_fingerprint",
            &mut blockers,
        );
        collect_rows_for_report(
            artifact,
            status,
            &report,
            &mut rows,
            &mut seen_rows,
            &mut blockers,
        );
    }
    source_artifacts.sort();
    source_artifacts.dedup();
    let covered = rows
        .iter()
        .map(|row| row.baseline_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_baselines = baseline_catalog
        .baselines
        .iter()
        .filter(|baseline| !covered.contains(&baseline.id))
        .map(|baseline| baseline.id.clone())
        .collect::<Vec<_>>();
    for missing in &missing_baselines {
        blockers.push(format!(
            "frontier leaderboard has no measured CUDA row for baseline `{missing}`"
        ));
    }
    let row_blocker_count = rows.iter().filter(|row| !row.blockers.is_empty()).count();
    if row_blocker_count > 0 {
        blockers.push(format!(
            "frontier leaderboard has {row_blocker_count} row(s) missing required VX-420 evidence fields"
        ));
    }
    if source_fingerprint.is_none() {
        blockers.push("frontier leaderboard has no source_fingerprint".to_string());
    }
    if source_tree_fingerprint.is_none() {
        blockers.push("frontier leaderboard has no source_tree_fingerprint".to_string());
    }
    let evidence = FrontierLeaderboard {
        schema_version: FRONTIER_LEADERBOARD_SCHEMA_VERSION,
        selected_backend: "cuda",
        source_suite,
        source_fingerprint,
        source_tree_fingerprint,
        source_artifact_count: source_artifacts.len(),
        source_artifacts,
        required_baseline_count: baseline_catalog.baselines.len(),
        covered_baseline_count: covered.len(),
        missing_baselines,
        row_count: rows.len(),
        baselines: baseline_catalog
            .baselines
            .iter()
            .map(|baseline| FrontierBaselineEvidence {
                id: baseline.id.clone(),
                research_key: baseline.research_key.clone(),
                baseline: baseline.baseline.clone(),
                workload_family: baseline.workload_family.clone(),
                match_terms: baseline.match_terms.clone(),
            })
            .collect(),
        rows,
        blockers,
    };
    write_json(&workspace_root.join(FRONTIER_LEADERBOARD_ARTIFACT), &evidence);
}

pub(crate) fn frontier_leaderboard_required_artifact_fields() -> Vec<&'static str> {
    FRONTIER_LEADERBOARD_REQUIRED_FIELDS.to_vec()
}

pub(crate) fn validate_frontier_leaderboard_artifact_bytes(bytes: &[u8]) -> Vec<String> {
    let mut blockers = Vec::new();
    let baseline_catalog = match baseline_catalog_result() {
        Ok(catalog) => catalog,
        Err(error) => {
            return vec![error.clone()];
        }
    };
    let value = match serde_json::from_slice::<Value>(bytes) {
        Ok(value) => value,
        Err(error) => {
            return vec![format!("frontier leaderboard artifact is not valid JSON: {error}")];
        }
    };
    for field in FRONTIER_LEADERBOARD_REQUIRED_FIELDS {
        if value.get(*field).is_none() {
            blockers.push(format!("frontier leaderboard artifact `{field}` is missing"));
        }
    }
    if value.get("schema_version").and_then(Value::as_u64)
        != Some(FRONTIER_LEADERBOARD_SCHEMA_VERSION.into())
    {
        blockers.push(format!(
            "frontier leaderboard artifact must use schema_version={FRONTIER_LEADERBOARD_SCHEMA_VERSION}"
        ));
    }
    if value.get("selected_backend").and_then(Value::as_str) != Some("cuda") {
        blockers.push("frontier leaderboard artifact selected_backend must be `cuda`".to_string());
    }
    if value.get("source_suite").and_then(Value::as_str)
        != Some("release/evidence/benchmarks/cuda-release-suite.json")
    {
        blockers.push(
            "frontier leaderboard artifact source_suite must be the CUDA release suite"
                .to_string(),
        );
    }
    for field in ["source_fingerprint", "source_tree_fingerprint"] {
        if value
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|raw| raw.trim().is_empty())
        {
            blockers.push(format!(
                "frontier leaderboard artifact `{field}` must be non-empty"
            ));
        }
    }
    let source_artifact_count = value
        .get("source_artifact_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    match value.get("source_artifacts").and_then(Value::as_array) {
        Some(source_artifacts) if source_artifacts.len() as u64 == source_artifact_count => {
            for (index, artifact) in source_artifacts.iter().enumerate() {
                if artifact.as_str().is_none_or(|raw| raw.trim().is_empty()) {
                    blockers.push(format!(
                        "frontier leaderboard artifact source_artifacts[{index}] must be non-empty"
                    ));
                }
            }
        }
        Some(source_artifacts) => {
            blockers.push(format!(
                "frontier leaderboard artifact source_artifact_count={source_artifact_count} does not match source_artifacts len {}",
                source_artifacts.len()
            ));
            for (index, artifact) in source_artifacts.iter().enumerate() {
                if artifact.as_str().is_none_or(|raw| raw.trim().is_empty()) {
                    blockers.push(format!(
                        "frontier leaderboard artifact source_artifacts[{index}] must be non-empty"
                    ));
                }
            }
        }
        None => blockers
            .push("frontier leaderboard artifact `source_artifacts` must be an array".to_string()),
    }
    if value
        .get("required_baseline_count")
        .and_then(Value::as_u64)
        != Some(baseline_catalog.baselines.len() as u64)
    {
        blockers.push(format!(
            "frontier leaderboard artifact required_baseline_count must be {}",
            baseline_catalog.baselines.len()
        ));
    }
    match value.get("baselines").and_then(Value::as_array) {
        Some(baselines) if baselines.len() == baseline_catalog.baselines.len() => {
            validate_frontier_baseline_evidence(&mut blockers, baseline_catalog, baselines);
        }
        Some(baselines) => {
            blockers.push(format!(
                "frontier leaderboard artifact baselines len {} must equal {}",
                baselines.len(),
                baseline_catalog.baselines.len()
            ));
            validate_frontier_baseline_evidence(&mut blockers, baseline_catalog, baselines);
        }
        None => blockers
            .push("frontier leaderboard artifact `baselines` must be an array".to_string()),
    }
    if value
        .get("missing_baselines")
        .and_then(Value::as_array)
        .is_some_and(|missing| !missing.is_empty())
    {
        blockers.push("frontier leaderboard artifact has missing baseline coverage".to_string());
    }
    match value.get("blockers").and_then(Value::as_array) {
        Some(blockers_value) if blockers_value.is_empty() => {}
        Some(blockers_value) => blockers.push(format!(
            "frontier leaderboard artifact contains {} blocker(s)",
            blockers_value.len()
        )),
        None => blockers.push("frontier leaderboard artifact `blockers` must be an array".to_string()),
    }
    match value.get("rows").and_then(Value::as_array) {
        Some(rows) => validate_frontier_leaderboard_rows(&mut blockers, baseline_catalog, &value, rows),
        None => blockers.push("frontier leaderboard artifact `rows` must be an array".to_string()),
    }
    blockers
}

fn validate_frontier_leaderboard_rows(
    blockers: &mut Vec<String>,
    baseline_catalog: &FrontierBaselineCatalog,
    value: &Value,
    rows: &[Value],
) {
    let row_count = value
        .get("row_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    if row_count != rows.len() as u64 {
        blockers.push(format!(
            "frontier leaderboard artifact row_count={row_count} does not match rows len {}",
            rows.len()
        ));
    }
    let covered = rows
        .iter()
        .filter_map(|row| row.get("baseline_id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    if value
        .get("covered_baseline_count")
        .and_then(Value::as_u64)
        != Some(covered.len() as u64)
    {
        blockers.push(
            "frontier leaderboard artifact covered_baseline_count must match distinct row baseline ids"
                .to_string(),
        );
    }
    for required in &baseline_catalog.baselines {
        if !covered.contains(required.id.as_str()) {
            blockers.push(format!(
                "frontier leaderboard artifact has no row for baseline `{}`",
                required.id
            ));
        }
    }
    let source_artifacts = value
        .get("source_artifacts")
        .and_then(Value::as_array)
        .map(|artifacts| {
            artifacts
                .iter()
                .filter_map(nonblank_value)
                .map(ToString::to_string)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    for (index, row) in rows.iter().enumerate() {
        for field in FRONTIER_LEADERBOARD_REQUIRED_ROW_FIELDS {
            if row.get(*field).is_none() {
                blockers.push(format!(
                    "frontier leaderboard artifact rows[{index}].{field} is missing"
                ));
            }
        }
        if row.get("selected_backend").and_then(Value::as_str) != Some("cuda") {
            blockers.push(format!(
                "frontier leaderboard artifact rows[{index}].selected_backend must be `cuda`"
            ));
        }
        validate_frontier_row_catalog_contract(blockers, baseline_catalog, index, row);
        validate_frontier_row_source_artifact(blockers, &source_artifacts, index, row);
        validate_frontier_row_reason_quality(blockers, index, row);
        validate_frontier_row_comparator_coverage(blockers, index, row);
        for field in [
            "corpus_digest",
            "baseline_version",
            "output_digest",
            "selected_plan_reason",
        ] {
            if row
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(|raw| raw.trim().is_empty())
            {
                blockers.push(format!(
                    "frontier leaderboard artifact rows[{index}].{field} must be non-empty"
                ));
            }
        }
        for field in [
            "throughput_gb_s_x1000_p50",
            "latency_wall_ns_p50",
            "memory_total_mib_p50",
            "transfer_bytes_p50",
        ] {
            if !row.get(field).and_then(Value::as_u64).is_some_and(|raw| raw > 0) {
                blockers.push(format!(
                    "frontier leaderboard artifact rows[{index}].{field} must be positive"
                ));
            }
        }
        for field in ["unsupported_cases", "rejected_plan_reasons", "blockers"] {
            if !row.get(field).is_some_and(Value::is_array) {
                blockers.push(format!(
                    "frontier leaderboard artifact rows[{index}].{field} must be an array"
                ));
            }
        }
        if row
            .get("blockers")
            .and_then(Value::as_array)
            .is_some_and(|row_blockers| !row_blockers.is_empty())
        {
            blockers.push(format!(
                "frontier leaderboard artifact rows[{index}] contains blocker(s)"
            ));
        }
    }
}

fn validate_frontier_baseline_evidence(
    blockers: &mut Vec<String>,
    baseline_catalog: &FrontierBaselineCatalog,
    baselines: &[Value],
) {
    for expected in &baseline_catalog.baselines {
        let Some(entry) = baselines
            .iter()
            .find(|baseline| baseline.get("id").and_then(Value::as_str) == Some(expected.id.as_str()))
        else {
            blockers.push(format!(
                "frontier leaderboard artifact baselines is missing catalog baseline `{}`",
                expected.id
            ));
            continue;
        };
        for (field, expected_value) in [
            ("research_key", expected.research_key.as_str()),
            ("baseline", expected.baseline.as_str()),
            ("workload_family", expected.workload_family.as_str()),
        ] {
            if entry.get(field).and_then(Value::as_str) != Some(expected_value) {
                blockers.push(format!(
                    "frontier leaderboard artifact baseline `{}` field `{field}` must match catalog `{expected_value}`",
                    expected.id
                ));
            }
        }
        let match_terms = entry
            .get("match_terms")
            .and_then(Value::as_array)
            .map(|terms| {
                terms
                    .iter()
                    .filter_map(nonblank_value)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if match_terms != expected.match_terms {
            blockers.push(format!(
                "frontier leaderboard artifact baseline `{}` match_terms must match catalog",
                expected.id
            ));
        }
    }
}

fn collect_rows_for_report(
    artifact: &str,
    status: &Value,
    report: &Value,
    rows: &mut Vec<FrontierLeaderboardRow>,
    seen_rows: &mut BTreeSet<String>,
    blockers: &mut Vec<String>,
) {
    let Some(cases) = report.get("cases").and_then(Value::as_array) else {
        blockers.push(format!(
            "frontier leaderboard source artifact `{artifact}` has no cases array"
        ));
        return;
    };
    let family_id = status
        .get("family_id")
        .and_then(nonblank_str)
        .unwrap_or("<unknown-family>");
    let selected_backend = report
        .get("selected_backend")
        .and_then(nonblank_str)
        .map(str::to_string);
    for case in cases {
        let case_id = case
            .get("id")
            .and_then(nonblank_str)
            .unwrap_or("<unknown-case>");
        let search_text = case_search_text(family_id, case_id, case);
        for baseline in baseline_catalog()
            .baselines
            .iter()
            .filter(|baseline| baseline_matches(baseline, &search_text))
        {
            let key = format!("{}::{artifact}::{case_id}", baseline.id.as_str());
            if !seen_rows.insert(key) {
                continue;
            }
            rows.push(build_row(
                baseline,
                artifact,
                family_id,
                case_id,
                selected_backend.clone(),
                case,
            ));
        }
    }
}

fn build_row(
    baseline: &FrontierBaseline,
    artifact: &str,
    family_id: &str,
    case_id: &str,
    selected_backend: Option<String>,
    case: &Value,
) -> FrontierLeaderboardRow {
    let cpu_digest = metric_p50(case, "cpu_digest").map(|digest| digest.to_string());
    let gpu_digest = metric_p50(case, "gpu_digest").map(|digest| digest.to_string());
    let output_digest = gpu_digest.clone().or_else(|| cpu_digest.clone());
    let corpus_digest = case
        .get("held_out_corpus_id")
        .and_then(nonblank_str)
        .or_else(|| case.get("workload_fingerprint").and_then(nonblank_str))
        .map(str::to_string);
    let baseline_version = baseline_contract_digest(baseline, case);
    let throughput_gb_s_x1000_p50 =
        first_metric_p50(case, &["wall_gb_s_x1000", "device_gb_s_x1000"]);
    let latency_wall_ns_p50 = first_metric_p50(case, &["wall_ns", "active_time_ns"]);
    let memory_total_mib_p50 = metric_p50(case, "memory_total_mib");
    let transfer_bytes_p50 = metric_p50(case, "transfer_bytes").or_else(|| {
        Some(metric_p50(case, "host_to_device_bytes")? + metric_p50(case, "device_to_host_bytes")?)
    });
    let unsupported_cases = string_array(case.get("unsupported_cases"));
    let rejected_plan_reasons = case
        .get("performance")
        .and_then(|performance| performance.get("violations"))
        .map(|violations| string_array(Some(violations)))
        .unwrap_or_default();
    let selected_plan_reason =
        selected_plan_reason(case, baseline.id.as_str(), selected_backend.as_deref());
    let mut blockers = Vec::new();
    require_present(&mut blockers, corpus_digest.as_deref(), "corpus_digest");
    require_present(&mut blockers, baseline_version.as_deref(), "baseline_version");
    require_present(&mut blockers, output_digest.as_deref(), "output_digest");
    require_positive(
        &mut blockers,
        throughput_gb_s_x1000_p50,
        "throughput_gb_s_x1000_p50",
    );
    require_positive(&mut blockers, latency_wall_ns_p50, "latency_wall_ns_p50");
    require_positive(&mut blockers, memory_total_mib_p50, "memory_total_mib_p50");
    require_positive(&mut blockers, transfer_bytes_p50, "transfer_bytes_p50");
    if selected_backend.as_deref() != Some("cuda") {
        blockers.push(format!(
            "selected_backend `{:?}` is not cuda",
            selected_backend
        ));
    }
    FrontierLeaderboardRow {
        baseline_id: baseline.id.clone(),
        research_key: baseline.research_key.clone(),
        baseline: baseline.baseline.clone(),
        workload_family: baseline.workload_family.clone(),
        metric_family: metric_family_for_workload(baseline.workload_family.as_str()).to_string(),
        source_artifact: artifact.to_string(),
        family_id: family_id.to_string(),
        case_id: case_id.to_string(),
        selected_backend,
        corpus_digest,
        baseline_version,
        output_digest,
        cpu_digest,
        gpu_digest,
        throughput_gb_s_x1000_p50,
        latency_wall_ns_p50,
        memory_total_mib_p50,
        transfer_bytes_p50,
        unsupported_cases,
        selected_plan_reason,
        rejected_plan_reasons,
        blockers,
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = read_text_bounded(path, MAX_RELEASE_BENCHMARK_TEXT_BYTES)
        .map_err(|error| error.to_string())?;
    serde_json::from_str::<Value>(&text).map_err(|error| error.to_string())
}

fn remember_fingerprint(
    current: &mut Option<String>,
    candidate: Option<&str>,
    field: &str,
    blockers: &mut Vec<String>,
) {
    let Some(candidate) = candidate else {
        return;
    };
    match current {
        Some(existing) if existing != candidate => blockers.push(format!(
            "frontier leaderboard {field} `{candidate}` does not match aggregate `{existing}`"
        )),
        Some(_) => {}
        None => *current = Some(candidate.to_string()),
    }
}

fn case_search_text(family_id: &str, case_id: &str, case: &Value) -> String {
    let mut parts = vec![family_id.to_string(), case_id.to_string()];
    for key in ["name", "owner_crate", "workload_class", "held_out_corpus_id"] {
        if let Some(value) = case.get(key).and_then(nonblank_str) {
            parts.push(value.to_string());
        }
    }
    if let Some(tags) = case.get("tags").and_then(Value::as_array) {
        parts.extend(
            tags.iter()
                .filter_map(nonblank_value)
                .map(ToString::to_string),
        );
    }
    parts.join(" ").to_ascii_lowercase()
}

fn baseline_matches(baseline: &FrontierBaseline, search_text: &str) -> bool {
    baseline
        .match_terms
        .iter()
        .any(|term| search_text.contains(term.as_str()))
}

fn validate_frontier_row_catalog_contract(
    blockers: &mut Vec<String>,
    baseline_catalog: &FrontierBaselineCatalog,
    index: usize,
    row: &Value,
) {
    let Some(baseline_id) = row.get("baseline_id").and_then(nonblank_str) else {
        return;
    };
    let Some(expected) = baseline_catalog
        .baselines
        .iter()
        .find(|baseline| baseline.id == baseline_id)
    else {
        blockers.push(format!(
            "frontier leaderboard artifact rows[{index}].baseline_id `{baseline_id}` is not in the baseline catalog"
        ));
        return;
    };
    for (field, expected_value) in [
        ("research_key", expected.research_key.as_str()),
        ("baseline", expected.baseline.as_str()),
        ("workload_family", expected.workload_family.as_str()),
        (
            "metric_family",
            metric_family_for_workload(expected.workload_family.as_str()),
        ),
    ] {
        if row.get(field).and_then(Value::as_str) != Some(expected_value) {
            blockers.push(format!(
                "frontier leaderboard artifact rows[{index}].{field} must match baseline catalog `{expected_value}`"
            ));
        }
    }
}

fn validate_frontier_row_source_artifact(
    blockers: &mut Vec<String>,
    source_artifacts: &BTreeSet<String>,
    index: usize,
    row: &Value,
) {
    let Some(source_artifact) = row.get("source_artifact").and_then(nonblank_str) else {
        blockers.push(format!(
            "frontier leaderboard artifact rows[{index}].source_artifact must be non-empty"
        ));
        return;
    };
    if !source_artifacts.contains(source_artifact) {
        blockers.push(format!(
            "frontier leaderboard artifact rows[{index}].source_artifact `{source_artifact}` is not listed in source_artifacts"
        ));
    }
}

fn validate_frontier_row_reason_quality(blockers: &mut Vec<String>, index: usize, row: &Value) {
    let Some(reason) = row.get("selected_plan_reason").and_then(nonblank_str) else {
        return;
    };
    let lower = reason.to_ascii_lowercase();
    let baseline_id = row.get("baseline_id").and_then(nonblank_str).unwrap_or_default();
    if !baseline_id.is_empty() && !reason.contains(baseline_id) {
        blockers.push(format!(
            "frontier leaderboard artifact rows[{index}].selected_plan_reason must name baseline_id `{baseline_id}`"
        ));
    }
    for marker in ["selected_backend", "case status", "performance_contract_passed"] {
        if !lower.contains(marker) {
            blockers.push(format!(
                "frontier leaderboard artifact rows[{index}].selected_plan_reason must include `{marker}`"
            ));
        }
    }
}

fn validate_frontier_row_comparator_coverage(blockers: &mut Vec<String>, index: usize, row: &Value) {
    let baseline_version = row
        .get("baseline_version")
        .and_then(nonblank_str)
        .unwrap_or_default();
    if !baseline_version.starts_with("frontier-baseline-contract:v1:") {
        blockers.push(format!(
            "frontier leaderboard artifact rows[{index}].baseline_version must be a frontier baseline contract digest"
        ));
    }
    let output_digest = row
        .get("output_digest")
        .and_then(nonblank_str)
        .unwrap_or_default();
    if output_digest.is_empty() {
        blockers.push(format!(
            "frontier leaderboard artifact rows[{index}].output_digest must be non-empty"
        ));
    } else if output_digest == baseline_version {
        blockers.push(format!(
            "frontier leaderboard artifact rows[{index}].output_digest must be independent of baseline_version"
        ));
    }
    let cpu_digest = row
        .get("cpu_digest")
        .and_then(nonblank_str)
        .unwrap_or_default();
    let gpu_digest = row
        .get("gpu_digest")
        .and_then(nonblank_str)
        .unwrap_or_default();
    if cpu_digest.is_empty() {
        blockers.push(format!(
            "frontier leaderboard artifact rows[{index}].cpu_digest must be non-empty"
        ));
    }
    if gpu_digest.is_empty() {
        blockers.push(format!(
            "frontier leaderboard artifact rows[{index}].gpu_digest must be non-empty"
        ));
    }
    if !cpu_digest.is_empty() && !gpu_digest.is_empty() && cpu_digest != gpu_digest {
        blockers.push(format!(
            "frontier leaderboard artifact rows[{index}].cpu_digest must match gpu_digest for comparator parity"
        ));
    }
    if !gpu_digest.is_empty() && !output_digest.is_empty() && output_digest != gpu_digest {
        blockers.push(format!(
            "frontier leaderboard artifact rows[{index}].output_digest must match gpu_digest"
        ));
    }
}

fn metric_family_for_workload(workload_family: &str) -> &'static str {
    match workload_family {
        "scan" | "scan-partitioning" => "scan-throughput-latency-memory-transfer",
        "ann-vector" | "out-of-core-vector" | "storage-ann-graph" => {
            "vector-latency-throughput-memory-transfer"
        }
        "vyre-native" => "vyre-plan-throughput-latency-memory-transfer",
        _ => "unknown",
    }
}

fn baseline_contract_digest(baseline: &FrontierBaseline, case: &Value) -> Option<String> {
    let baselines = case
        .get("contract")
        .and_then(|contract| contract.get("baselines"))
        .and_then(Value::as_array)?;
    let first = baselines.first()?;
    let material = format!(
        "frontier-baseline-contract:v1\nbaseline_id={}\nresearch_key={}\ncontract={}\n",
        baseline.id.as_str(),
        baseline.research_key.as_str(),
        serde_json::to_string(first).ok()?
    );
    Some(format!(
        "frontier-baseline-contract:v1:{}",
        sha256_hex(material.as_bytes())
    ))
}

fn selected_plan_reason(case: &Value, baseline_id: &str, selected_backend: Option<&str>) -> String {
    let status = case
        .get("status")
        .and_then(nonblank_str)
        .unwrap_or("<missing-status>");
    let performance_passed = case
        .get("performance")
        .and_then(|performance| performance.get("contract_passed"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    format!(
        "baseline `{baseline_id}` uses selected_backend `{}` with case status `{status}` and performance_contract_passed={performance_passed}",
        selected_backend.unwrap_or("<missing-backend>")
    )
}

fn metric_p50(case: &Value, metric: &str) -> Option<u64> {
    case.get("metrics")
        .and_then(|metrics| metrics.get(metric))
        .and_then(|metric| metric.get("p50"))
        .and_then(Value::as_u64)
}

fn first_metric_p50(case: &Value, metrics: &[&str]) -> Option<u64> {
    metrics.iter().find_map(|metric| metric_p50(case, metric))
}

fn require_present(blockers: &mut Vec<String>, value: Option<&str>, field: &str) {
    if value.is_none_or(|value| value.trim().is_empty()) {
        blockers.push(format!("missing required `{field}`"));
    }
}

fn require_positive(blockers: &mut Vec<String>, value: Option<u64>, field: &str) {
    if !value.is_some_and(|value| value > 0) {
        blockers.push(format!("missing positive `{field}`"));
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(nonblank_value)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn nonblank_value(value: &Value) -> Option<&str> {
    value.as_str().and_then(nonblank)
}

fn nonblank_str(value: &Value) -> Option<&str> {
    value.as_str().and_then(nonblank)
}

fn nonblank(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

#[cfg(test)]
mod tests {
    use super::{
        metric_family_for_workload, validate_frontier_baseline_catalog,
        validate_frontier_row_comparator_coverage, validate_frontier_row_reason_quality,
        FrontierBaseline, FrontierBaselineCatalog, FrontierBaselineCatalogSchema,
    };
    use serde_json::json;

    #[test]
    fn metric_family_rejects_unknown_workload_family() {
        assert_eq!(
            metric_family_for_workload("scan"),
            "scan-throughput-latency-memory-transfer"
        );
        assert_eq!(metric_family_for_workload("unexpected"), "unknown");
    }

    #[test]
    fn selected_plan_reason_requires_catalog_quality_markers() {
        let row = json!({
            "baseline_id": "hyperscan-vectorscan-scan",
            "selected_plan_reason": "fast"
        });
        let mut blockers = Vec::new();
        validate_frontier_row_reason_quality(&mut blockers, 0, &row);
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("baseline_id")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("selected_backend")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("case status")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("performance_contract_passed")));
    }

    #[test]
    fn comparator_coverage_requires_digest_parity() {
        let row = json!({
            "baseline_version": "frontier-baseline-contract:v1:abc",
            "output_digest": "gpu-a",
            "cpu_digest": "cpu-b",
            "gpu_digest": "gpu-c"
        });
        let mut blockers = Vec::new();
        validate_frontier_row_comparator_coverage(&mut blockers, 0, &row);

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("cpu_digest must match gpu_digest")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("output_digest must match gpu_digest")));

        let missing = json!({
            "baseline_version": "frontier-baseline-contract:v1:abc",
            "output_digest": "frontier-baseline-contract:v1:abc"
        });
        let mut blockers = Vec::new();
        validate_frontier_row_comparator_coverage(&mut blockers, 1, &missing);
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("output_digest must be independent")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("cpu_digest must be non-empty")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("gpu_digest must be non-empty")));
    }

    #[test]
    fn frontier_catalog_rejects_unknown_research_key() {
        let catalog = FrontierBaselineCatalog {
            schema: FrontierBaselineCatalogSchema {
                version: 1,
                catalog: "vyre-frontier-leaderboard-baselines".to_string(),
            },
            baselines: vec![FrontierBaseline {
                id: "unknown-source".to_string(),
                research_key: "NOT_IN_LEDGER".to_string(),
                baseline: "Unknown source baseline".to_string(),
                workload_family: "scan".to_string(),
                match_terms: vec!["scan".to_string()],
            }],
        };

        let error = validate_frontier_baseline_catalog(catalog)
            .expect_err("Fix: frontier baseline catalog must reject unknown research keys.");

        assert!(error.contains("NOT_IN_LEDGER"));
        assert!(error.contains("RESEARCH_SOURCE_LEDGER.toml"));
    }
}
