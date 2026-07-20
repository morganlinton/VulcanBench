use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::{Map, Value};

static CURRENT_SOURCE_FINGERPRINTS: OnceLock<Mutex<BTreeMap<PathBuf, String>>> = OnceLock::new();
static CURRENT_SOURCE_TREE_FINGERPRINTS: OnceLock<Mutex<BTreeMap<PathBuf, String>>> =
    OnceLock::new();
const MAX_BENCHMARK_EVIDENCE_SEMANTIC_TEXT_BYTES: u64 = 16_777_216;
const BENCHMARK_SCHEMA_DIGEST_CHAIN_PREFIX: &str = "benchmark-schema-digest-chain:v1:";

pub(crate) fn benchmark_schema_digest_chain_value(
    artifact_kind: &str,
    artifact_schema_version: u32,
    source_digest: &str,
    command_digest: &str,
    config_digest: &str,
    hardware_digest: &str,
    dataset_digest: &str,
    comparator_version: &str,
) -> Value {
    let material = benchmark_schema_digest_chain_material(
        artifact_kind,
        artifact_schema_version,
        source_digest,
        command_digest,
        config_digest,
        hardware_digest,
        dataset_digest,
        comparator_version,
    );
    let digest = format!(
        "{BENCHMARK_SCHEMA_DIGEST_CHAIN_PREFIX}{}",
        crate::hash::sha256_hex(material.as_bytes())
    );
    serde_json::json!({
        "schema": "benchmark-schema-digest-chain",
        "schema_version": 1,
        "artifact_kind": artifact_kind,
        "artifact_schema_version": artifact_schema_version,
        "source_digest": source_digest,
        "command_digest": command_digest,
        "config_digest": config_digest,
        "hardware_digest": hardware_digest,
        "dataset_digest": dataset_digest,
        "comparator_version": comparator_version,
        "digest": digest,
    })
}

pub(crate) fn benchmark_schema_digest_chain_issues(
    evidence: &str,
    artifact: &Value,
    expected_artifact_kind: &str,
) -> Vec<String> {
    let mut issues = Vec::new();
    let Some(chain) = artifact.get("schema_digest_chain") else {
        issues.push(format!("{evidence}: missing schema_digest_chain"));
        return issues;
    };
    if chain.get("schema").and_then(Value::as_str) != Some("benchmark-schema-digest-chain") {
        issues.push(format!(
            "{evidence}: schema_digest_chain.schema must be benchmark-schema-digest-chain"
        ));
    }
    if chain.get("schema_version").and_then(Value::as_u64) != Some(1) {
        issues.push(format!(
            "{evidence}: schema_digest_chain.schema_version must be 1"
        ));
    }
    if chain.get("artifact_kind").and_then(Value::as_str) != Some(expected_artifact_kind) {
        issues.push(format!(
            "{evidence}: schema_digest_chain.artifact_kind must be `{expected_artifact_kind}`"
        ));
    }
    let artifact_schema_version = artifact
        .get("schema_version")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if chain
        .get("artifact_schema_version")
        .and_then(Value::as_u64)
        != Some(artifact_schema_version)
    {
        issues.push(format!(
            "{evidence}: schema_digest_chain.artifact_schema_version must match artifact schema_version={artifact_schema_version}"
        ));
    }
    for field in [
        "source_digest",
        "command_digest",
        "config_digest",
        "hardware_digest",
        "dataset_digest",
        "comparator_version",
    ] {
        if chain
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            issues.push(format!(
                "{evidence}: schema_digest_chain.{field} is blank or missing"
            ));
        }
    }
    let digest = chain.get("digest").and_then(Value::as_str).unwrap_or("");
    if !digest.starts_with(BENCHMARK_SCHEMA_DIGEST_CHAIN_PREFIX) {
        issues.push(format!(
            "{evidence}: schema_digest_chain.digest must start with {BENCHMARK_SCHEMA_DIGEST_CHAIN_PREFIX}"
        ));
    }
    if let (
        Some(artifact_kind),
        Some(source_digest),
        Some(command_digest),
        Some(config_digest),
        Some(hardware_digest),
        Some(dataset_digest),
        Some(comparator_version),
    ) = (
        chain.get("artifact_kind").and_then(Value::as_str),
        chain.get("source_digest").and_then(Value::as_str),
        chain.get("command_digest").and_then(Value::as_str),
        chain.get("config_digest").and_then(Value::as_str),
        chain.get("hardware_digest").and_then(Value::as_str),
        chain.get("dataset_digest").and_then(Value::as_str),
        chain.get("comparator_version").and_then(Value::as_str),
    ) {
        let recomputed = benchmark_schema_digest_chain_value(
            artifact_kind,
            artifact_schema_version as u32,
            source_digest,
            command_digest,
            config_digest,
            hardware_digest,
            dataset_digest,
            comparator_version,
        );
        if recomputed
            .get("digest")
            .and_then(Value::as_str)
            .is_some_and(|expected| expected != digest)
        {
            issues.push(format!(
                "{evidence}: schema_digest_chain.digest does not match chained benchmark provenance fields"
            ));
        }
    }
    issues
}

fn benchmark_schema_digest_chain_material(
    artifact_kind: &str,
    artifact_schema_version: u32,
    source_digest: &str,
    command_digest: &str,
    config_digest: &str,
    hardware_digest: &str,
    dataset_digest: &str,
    comparator_version: &str,
) -> String {
    format!(
        "benchmark-schema-digest-chain:v1\nartifact_kind={artifact_kind}\nartifact_schema_version={artifact_schema_version}\nsource_digest={source_digest}\ncommand_digest={command_digest}\nconfig_digest={config_digest}\nhardware_digest={hardware_digest}\ndataset_digest={dataset_digest}\ncomparator_version={comparator_version}\n"
    )
}

pub(crate) fn benchmark_fused_execution_dag_issues(
    artifact_label: &str,
    report: &Value,
) -> Vec<String> {
    let mut issues = Vec::new();
    let Some(dag) = report.get("fused_execution_dag") else {
        return vec![format!(
            "{artifact_label}: missing fused_execution_dag evidence"
        )];
    };
    if dag.get("schema_version").and_then(Value::as_u64) != Some(1) {
        issues.push(format!(
            "{artifact_label}: fused_execution_dag.schema_version must be 1"
        ));
    }
    let graph_nodes = dag_node_ids(dag.get("graph_nodes"));
    if graph_nodes.len() < 5 {
        issues.push(format!(
            "{artifact_label}: fused_execution_dag.graph_nodes needs ingest, scan, verify, confidence, and report nodes"
        ));
    }
    for required in ["ingest", "scan", "verify", "confidence", "report"] {
        if !graph_nodes.iter().any(|node| node == required) {
            issues.push(format!(
                "{artifact_label}: fused_execution_dag.graph_nodes is missing `{required}`"
            ));
        }
    }
    let Some(memory_edges) = dag.get("memory_edges").and_then(Value::as_array) else {
        issues.push(format!(
            "{artifact_label}: fused_execution_dag.memory_edges must be a non-empty array"
        ));
        return issues;
    };
    if memory_edges.is_empty() {
        issues.push(format!(
            "{artifact_label}: fused_execution_dag.memory_edges must be non-empty"
        ));
    }
    for (index, edge) in memory_edges.iter().enumerate() {
        for field in ["from", "to", "buffer"] {
            if edge
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                issues.push(format!(
                    "{artifact_label}: fused_execution_dag.memory_edges[{index}].{field} is blank or missing"
                ));
            }
        }
        if edge.get("bytes").and_then(Value::as_u64).unwrap_or(0) == 0 {
            issues.push(format!(
                "{artifact_label}: fused_execution_dag.memory_edges[{index}].bytes must be positive"
            ));
        }
    }
    match dag.get("host_sync_points").and_then(Value::as_u64) {
        Some(points) if points <= 1 => {}
        Some(points) => issues.push(format!(
            "{artifact_label}: fused_execution_dag.host_sync_points={points}, expected final-state-only sync <=1"
        )),
        None => issues.push(format!(
            "{artifact_label}: fused_execution_dag.host_sync_points is missing"
        )),
    }
    let bytes = dag.get("bytes_transferred");
    for field in ["host_to_device_bytes", "device_to_host_bytes"] {
        if bytes
            .and_then(|bytes| bytes.get(field))
            .and_then(Value::as_u64)
            .is_none()
        {
            issues.push(format!(
                "{artifact_label}: fused_execution_dag.bytes_transferred.{field} is missing"
            ));
        }
    }
    let parity = dag.get("reporter_parity");
    if parity
        .and_then(|parity| parity.get("parity_passed"))
        .and_then(Value::as_bool)
        != Some(true)
    {
        issues.push(format!(
            "{artifact_label}: fused_execution_dag.reporter_parity.parity_passed must be true"
        ));
    }
    for field in ["cpu_digest", "gpu_digest", "output_digest"] {
        if parity
            .and_then(|parity| parity.get(field))
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            issues.push(format!(
                "{artifact_label}: fused_execution_dag.reporter_parity.{field} is blank or missing"
            ));
        }
    }
    let Some(fallback_reasons) = dag.get("fallback_reasons").and_then(Value::as_array) else {
        issues.push(format!(
            "{artifact_label}: fused_execution_dag.fallback_reasons must be an array"
        ));
        return issues;
    };
    for (index, reason) in fallback_reasons.iter().enumerate() {
        if reason
            .get("reason")
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            issues.push(format!(
                "{artifact_label}: fused_execution_dag.fallback_reasons[{index}].reason is blank or missing"
            ));
        }
        if reason
            .get("fix")
            .and_then(Value::as_str)
            .is_none_or(|fix| !fix.starts_with("Fix:"))
        {
            issues.push(format!(
                "{artifact_label}: fused_execution_dag.fallback_reasons[{index}].fix must start with `Fix:`"
            ));
        }
    }
    for field in [
        "comparator",
        "dataset_id",
        "metric_family",
        "release_floor",
        "failure_mode",
    ] {
        if dag
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            issues.push(format!(
                "{artifact_label}: fused_execution_dag.{field} is blank or missing"
            ));
        }
    }
    if dag
        .get("frontier_leaderboard_artifact")
        .and_then(Value::as_str)
        != Some("release/evidence/benchmarks/frontier-leaderboard.json")
    {
        issues.push(format!(
            "{artifact_label}: fused_execution_dag.frontier_leaderboard_artifact must point at release/evidence/benchmarks/frontier-leaderboard.json"
        ));
    }
    issues
}

fn dag_node_ids(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |nodes| {
            nodes
                .iter()
                .filter_map(|node| {
                    node.as_str()
                        .or_else(|| node.get("id").and_then(Value::as_str))
                })
                .filter(|node| !node.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
}

pub(crate) fn benchmark_case_failure_reason(case: &Value) -> Option<String> {
    let status = case.get("status").and_then(Value::as_str);
    let contract_failed = case
        .get("performance")
        .and_then(|performance| performance.get("contract_passed"))
        .and_then(Value::as_bool)
        == Some(false);
    let invalid_reason = case
        .get("correctness")
        .and_then(|correctness| correctness.get("Invalid"))
        .map(|invalid| {
            invalid
                .get("reason")
                .and_then(non_empty_str)
                .map(str::to_string)
                .unwrap_or_else(|| "invalid correctness".to_string())
        });
    let violation_reason = case
        .get("performance")
        .and_then(|performance| performance.get("violations"))
        .and_then(Value::as_array)
        .map(|violations| {
            violations
                .iter()
                .filter_map(non_empty_str)
                .collect::<Vec<_>>()
        })
        .and_then(|violations| (!violations.is_empty()).then(|| violations.join("; ")));
    invalid_reason
        .or(violation_reason)
        .or_else(|| match status {
            Some("pass") => None,
            Some(status) if !status.is_empty() => Some(format!("status `{status}`")),
            _ => Some("missing pass status".to_string()),
        })
        .or_else(|| contract_failed.then(|| "performance contract failed".to_string()))
}

pub(crate) fn benchmark_case_passes_summary_evidence(case: &Value) -> bool {
    case.get("status").and_then(Value::as_str) == Some("pass")
        && benchmark_case_failure_reason(case).is_none()
}

pub(crate) fn benchmark_report_summary_matches_case_evidence(report: &Value) -> bool {
    benchmark_report_summary_case_evidence_mismatch(report).is_none()
}

pub(crate) fn benchmark_report_has_source_provenance(report: &Value) -> bool {
    report
        .get("source_fingerprint")
        .and_then(non_empty_str)
        .is_some()
}

pub(crate) fn benchmark_source_artifact_count(report: &Value) -> usize {
    benchmark_source_artifact_paths(report).len()
}

pub(crate) fn benchmark_source_artifact_entry_count(report: &Value) -> usize {
    report
        .get("source_artifacts")
        .and_then(Value::as_array)
        .map_or(0, |items| items.iter().filter_map(non_empty_str).count())
}

pub(crate) fn benchmark_source_artifact_paths(report: &Value) -> BTreeSet<String> {
    report
        .get("source_artifacts")
        .and_then(Value::as_array)
        .map_or_else(BTreeSet::new, |items| {
            items
                .iter()
                .filter_map(non_empty_str)
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
        })
}

pub(crate) fn benchmark_duplicate_source_artifact_paths(report: &Value) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    report
        .get("source_artifacts")
        .and_then(Value::as_array)
        .map_or_else(BTreeSet::new, |items| {
            items
                .iter()
                .filter_map(non_empty_str)
                .filter_map(|artifact| {
                    if seen.insert(artifact.to_string()) {
                        None
                    } else {
                        Some(artifact.to_string())
                    }
                })
                .collect::<BTreeSet<_>>()
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BenchmarkArtifactPathIssue {
    AbsolutePath,
    NonReleasePath,
    ParentTraversal,
    Missing {
        artifact_path: PathBuf,
    },
    OutsideWorkspace {
        artifact_path: PathBuf,
        workspace_root: PathBuf,
    },
}

impl BenchmarkArtifactPathIssue {
    pub(crate) fn describe(&self, label: &str, artifact: &str) -> String {
        match self {
            Self::AbsolutePath => {
                format!("{label} `{artifact}` must be a relative release path")
            }
            Self::NonReleasePath => {
                format!("{label} `{artifact}` must start with `release/`")
            }
            Self::ParentTraversal => {
                format!("{label} `{artifact}` must not contain parent directory traversal")
            }
            Self::Missing { artifact_path } => format!(
                "{label} `{artifact}` is not a readable file at {}",
                artifact_path.display()
            ),
            Self::OutsideWorkspace {
                artifact_path,
                workspace_root,
            } => format!(
                "{label} `{artifact}` resolves outside workspace: {} is outside {}",
                artifact_path.display(),
                workspace_root.display()
            ),
        }
    }
}

pub(crate) fn benchmark_source_artifact_path_issue(
    workspace_root: &Path,
    artifact: &str,
) -> Option<BenchmarkArtifactPathIssue> {
    benchmark_release_artifact_path_issue(workspace_root, artifact)
}

pub(crate) fn benchmark_suite_artifact_path_issue(
    workspace_root: &Path,
    artifact: &str,
) -> Option<BenchmarkArtifactPathIssue> {
    benchmark_release_artifact_path_issue(workspace_root, artifact)
}

fn benchmark_release_artifact_path_issue(
    workspace_root: &Path,
    artifact: &str,
) -> Option<BenchmarkArtifactPathIssue> {
    let candidate = PathBuf::from(artifact);
    if candidate.is_absolute() {
        return Some(BenchmarkArtifactPathIssue::AbsolutePath);
    }
    if !artifact.starts_with("release/") {
        return Some(BenchmarkArtifactPathIssue::NonReleasePath);
    }
    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Some(BenchmarkArtifactPathIssue::ParentTraversal);
    }
    let artifact_path = workspace_root.join(&candidate);
    if !artifact_path.is_file() {
        return Some(BenchmarkArtifactPathIssue::Missing { artifact_path });
    }
    let Ok(canonical_root) = workspace_root.canonicalize() else {
        return Some(BenchmarkArtifactPathIssue::Missing { artifact_path });
    };
    let Ok(canonical_artifact) = artifact_path.canonicalize() else {
        return Some(BenchmarkArtifactPathIssue::Missing { artifact_path });
    };
    if !canonical_artifact.starts_with(&canonical_root) {
        return Some(BenchmarkArtifactPathIssue::OutsideWorkspace {
            artifact_path: canonical_artifact,
            workspace_root: canonical_root,
        });
    }
    None
}

pub(crate) fn duplicate_nonblank_string_array_values(
    value: &Value,
    field: &str,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    value
        .get(field)
        .and_then(Value::as_array)
        .map_or_else(BTreeSet::new, |items| {
            items
                .iter()
                .filter_map(non_empty_str)
                .filter_map(|item| {
                    if seen.insert(item.to_string()) {
                        None
                    } else {
                        Some(item.to_string())
                    }
                })
                .collect::<BTreeSet<_>>()
        })
}

pub(crate) fn duplicate_nonblank_object_array_field_values(
    value: &Value,
    array_field: &str,
    object_field: &str,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    value
        .get(array_field)
        .and_then(Value::as_array)
        .map_or_else(BTreeSet::new, |items| {
            items
                .iter()
                .filter_map(|item| item.get(object_field).and_then(non_empty_str))
                .filter_map(|item| {
                    if seen.insert(item.to_string()) {
                        None
                    } else {
                        Some(item.to_string())
                    }
                })
                .collect::<BTreeSet<_>>()
        })
}

pub(crate) fn cuda_release_axes_source_artifact_issues(
    workspace_root: &Path,
    axes: &Value,
    cuda_suite: &Value,
) -> Vec<String> {
    let mut issues = Vec::new();
    if let Some(issue) = backend_suite_backend_issue(cuda_suite, "cuda") {
        match issue {
            BackendSuiteBackendIssue::Missing { expected_backend } => issues.push(format!(
                "cuda-release-suite is missing backend identity `{expected_backend}`"
            )),
            BackendSuiteBackendIssue::Mismatch {
                expected_backend,
                actual_backend,
            } => issues.push(format!(
                "cuda-release-suite backend `{actual_backend}` does not match required `{expected_backend}`"
            )),
        }
    }
    let source_artifacts = release_axes_source_artifacts(axes, &mut issues);
    if source_artifacts.len() < 12 {
        issues.push(format!(
            "source_artifacts has {} CUDA workload artifact(s), needs at least 12",
            source_artifacts.len()
        ));
    }

    let suite_artifacts = cuda_suite_artifact_paths(cuda_suite, &mut issues);
    if suite_artifacts.is_empty() {
        issues.push("cuda-release-suite artifacts are empty or missing".to_string());
    }
    for issue in backend_suite_inventory_issues(cuda_suite) {
        issues.push(format!(
            "cuda-release-suite {}",
            describe_backend_suite_inventory_issue(&issue)
        ));
    }
    for artifact in source_artifacts.difference(&suite_artifacts) {
        issues.push(format!(
            "source_artifact `{artifact}` is not listed in cuda-release-suite artifacts"
        ));
    }
    for artifact in suite_artifacts.difference(&source_artifacts) {
        issues.push(format!(
            "cuda-release-suite artifact `{artifact}` is absent from bench-release-axes source_artifacts"
        ));
    }

    let mut source_reports = Vec::new();
    for artifact in source_artifacts {
        if let Some(issue) = benchmark_source_artifact_path_issue(workspace_root, &artifact) {
            issues.push(issue.describe("source_artifact", &artifact));
            continue;
        }
        let artifact_path = resolve_benchmark_artifact_path(workspace_root, &artifact);
        let text =
            match read_text_bounded(&artifact_path, MAX_BENCHMARK_EVIDENCE_SEMANTIC_TEXT_BYTES) {
                Ok(text) => text,
                Err(error) => {
                    issues.push(format!(
                        "source_artifact `{artifact}` is unreadable: {error}"
                    ));
                    continue;
                }
            };
        let report = match serde_json::from_str::<Value>(&text) {
            Ok(report) => report,
            Err(error) => {
                issues.push(format!(
                    "source_artifact `{artifact}` is invalid JSON: {error}"
                ));
                continue;
            }
        };
        inspect_release_axis_source_artifact_provenance(
            &artifact,
            &artifact_path,
            &report,
            &mut issues,
        );
        if report.get("selected_backend").and_then(Value::as_str) != Some("cuda") {
            issues.push(format!(
                "source_artifact `{artifact}` selected_backend must be cuda"
            ));
        }
        inspect_source_artifact_case_integrity(
            &artifact,
            &report,
            "canonical CUDA release axes",
            &mut issues,
        );
        inspect_release_axis_source_artifact_metrics(&artifact, &report, &mut issues);
        source_reports.push(report);
    }
    inspect_release_axes_scalar_values(axes, &source_reports, &mut issues);
    issues
}

pub(crate) fn cpu_sota_100x_source_artifact_issues(
    workspace_root: &Path,
    proof: &Value,
) -> Vec<String> {
    let mut issues = Vec::new();
    let aggregate_source_tree_fingerprint =
        proof.get("source_tree_fingerprint").and_then(non_empty_str);
    for artifact in benchmark_source_artifact_paths(proof) {
        if let Some(issue) = benchmark_source_artifact_path_issue(workspace_root, &artifact) {
            issues.push(issue.describe("source_artifact", &artifact));
            continue;
        }
        let artifact_path = resolve_benchmark_artifact_path(workspace_root, &artifact);
        let text =
            match read_text_bounded(&artifact_path, MAX_BENCHMARK_EVIDENCE_SEMANTIC_TEXT_BYTES) {
                Ok(text) => text,
                Err(error) => {
                    issues.push(format!(
                        "source_artifact `{artifact}` is unreadable: {error}"
                    ));
                    continue;
                }
            };
        let report = match serde_json::from_str::<Value>(&text) {
            Ok(report) => report,
            Err(error) => {
                issues.push(format!(
                    "source_artifact `{artifact}` is invalid JSON: {error}"
                ));
                continue;
            }
        };
        if report.get("selected_backend").and_then(Value::as_str) != Some("cuda") {
            issues.push(format!(
                "source_artifact `{artifact}` was not produced for cuda"
            ));
        }
        inspect_source_artifact_case_integrity(
            &artifact,
            &report,
            "CPU-SOTA aggregate proof",
            &mut issues,
        );
        let report_source_fingerprint = report.get("source_fingerprint").and_then(non_empty_str);
        if let Some(fingerprint) = report_source_fingerprint {
            for issue in source_fingerprint_issues(fingerprint) {
                match issue {
                    SourceFingerprintIssue::DirtyUnknownState { source_fingerprint } => {
                        issues.push(format!(
                            "source_artifact `{artifact}` source_fingerprint `{source_fingerprint}` has unknown dirty state"
                        ));
                    }
                    SourceFingerprintIssue::DirtyMissingWorktree { source_fingerprint } => {
                        issues.push(format!(
                            "source_artifact `{artifact}` source_fingerprint `{source_fingerprint}` is dirty but has no worktree digest"
                        ));
                    }
                    SourceFingerprintIssue::DirtyUnknownWorktree { source_fingerprint } => {
                        issues.push(format!(
                            "source_artifact `{artifact}` source_fingerprint `{source_fingerprint}` is dirty but has unknown worktree digest"
                        ));
                    }
                    SourceFingerprintIssue::DirtyInvalidWorktree {
                        source_fingerprint,
                        worktree,
                    } => {
                        issues.push(format!(
                            "source_artifact `{artifact}` source_fingerprint `{source_fingerprint}` has invalid worktree digest `{worktree}`"
                        ));
                    }
                }
            }
        } else {
            issues.push(format!(
                "source_artifact `{artifact}` has no source_fingerprint"
            ));
        }
        let report_source_tree_fingerprint = report
            .get("source_tree_fingerprint")
            .and_then(non_empty_str);
        match (
            aggregate_source_tree_fingerprint,
            report_source_tree_fingerprint,
        ) {
            (_, None) => issues.push(format!(
                "source_artifact `{artifact}` has no source_tree_fingerprint"
            )),
            (Some(aggregate), Some(fingerprint)) if fingerprint != aggregate => {
                issues.push(format!(
                    "source_artifact `{artifact}` source_tree_fingerprint `{fingerprint}` does not match aggregate source tree `{aggregate}`"
                ));
            }
            _ => {}
        }
        if let (Some((field, source_fingerprint)), Some(current_source_fingerprint)) = (
            report_freshness_fingerprint(&report),
            current_freshness_fingerprint_for_report(&artifact_path, &report),
        ) {
            for issue in
                source_fingerprint_freshness_issues(source_fingerprint, &current_source_fingerprint)
            {
                match issue {
                    SourceFingerprintFreshnessIssue::Mismatch {
                        source_fingerprint,
                        current_source_fingerprint,
                    } => issues.push(format!(
                        "source_artifact `{artifact}` {field} `{source_fingerprint}` does not match current workspace source `{current_source_fingerprint}`"
                    )),
                }
            }
        }
    }
    issues
}

fn inspect_release_axes_scalar_values(
    axes: &Value,
    source_reports: &[Value],
    issues: &mut Vec<String>,
) {
    if source_reports.is_empty() {
        return;
    }
    if let Some(expected) = min_positive_metric_percentile(source_reports, "wall_ns", "p50") {
        inspect_release_axis_f64(axes, "warm_us_per_file", expected as f64 / 1_000.0, issues);
    }
    if let Some(expected) = first_min_positive_metric_percentile(
        source_reports,
        &[
            "cold_compile_ns",
            "cold_wall_ns",
            "compile_ns",
            "lower_ns",
            "optimize_ns",
        ],
        "p50",
    ) {
        inspect_release_axis_f64(
            axes,
            "cold_pipeline_build_ms",
            expected as f64 / 1_000_000.0,
            issues,
        );
    }
    if let Some(expected) = first_max_positive_metric_percentile(
        source_reports,
        &["wall_gb_s_x1000", "device_gb_s_x1000"],
        "p50",
    ) {
        inspect_release_axis_f64(
            axes,
            "gbs_scan_throughput",
            expected as f64 / 1_000.0,
            issues,
        );
    }
    inspect_release_axis_u64(
        axes,
        "ulp_drift_max",
        max_observed_ulp(source_reports),
        issues,
    );
    if let Some(expected) = max_release_axis_vram_mib(source_reports) {
        inspect_release_axis_u64(axes, "max_vram_mib", expected, issues);
    }
}

fn inspect_release_axis_f64(axes: &Value, axis: &str, expected: f64, issues: &mut Vec<String>) {
    let Some(actual) = axes_number_f64(axes, axis) else {
        issues.push(format!(
            "bench-release-axes {axis} is missing or not numeric; expected {expected}"
        ));
        return;
    };
    if (actual - expected).abs() > 0.000_001 {
        issues.push(format!(
            "bench-release-axes {axis}={actual} does not match source artifacts {expected}"
        ));
    }
}

fn inspect_release_axis_u64(axes: &Value, axis: &str, expected: u64, issues: &mut Vec<String>) {
    let Some(actual) = axes_number_u64(axes, axis) else {
        issues.push(format!(
            "bench-release-axes {axis} is missing or not numeric; expected {expected}"
        ));
        return;
    };
    if actual != expected {
        issues.push(format!(
            "bench-release-axes {axis}={actual} does not match source artifacts {expected}"
        ));
    }
}

fn axes_number_f64(axes: &Value, axis: &str) -> Option<f64> {
    axes.get(axis).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str()?.parse::<f64>().ok())
    })
}

fn axes_number_u64(axes: &Value, axis: &str) -> Option<u64> {
    axes.get(axis).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str()?.parse::<u64>().ok())
    })
}

fn min_positive_metric_percentile(
    reports: &[Value],
    metric_name: &str,
    percentile: &str,
) -> Option<u64> {
    reports
        .iter()
        .filter_map(|report| artifact_positive_metric_percentile(report, metric_name, percentile))
        .min()
}

fn max_positive_metric_percentile(
    reports: &[Value],
    metric_name: &str,
    percentile: &str,
) -> Option<u64> {
    reports
        .iter()
        .filter_map(|report| artifact_positive_metric_percentile(report, metric_name, percentile))
        .max()
}

fn first_min_positive_metric_percentile(
    reports: &[Value],
    metric_names: &[&str],
    percentile: &str,
) -> Option<u64> {
    metric_names
        .iter()
        .find_map(|metric_name| min_positive_metric_percentile(reports, metric_name, percentile))
}

fn first_max_positive_metric_percentile(
    reports: &[Value],
    metric_names: &[&str],
    percentile: &str,
) -> Option<u64> {
    metric_names
        .iter()
        .find_map(|metric_name| max_positive_metric_percentile(reports, metric_name, percentile))
}

fn max_observed_ulp(reports: &[Value]) -> u64 {
    reports
        .iter()
        .filter_map(|report| report.get("cases").and_then(Value::as_array))
        .flat_map(|cases| cases.iter())
        .filter_map(|case| {
            case.get("correctness")
                .and_then(|correctness| correctness.get("Toleranced"))
                .and_then(|toleranced| toleranced.get("max_observed_ulp"))
                .and_then(Value::as_u64)
        })
        .max()
        .unwrap_or(0)
}

fn max_release_axis_vram_mib(reports: &[Value]) -> Option<u64> {
    let environment_values = reports
        .iter()
        .filter_map(|report| artifact_environment_first_gpu_u64(report, "memory_total_mib"))
        .filter(|value| *value > 0);
    let metric_values = reports.iter().filter_map(|report| {
        artifact_positive_metric_percentile(report, "memory_total_mib", "p50")
    });
    environment_values.chain(metric_values).max()
}

pub(crate) fn inspect_source_artifact_case_integrity(
    artifact: &str,
    report: &Value,
    native_dispatch_context: &str,
    issues: &mut Vec<String>,
) {
    for issue in backend_consistency_issues(report) {
        match issue {
            BackendConsistencyIssue::MissingCaseId { case_index } => issues.push(format!(
                "source_artifact `{artifact}` case index {case_index} must include a nonblank id"
            )),
            BackendConsistencyIssue::DuplicateCaseId { case_id, count } => issues.push(format!(
                "source_artifact `{artifact}` has {count} cases with id `{case_id}`"
            )),
            BackendConsistencyIssue::MissingCaseBackend {
                case_id,
                expected_backend,
            } => issues.push(format!(
                "source_artifact `{artifact}` case `{case_id}` must include backend_id `{expected_backend}` matching selected_backend"
            )),
            BackendConsistencyIssue::CaseBackendMismatch {
                case_id,
                expected_backend,
                actual_backend,
            } => issues.push(format!(
                "source_artifact `{artifact}` case `{case_id}` backend_id `{actual_backend}` does not match selected_backend `{expected_backend}`"
            )),
        }
    }
    for issue in contract_backend_issues(report) {
        match issue {
            ContractBackendIssue::MissingBaselines {
                case_id,
                backend_id,
            } => issues.push(format!(
                "source_artifact `{artifact}` case `{case_id}` backend `{backend_id}` has a performance contract with no baselines"
            )),
            ContractBackendIssue::NoApplicableBaseline {
                case_id,
                backend_id,
            } => issues.push(format!(
                "source_artifact `{artifact}` case `{case_id}` backend `{backend_id}` has no applicable performance contract baseline"
            )),
        }
    }
    for issue in cuda_forbidden_telemetry_issues(report) {
        match issue {
            CudaForbiddenTelemetryIssue::ResidentBorrowedEscapeHatch {
                case_id,
                observed_p50,
            } => issues.push(format!(
                "source_artifact `{artifact}` case `{case_id}` has cuda_resident_borrowed_fallback_dispatches p50={observed_p50}; {native_dispatch_context} must use native resident dispatch"
            )),
        }
    }
    for issue in cuda_telemetry_label_issues(report) {
        match issue {
            CudaTelemetryLabelIssue::MissingLabel { case_id, label } => issues.push(format!(
                "source_artifact `{artifact}` case `{case_id}` has positive CUDA telemetry counters but is missing `{label}`"
            )),
            CudaTelemetryLabelIssue::LabelWithoutCounters { case_id, label } => issues.push(format!(
                "source_artifact `{artifact}` case `{case_id}` lists `{label}` but all matching CUDA telemetry counters are zero or missing"
            )),
        }
    }
    if report
        .get("cases")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        issues.push(format!(
            "source_artifact `{artifact}` has no benchmark cases"
        ));
    }
    if let Some(mismatch) = benchmark_report_summary_case_evidence_mismatch(report) {
        issues.push(format!(
            "source_artifact `{artifact}` summary does not match case evidence: {mismatch}"
        ));
    }
    if report
        .get("summary")
        .and_then(|summary| summary.get("failed"))
        .and_then(Value::as_u64)
        != Some(0)
    {
        issues.push(format!(
            "source_artifact `{artifact}` summary.failed must be 0"
        ));
    }
}

fn inspect_release_axis_source_artifact_metrics(
    artifact: &str,
    report: &Value,
    issues: &mut Vec<String>,
) {
    if artifact_positive_metric_percentile(report, "wall_ns", "p50").is_none() {
        issues.push(format!(
            "source_artifact `{artifact}` has no positive p50 wall_ns metric for warm_us_per_file"
        ));
    }
    if first_positive_artifact_metric_percentile(
        report,
        &[
            "cold_compile_ns",
            "cold_wall_ns",
            "compile_ns",
            "lower_ns",
            "optimize_ns",
        ],
        "p50",
    )
    .is_none()
    {
        issues.push(format!(
            "source_artifact `{artifact}` has no positive p50 cold/compile metric for cold_pipeline_build_ms"
        ));
    }
    if first_positive_artifact_metric_percentile(
        report,
        &["wall_gb_s_x1000", "device_gb_s_x1000"],
        "p50",
    )
    .is_none()
    {
        issues.push(format!(
            "source_artifact `{artifact}` has no positive p50 throughput metric for gbs_scan_throughput"
        ));
    }
    if artifact_environment_first_gpu_u64(report, "memory_total_mib")
        .filter(|value| *value > 0)
        .or_else(|| artifact_positive_metric_percentile(report, "memory_total_mib", "p50"))
        .is_none()
    {
        issues.push(format!(
            "source_artifact `{artifact}` has no GPU memory_total_mib evidence for max_vram_mib"
        ));
    }
}

fn first_positive_artifact_metric_percentile(
    report: &Value,
    metric_names: &[&str],
    percentile: &str,
) -> Option<u64> {
    metric_names.iter().find_map(|metric_name| {
        artifact_positive_metric_percentile(report, metric_name, percentile)
    })
}

fn artifact_positive_metric_percentile(
    report: &Value,
    metric_name: &str,
    percentile: &str,
) -> Option<u64> {
    artifact_min_metric_percentile(report, metric_name, percentile).filter(|value| *value > 0)
}

fn inspect_release_axis_source_artifact_provenance(
    artifact: &str,
    artifact_path: &Path,
    report: &Value,
    issues: &mut Vec<String>,
) {
    let source_fingerprint = report.get("source_fingerprint").and_then(non_empty_str);
    let source_tree_fingerprint = report
        .get("source_tree_fingerprint")
        .and_then(non_empty_str);
    let Some(source_fingerprint) = source_fingerprint else {
        issues.push(format!(
            "source_artifact `{artifact}` has no source_fingerprint"
        ));
        return inspect_release_axis_source_artifact_freshness(
            artifact,
            artifact_path,
            report,
            issues,
        );
    };
    for issue in source_fingerprint_issues(source_fingerprint) {
        match issue {
            SourceFingerprintIssue::DirtyUnknownState { source_fingerprint } => {
                issues.push(format!(
                    "source_artifact `{artifact}` source_fingerprint `{source_fingerprint}` has unknown dirty state"
                ));
            }
            SourceFingerprintIssue::DirtyMissingWorktree { source_fingerprint } => {
                issues.push(format!(
                    "source_artifact `{artifact}` source_fingerprint `{source_fingerprint}` is dirty but has no worktree digest"
                ));
            }
            SourceFingerprintIssue::DirtyUnknownWorktree { source_fingerprint } => {
                issues.push(format!(
                    "source_artifact `{artifact}` source_fingerprint `{source_fingerprint}` is dirty but has unknown worktree digest"
                ));
            }
            SourceFingerprintIssue::DirtyInvalidWorktree {
                source_fingerprint,
                worktree,
            } => {
                issues.push(format!(
                    "source_artifact `{artifact}` source_fingerprint `{source_fingerprint}` has invalid worktree digest `{worktree}`"
                ));
            }
        }
    }
    if source_tree_fingerprint.is_none() {
        issues.push(format!(
            "source_artifact `{artifact}` has no source_tree_fingerprint"
        ));
    }
    inspect_release_axis_source_artifact_freshness(artifact, artifact_path, report, issues);
}

fn inspect_release_axis_source_artifact_freshness(
    artifact: &str,
    artifact_path: &Path,
    report: &Value,
    issues: &mut Vec<String>,
) {
    let Some((field, source_fingerprint)) = report_freshness_fingerprint(report) else {
        return;
    };
    let Some(current_source_fingerprint) =
        current_freshness_fingerprint_for_report(artifact_path, report)
    else {
        issues.push(format!(
            "source_artifact `{artifact}` current workspace source fingerprint could not be resolved"
        ));
        return;
    };
    for issue in
        source_fingerprint_freshness_issues(source_fingerprint, &current_source_fingerprint)
    {
        match issue {
            SourceFingerprintFreshnessIssue::Mismatch {
                source_fingerprint,
                current_source_fingerprint,
            } => issues.push(format!(
                "source_artifact `{artifact}` {field} `{source_fingerprint}` does not match current workspace source `{current_source_fingerprint}`"
            )),
        }
    }
}

fn release_axes_source_artifacts(axes: &Value, issues: &mut Vec<String>) -> BTreeSet<String> {
    let Some(items) = axes.get("source_artifacts").and_then(Value::as_array) else {
        issues.push("source_artifacts array is missing".to_string());
        return BTreeSet::new();
    };
    collect_nonblank_string_set("source_artifacts", items, issues)
}

fn cuda_suite_artifact_paths(cuda_suite: &Value, issues: &mut Vec<String>) -> BTreeSet<String> {
    let Some(items) = cuda_suite.get("artifacts").and_then(Value::as_array) else {
        issues.push("cuda-release-suite artifacts array is missing".to_string());
        return BTreeSet::new();
    };
    collect_nonblank_string_set("cuda-release-suite artifacts", items, issues)
}

fn collect_nonblank_string_set(
    field: &str,
    items: &[Value],
    issues: &mut Vec<String>,
) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for (index, item) in items.iter().enumerate() {
        let Some(path) = item.as_str().filter(|path| !path.trim().is_empty()) else {
            issues.push(format!("{field}[{index}] is not a nonblank string"));
            continue;
        };
        if !paths.insert(path.to_string()) {
            issues.push(format!("{field} contains duplicate artifact `{path}`"));
        }
    }
    paths
}

fn resolve_benchmark_artifact_path(workspace_root: &Path, artifact: &str) -> PathBuf {
    let candidate = PathBuf::from(artifact);
    if candidate.is_absolute() {
        candidate
    } else {
        workspace_root.join(candidate)
    }
}

fn read_text_bounded(path: &Path, max_bytes: u64) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(max_bytes.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {max_bytes} byte evidence read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}

pub(crate) fn benchmark_report_summary_case_evidence_mismatch(report: &Value) -> Option<String> {
    let Some(cases) = report.get("cases").and_then(Value::as_array) else {
        return Some("missing cases array".to_string());
    };
    let Some(summary) = report.get("summary") else {
        return Some("missing summary".to_string());
    };
    let passed = cases
        .iter()
        .filter(|case| benchmark_case_passes_summary_evidence(case))
        .count() as u64;
    let failed = cases.len() as u64 - passed;
    let summary_total_cases = summary.get("total_cases").and_then(Value::as_u64);
    let summary_passed = summary.get("passed").and_then(Value::as_u64);
    let summary_failed = summary.get("failed").and_then(Value::as_u64);
    if summary_total_cases == Some(cases.len() as u64)
        && summary_passed == Some(passed)
        && summary_failed == Some(failed)
    {
        return None;
    }
    Some(format!(
        "summary total/pass/fail ({summary_total_cases:?}/{summary_passed:?}/{summary_failed:?}) contradicts case evidence ({}/{passed}/{failed})",
        cases.len()
    ))
}

pub(crate) fn benchmark_failed_case_summaries(report: &Value) -> Vec<String> {
    report
        .get("cases")
        .and_then(Value::as_array)
        .map(|cases| {
            cases
                .iter()
                .filter_map(|case| {
                    let id = case
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("<unknown>");
                    benchmark_case_failure_reason(case).map(|reason| format!("`{id}`: {reason}"))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LaunchPlanLabelIssue {
    MissingSingle,
    SingleHasMulti,
    MissingMulti { launch_count: f64 },
    MultiHasSingle { launch_count: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BackendConsistencyIssue {
    MissingCaseId {
        case_index: usize,
    },
    DuplicateCaseId {
        case_id: String,
        count: usize,
    },
    MissingCaseBackend {
        case_id: String,
        expected_backend: String,
    },
    CaseBackendMismatch {
        case_id: String,
        expected_backend: String,
        actual_backend: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContractBackendIssue {
    MissingBaselines { case_id: String, backend_id: String },
    NoApplicableBaseline { case_id: String, backend_id: String },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CudaTelemetryLabelIssue {
    MissingLabel {
        case_id: String,
        label: &'static str,
    },
    LabelWithoutCounters {
        case_id: String,
        label: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CudaForbiddenTelemetryIssue {
    ResidentBorrowedEscapeHatch { case_id: String, observed_p50: f64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceFingerprintIssue {
    DirtyUnknownState {
        source_fingerprint: String,
    },
    DirtyMissingWorktree {
        source_fingerprint: String,
    },
    DirtyUnknownWorktree {
        source_fingerprint: String,
    },
    DirtyInvalidWorktree {
        source_fingerprint: String,
        worktree: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceFingerprintFreshnessIssue {
    Mismatch {
        source_fingerprint: String,
        current_source_fingerprint: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BackendSuiteParityIssue {
    CudaBackendIdentity {
        issue: BackendSuiteBackendIssue,
    },
    WgpuBackendIdentity {
        issue: BackendSuiteBackendIssue,
    },
    MissingCudaPair {
        family_id: String,
        requested_case_id: String,
    },
    MissingWgpuPair {
        family_id: String,
        requested_case_id: String,
    },
    CountMismatch {
        cuda_count: usize,
        wgpu_count: usize,
    },
    SharedArtifactPath {
        path: String,
    },
    DuplicateCudaPair {
        family_id: String,
        requested_case_id: String,
        count: usize,
    },
    DuplicateWgpuPair {
        family_id: String,
        requested_case_id: String,
        count: usize,
    },
    StatusFieldMismatch {
        family_id: String,
        requested_case_id: String,
        field: &'static str,
        cuda_value: Option<u64>,
        wgpu_value: Option<u64>,
    },
    StatusStringFieldMismatch {
        family_id: String,
        requested_case_id: String,
        field: &'static str,
        cuda_value: Option<String>,
        wgpu_value: Option<String>,
    },
    StatusBlockersMismatch {
        family_id: String,
        requested_case_id: String,
        cuda_blockers: Option<Vec<String>>,
        wgpu_blockers: Option<Vec<String>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BackendSuiteInventoryIssue {
    CountMismatch {
        artifact_count: usize,
        status_count: usize,
    },
    DeclaredFamilyArtifactCountMismatch {
        family_count: u64,
        artifact_count: usize,
    },
    DeclaredFamilyStatusCountMismatch {
        family_count: u64,
        status_family_count: usize,
    },
    MissingStatus {
        path: String,
    },
    MissingArtifact {
        path: String,
    },
    DuplicateArtifact {
        path: String,
    },
    DuplicateStatus {
        path: String,
    },
    DuplicateFamily {
        family_id: String,
        count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BackendSuiteMatrixCoverageIssue {
    FamilyCountMismatch {
        matrix_family_count: usize,
        suite_family_count: usize,
    },
    MissingMatrixFamily {
        family_id: String,
    },
    ExtraSuiteFamily {
        family_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BackendSuiteBackendIssue {
    Missing {
        expected_backend: String,
    },
    Mismatch {
        expected_backend: String,
        actual_backend: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BackendSuiteArtifactStatusIssue {
    MissingField {
        path: String,
        field: &'static str,
    },
    SourceFingerprintMismatch {
        path: String,
        status_source_fingerprint: String,
        artifact_source_fingerprint: String,
    },
    SourceTreeFingerprintMismatch {
        path: String,
        status_source_tree_fingerprint: String,
        artifact_source_tree_fingerprint: String,
    },
    SelectedBackendMismatch {
        path: String,
        status_selected_backend: String,
        artifact_selected_backend: String,
    },
    CaseCountMismatch {
        path: String,
        status_case_count: u64,
        artifact_case_count: u64,
    },
    FailedCountMismatch {
        path: String,
        status_failed_count: u64,
        artifact_failed_count: u64,
    },
    NumericFieldMismatch {
        path: String,
        field: &'static str,
        status_value: u64,
        artifact_value: u64,
    },
    StringFieldMismatch {
        path: String,
        field: &'static str,
        status_value: String,
        artifact_value: String,
    },
    CpuSota100xContractCaseCountMismatch {
        path: String,
        status_contract_cases: u64,
        artifact_contract_cases: u64,
    },
    CpuSota100xPassingCaseCountMismatch {
        path: String,
        status_passing_cases: u64,
        artifact_passing_cases: u64,
    },
    MissingRequestedCase {
        path: String,
        requested_case_id: String,
    },
    DuplicateRequestedCase {
        path: String,
        requested_case_id: String,
        count: usize,
    },
}

pub(crate) fn source_fingerprint_issues(source_fingerprint: &str) -> Vec<SourceFingerprintIssue> {
    let Some(rest) = source_fingerprint.strip_prefix("git:") else {
        return Vec::new();
    };
    let mut issues = Vec::new();
    if rest.contains(":dirty=unknown") {
        issues.push(SourceFingerprintIssue::DirtyUnknownState {
            source_fingerprint: source_fingerprint.to_string(),
        });
    }
    let Some(dirty_offset) = rest.find(":dirty=true") else {
        return issues;
    };
    let after_dirty = &rest[dirty_offset + ":dirty=true".len()..];
    let Some(worktree) = after_dirty.strip_prefix(":worktree=") else {
        issues.push(SourceFingerprintIssue::DirtyMissingWorktree {
            source_fingerprint: source_fingerprint.to_string(),
        });
        return issues;
    };
    if worktree == "unknown" {
        issues.push(SourceFingerprintIssue::DirtyUnknownWorktree {
            source_fingerprint: source_fingerprint.to_string(),
        });
    } else if !is_blake3_hex_digest(worktree) {
        issues.push(SourceFingerprintIssue::DirtyInvalidWorktree {
            source_fingerprint: source_fingerprint.to_string(),
            worktree: worktree.to_string(),
        });
    }
    issues
}

pub(crate) fn source_fingerprint_freshness_issues(
    source_fingerprint: &str,
    current_source_fingerprint: &str,
) -> Vec<SourceFingerprintFreshnessIssue> {
    if source_fingerprint == current_source_fingerprint {
        Vec::new()
    } else {
        vec![SourceFingerprintFreshnessIssue::Mismatch {
            source_fingerprint: source_fingerprint.to_string(),
            current_source_fingerprint: current_source_fingerprint.to_string(),
        }]
    }
}

pub(crate) fn report_freshness_fingerprint(report: &Value) -> Option<(&'static str, &str)> {
    for field in ["source_tree_fingerprint", "source_fingerprint"] {
        if let Some(value) = report
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            return Some((field, value));
        }
    }
    None
}

pub(crate) fn current_freshness_fingerprint_for_report(
    path: &Path,
    report: &Value,
) -> Option<String> {
    if report
        .get("source_tree_fingerprint")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Some(current_source_tree_fingerprint_for_evidence_path(path)?);
    }
    current_source_fingerprint_for_evidence_path(path)
}

pub(crate) fn current_source_tree_fingerprint_for_evidence_path(path: &Path) -> Option<String> {
    let workspace_root = workspace_root_for_evidence_path(path)?;
    Some(current_source_tree_fingerprint_at(&workspace_root))
}

pub(crate) fn current_source_fingerprint_for_evidence_path(path: &Path) -> Option<String> {
    let workspace_root = workspace_root_for_evidence_path(path)?;
    Some(current_source_fingerprint_at(&workspace_root))
}

fn current_source_fingerprint_at(workspace_root: &Path) -> String {
    let key = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let cache = CURRENT_SOURCE_FINGERPRINTS.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(cache) = cache.lock() {
        if let Some(fingerprint) = cache.get(&key) {
            return fingerprint.clone();
        }
    }

    let git = vyre_bench::probes::capture_git_info_at(workspace_root);
    let fingerprint = vyre_bench::probes::source_fingerprint(&git);
    if let Ok(mut cache) = cache.lock() {
        cache.insert(key, fingerprint.clone());
    }
    fingerprint
}

fn current_source_tree_fingerprint_at(workspace_root: &Path) -> String {
    let key = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let cache = CURRENT_SOURCE_TREE_FINGERPRINTS.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(cache) = cache.lock() {
        if let Some(fingerprint) = cache.get(&key) {
            return fingerprint.clone();
        }
    }

    let fingerprint = vyre_bench::probes::source_tree_fingerprint_at(workspace_root);
    if let Ok(mut cache) = cache.lock() {
        cache.insert(key, fingerprint.clone());
    }
    fingerprint
}

fn workspace_root_for_evidence_path(path: &Path) -> Option<PathBuf> {
    let mut cursor = if path.is_dir() { path } else { path.parent()? };
    loop {
        if cursor.join("Cargo.toml").is_file() && cursor.join("release").is_dir() {
            return Some(cursor.to_path_buf());
        }
        cursor = cursor.parent()?;
    }
}

pub(crate) fn backend_suite_artifact_status_issues(
    status: &Value,
    artifact_report: &Value,
) -> Vec<BackendSuiteArtifactStatusIssue> {
    let path = status
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("<unknown>")
        .to_string();
    let mut issues = Vec::new();

    let status_source = status.get("source_fingerprint").and_then(non_empty_str);
    let artifact_source = artifact_report
        .get("source_fingerprint")
        .and_then(non_empty_str);
    match (status_source, artifact_source) {
        (None, Some(_)) => issues.push(BackendSuiteArtifactStatusIssue::MissingField {
            path: path.clone(),
            field: "source_fingerprint",
        }),
        (Some(status_source), Some(artifact_source)) if status_source != artifact_source => {
            issues.push(BackendSuiteArtifactStatusIssue::SourceFingerprintMismatch {
                path: path.clone(),
                status_source_fingerprint: status_source.to_string(),
                artifact_source_fingerprint: artifact_source.to_string(),
            });
        }
        _ => {}
    }

    let status_source_tree = status
        .get("source_tree_fingerprint")
        .and_then(non_empty_str);
    let artifact_source_tree = artifact_report
        .get("source_tree_fingerprint")
        .and_then(non_empty_str);
    match (status_source_tree, artifact_source_tree) {
        (None, Some(_)) => issues.push(BackendSuiteArtifactStatusIssue::MissingField {
            path: path.clone(),
            field: "source_tree_fingerprint",
        }),
        (Some(status_source_tree), Some(artifact_source_tree))
            if status_source_tree != artifact_source_tree =>
        {
            issues.push(
                BackendSuiteArtifactStatusIssue::SourceTreeFingerprintMismatch {
                    path: path.clone(),
                    status_source_tree_fingerprint: status_source_tree.to_string(),
                    artifact_source_tree_fingerprint: artifact_source_tree.to_string(),
                },
            );
        }
        _ => {}
    }

    let status_backend = status.get("selected_backend").and_then(non_empty_str);
    let artifact_backend = artifact_report
        .get("selected_backend")
        .and_then(non_empty_str);
    match (status_backend, artifact_backend) {
        (None, Some(_)) => issues.push(BackendSuiteArtifactStatusIssue::MissingField {
            path: path.clone(),
            field: "selected_backend",
        }),
        (Some(status_backend), Some(artifact_backend)) if status_backend != artifact_backend => {
            issues.push(BackendSuiteArtifactStatusIssue::SelectedBackendMismatch {
                path: path.clone(),
                status_selected_backend: status_backend.to_string(),
                artifact_selected_backend: artifact_backend.to_string(),
            });
        }
        _ => {}
    }

    let status_case_count = status.get("case_count").and_then(Value::as_u64);
    let artifact_case_count = artifact_report
        .get("cases")
        .and_then(Value::as_array)
        .map(|cases| cases.len() as u64);
    let artifact_summary_total_cases = artifact_report
        .get("summary")
        .and_then(|summary| summary.get("total_cases"))
        .and_then(Value::as_u64);
    if artifact_case_count.is_some() && artifact_summary_total_cases.is_none() {
        issues.push(BackendSuiteArtifactStatusIssue::MissingField {
            path: path.clone(),
            field: "summary.total_cases",
        });
    }
    if let (Some(summary_total_cases), Some(case_count)) =
        (artifact_summary_total_cases, artifact_case_count)
    {
        if summary_total_cases != case_count {
            issues.push(BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                path: path.clone(),
                field: "summary.total_cases",
                status_value: summary_total_cases,
                artifact_value: case_count,
            });
        }
    }
    match (status_case_count, artifact_case_count) {
        (None, Some(_)) => issues.push(BackendSuiteArtifactStatusIssue::MissingField {
            path: path.clone(),
            field: "case_count",
        }),
        (Some(status_case_count), Some(artifact_case_count))
            if status_case_count != artifact_case_count =>
        {
            issues.push(BackendSuiteArtifactStatusIssue::CaseCountMismatch {
                path: path.clone(),
                status_case_count,
                artifact_case_count,
            });
        }
        _ => {}
    }

    let status_nonmatching_backend_count = status
        .get("nonmatching_case_backend_count")
        .and_then(Value::as_u64);
    let artifact_nonmatching_backend_count =
        artifact_nonmatching_case_backend_count(status, artifact_report);
    match (
        status_nonmatching_backend_count,
        artifact_nonmatching_backend_count,
    ) {
        (None, Some(_)) => issues.push(BackendSuiteArtifactStatusIssue::MissingField {
            path: path.clone(),
            field: "nonmatching_case_backend_count",
        }),
        (Some(status_value), Some(artifact_value)) if status_value != artifact_value => {
            issues.push(BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                path: path.clone(),
                field: "nonmatching_case_backend_count",
                status_value,
                artifact_value,
            });
        }
        _ => {}
    }

    let status_failed_count = status.get("failed_count").and_then(Value::as_u64);
    let artifact_summary_passed_count = artifact_report
        .get("summary")
        .and_then(|summary| summary.get("passed"))
        .and_then(Value::as_u64);
    let artifact_case_passed_count = artifact_case_passed_count(artifact_report);
    if artifact_case_passed_count.is_some() && artifact_summary_passed_count.is_none() {
        issues.push(BackendSuiteArtifactStatusIssue::MissingField {
            path: path.clone(),
            field: "summary.passed",
        });
    }
    if let (Some(summary_passed_count), Some(case_passed_count)) =
        (artifact_summary_passed_count, artifact_case_passed_count)
    {
        if summary_passed_count != case_passed_count {
            issues.push(BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                path: path.clone(),
                field: "summary.passed",
                status_value: summary_passed_count,
                artifact_value: case_passed_count,
            });
        }
    }
    let artifact_summary_failed_count = artifact_report
        .get("summary")
        .and_then(|summary| summary.get("failed"))
        .and_then(Value::as_u64);
    let artifact_case_failed_count = artifact_case_failed_count(artifact_report);
    if artifact_case_failed_count.is_some() && artifact_summary_failed_count.is_none() {
        issues.push(BackendSuiteArtifactStatusIssue::MissingField {
            path: path.clone(),
            field: "summary.failed",
        });
    }
    if let (Some(summary_failed_count), Some(case_failed_count)) =
        (artifact_summary_failed_count, artifact_case_failed_count)
    {
        if summary_failed_count != case_failed_count {
            issues.push(BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                path: path.clone(),
                field: "summary.failed",
                status_value: summary_failed_count,
                artifact_value: case_failed_count,
            });
        }
    }
    let artifact_failed_count = artifact_case_failed_count.or(artifact_summary_failed_count);
    match (status_failed_count, artifact_failed_count) {
        (None, Some(_)) => issues.push(BackendSuiteArtifactStatusIssue::MissingField {
            path: path.clone(),
            field: "failed_count",
        }),
        (Some(status_failed_count), Some(artifact_failed_count))
            if status_failed_count != artifact_failed_count =>
        {
            issues.push(BackendSuiteArtifactStatusIssue::FailedCountMismatch {
                path: path.clone(),
                status_failed_count,
                artifact_failed_count,
            });
        }
        _ => {}
    }

    for (field, artifact_value) in backend_suite_numeric_artifact_fields(artifact_report) {
        match status.get(field).and_then(Value::as_u64) {
            None => issues.push(BackendSuiteArtifactStatusIssue::MissingField {
                path: path.clone(),
                field,
            }),
            Some(status_value) if status_value != artifact_value => {
                issues.push(BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: path.clone(),
                    field,
                    status_value,
                    artifact_value,
                });
            }
            _ => {}
        }
    }
    for (field, artifact_value) in backend_suite_string_artifact_fields(artifact_report) {
        match (status.get(field).and_then(non_empty_str), artifact_value) {
            (_, None) => issues.push(BackendSuiteArtifactStatusIssue::MissingField {
                path: path.clone(),
                field,
            }),
            (None, Some(_)) => issues.push(BackendSuiteArtifactStatusIssue::MissingField {
                path: path.clone(),
                field,
            }),
            (Some(status_value), Some(artifact_value)) if status_value != artifact_value => {
                issues.push(BackendSuiteArtifactStatusIssue::StringFieldMismatch {
                    path: path.clone(),
                    field,
                    status_value: status_value.to_string(),
                    artifact_value,
                });
            }
            _ => {}
        }
    }

    let (artifact_contract_cases, artifact_passing_cases) =
        cpu_sota_100x_case_counts(artifact_report);
    match status
        .get("cpu_sota_100x_contract_cases")
        .and_then(Value::as_u64)
    {
        None if artifact_contract_cases > 0 => {
            issues.push(BackendSuiteArtifactStatusIssue::MissingField {
                path: path.clone(),
                field: "cpu_sota_100x_contract_cases",
            });
        }
        Some(status_contract_cases) if status_contract_cases != artifact_contract_cases => {
            issues.push(
                BackendSuiteArtifactStatusIssue::CpuSota100xContractCaseCountMismatch {
                    path: path.clone(),
                    status_contract_cases,
                    artifact_contract_cases,
                },
            );
        }
        _ => {}
    }
    match status
        .get("cpu_sota_100x_passing_cases")
        .and_then(Value::as_u64)
    {
        None if artifact_passing_cases > 0 => {
            issues.push(BackendSuiteArtifactStatusIssue::MissingField {
                path: path.clone(),
                field: "cpu_sota_100x_passing_cases",
            });
        }
        Some(status_passing_cases) if status_passing_cases != artifact_passing_cases => {
            issues.push(
                BackendSuiteArtifactStatusIssue::CpuSota100xPassingCaseCountMismatch {
                    path: path.clone(),
                    status_passing_cases,
                    artifact_passing_cases,
                },
            );
        }
        _ => {}
    }

    if let Some(requested_case_id) = status.get("requested_case_id").and_then(non_empty_str) {
        let requested_case_count =
            artifact_report
                .get("cases")
                .and_then(Value::as_array)
                .map(|cases| {
                    cases
                        .iter()
                        .filter(|case| {
                            case.get("id").and_then(Value::as_str) == Some(requested_case_id)
                        })
                        .count()
                });
        match requested_case_count {
            Some(0) => issues.push(BackendSuiteArtifactStatusIssue::MissingRequestedCase {
                path: path.clone(),
                requested_case_id: requested_case_id.to_string(),
            }),
            Some(count) if count > 1 => {
                issues.push(BackendSuiteArtifactStatusIssue::DuplicateRequestedCase {
                    path,
                    requested_case_id: requested_case_id.to_string(),
                    count,
                });
            }
            _ => {}
        }
    }

    issues
}

fn backend_suite_numeric_artifact_fields(artifact_report: &Value) -> Vec<(&'static str, u64)> {
    let fields = [
        (
            "min_wall_samples",
            artifact_min_metric_samples(artifact_report, "wall_ns"),
        ),
        (
            "min_baseline_wall_samples",
            artifact_min_metric_samples(artifact_report, "baseline_wall_ns"),
        ),
        (
            "min_wall_p50",
            artifact_min_metric_percentile(artifact_report, "wall_ns", "p50"),
        ),
        (
            "min_wall_p95",
            artifact_min_metric_percentile(artifact_report, "wall_ns", "p95"),
        ),
        (
            "min_wall_p99",
            artifact_min_metric_percentile(artifact_report, "wall_ns", "p99"),
        ),
        (
            "min_baseline_wall_p50",
            artifact_min_metric_percentile(artifact_report, "baseline_wall_ns", "p50"),
        ),
        (
            "min_baseline_wall_p95",
            artifact_min_metric_percentile(artifact_report, "baseline_wall_ns", "p95"),
        ),
        (
            "min_baseline_wall_p99",
            artifact_min_metric_percentile(artifact_report, "baseline_wall_ns", "p99"),
        ),
        (
            "min_kernel_launches",
            artifact_min_metric_percentile(artifact_report, "kernel_launches", "p50"),
        ),
        (
            "min_cuda_ptx_source_cache_entries",
            artifact_min_metric_percentile(artifact_report, "cuda_ptx_source_cache_entries", "p50"),
        ),
        (
            "min_cuda_ptx_source_cache_hits",
            artifact_min_metric_percentile(artifact_report, "cuda_ptx_source_cache_hits", "p50"),
        ),
        (
            "min_cuda_ptx_source_cache_misses",
            artifact_min_metric_percentile(artifact_report, "cuda_ptx_source_cache_misses", "p50"),
        ),
        (
            "gpu_memory_total_mib",
            artifact_environment_first_gpu_u64(artifact_report, "memory_total_mib"),
        ),
        (
            "gpu_compute_capability_major",
            artifact_environment_first_gpu_u64(artifact_report, "compute_capability_major"),
        ),
        (
            "gpu_compute_capability_minor",
            artifact_environment_first_gpu_u64(artifact_report, "compute_capability_minor"),
        ),
    ];
    fields
        .into_iter()
        .filter_map(|(field, value)| value.map(|value| (field, value)))
        .collect()
}

fn backend_suite_string_artifact_fields(
    artifact_report: &Value,
) -> Vec<(&'static str, Option<String>)> {
    let fields = [
        (
            "host_cpu_model",
            artifact_environment_host_cpu_model(artifact_report),
        ),
        (
            "gpu_model",
            artifact_environment_first_gpu_str(artifact_report, "name"),
        ),
        (
            "nvidia_driver_version",
            artifact_environment_str(artifact_report, "nvidia_driver_version"),
        ),
        (
            "nvidia_cuda_version",
            artifact_environment_str(artifact_report, "nvidia_cuda_version"),
        ),
    ];
    fields
        .into_iter()
        .filter(|(_, value)| value.is_some())
        .map(|(field, value)| (field, value.flatten()))
        .collect()
}

fn artifact_environment<'a>(artifact_report: &'a Value) -> Option<&'a Value> {
    artifact_report.get("environment")
}

fn artifact_environment_str(artifact_report: &Value, field: &str) -> Option<Option<String>> {
    let value = artifact_environment(artifact_report)?.get(field)?;
    Some(non_empty_str(value).map(str::to_string))
}

fn artifact_environment_host_cpu_model(artifact_report: &Value) -> Option<Option<String>> {
    let environment = artifact_environment(artifact_report)?;
    let value = environment
        .get("host_cpu_model")
        .or_else(|| environment.get("cpu_model"))
        .or_else(|| environment.get("host_cpu"))?;
    Some(non_empty_str(value).map(str::to_string))
}

fn artifact_environment_first_gpu<'a>(artifact_report: &'a Value) -> Option<&'a Value> {
    artifact_environment(artifact_report)?
        .get("gpu_devices")
        .and_then(Value::as_array)
        .and_then(|devices| devices.first())
}

fn artifact_environment_first_gpu_str(
    artifact_report: &Value,
    field: &str,
) -> Option<Option<String>> {
    let value = artifact_environment_first_gpu(artifact_report)?.get(field)?;
    Some(non_empty_str(value).map(str::to_string))
}

fn artifact_environment_first_gpu_u64(artifact_report: &Value, field: &str) -> Option<u64> {
    artifact_environment_first_gpu(artifact_report)?
        .get(field)
        .and_then(Value::as_u64)
}

fn artifact_nonmatching_case_backend_count(status: &Value, artifact_report: &Value) -> Option<u64> {
    let expected_backend = status
        .get("selected_backend")
        .and_then(non_empty_str)
        .or_else(|| {
            artifact_report
                .get("selected_backend")
                .and_then(non_empty_str)
        })?;
    let cases = artifact_report.get("cases").and_then(Value::as_array)?;
    Some(
        cases
            .iter()
            .filter(|case| case.get("backend_id").and_then(Value::as_str) != Some(expected_backend))
            .count() as u64,
    )
}

fn artifact_case_failed_count(artifact_report: &Value) -> Option<u64> {
    let cases = artifact_report.get("cases").and_then(Value::as_array)?;
    Some(
        cases
            .iter()
            .filter(|case| !benchmark_case_passes_summary_evidence(case))
            .count() as u64,
    )
}

fn artifact_case_passed_count(artifact_report: &Value) -> Option<u64> {
    let cases = artifact_report.get("cases").and_then(Value::as_array)?;
    Some(
        cases
            .iter()
            .filter(|case| benchmark_case_passes_summary_evidence(case))
            .count() as u64,
    )
}

fn artifact_min_metric_samples(artifact_report: &Value, metric_name: &str) -> Option<u64> {
    let cases = artifact_report.get("cases").and_then(Value::as_array)?;
    if cases.is_empty() {
        return None;
    }
    let mut seen_metric = false;
    let min = cases
        .iter()
        .map(|case| {
            let metric = case
                .get("metrics")
                .and_then(|metrics| metrics.get(metric_name));
            if metric.is_some() {
                seen_metric = true;
            }
            metric
                .and_then(|metric| metric.get("samples"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        })
        .min()
        .unwrap_or(0);
    seen_metric.then_some(min)
}

fn artifact_min_metric_percentile(
    artifact_report: &Value,
    metric_name: &str,
    percentile: &str,
) -> Option<u64> {
    let cases = artifact_report.get("cases").and_then(Value::as_array)?;
    if cases.is_empty() {
        return None;
    }
    let mut seen_metric = false;
    let min = cases
        .iter()
        .map(|case| {
            let metric = case
                .get("metrics")
                .and_then(|metrics| metrics.get(metric_name));
            if metric.is_some() {
                seen_metric = true;
            }
            metric
                .and_then(|metric| metric.get(percentile))
                .and_then(nonnegative_json_number_as_u64)
                .unwrap_or(0)
        })
        .min()
        .unwrap_or(0);
    seen_metric.then_some(min)
}

fn nonnegative_json_number_as_u64(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_f64()
            .filter(|value| *value >= 0.0)
            .map(|value| value as u64)
    })
}

pub(crate) fn cpu_sota_100x_case_counts(artifact_report: &Value) -> (u64, u64) {
    let report_backend = artifact_report
        .get("selected_backend")
        .and_then(Value::as_str);
    let Some(cases) = artifact_report.get("cases").and_then(Value::as_array) else {
        return (0, 0);
    };
    cases
        .iter()
        .fold((0, 0), |(contract_count, passing_count), case| {
            let case_backend = case
                .get("backend_id")
                .and_then(Value::as_str)
                .or(report_backend);
            if !benchmark_case_has_cpu_sota_contract(case, case_backend, 100.0) {
                return (contract_count, passing_count);
            }
            (
                contract_count + 1,
                passing_count + u64::from(benchmark_case_proves_cpu_sota_100x(case, case_backend)),
            )
        })
}

pub(crate) fn benchmark_case_proves_cpu_sota_100x(case: &Value, backend_id: Option<&str>) -> bool {
    benchmark_case_has_cpu_sota_contract(case, backend_id, 100.0)
        && benchmark_case_passes_summary_evidence(case)
        && case
            .get("performance")
            .and_then(|performance| performance.get("contract_passed"))
            .and_then(Value::as_bool)
            == Some(true)
        && case
            .get("performance")
            .and_then(|performance| performance.get("speedup_x"))
            .and_then(Value::as_f64)
            .is_some_and(|speedup| speedup >= 100.0)
        && cpu_sota_100x_measured_speedup(case)
            .is_some_and(|measured_speedup| measured_speedup >= 100.0)
}

fn cpu_sota_100x_measured_speedup(case: &Value) -> Option<f64> {
    let metrics = case.get("metrics").and_then(Value::as_object)?;
    let active_gpu = metrics
        .get("dispatch_ns")
        .or_else(|| metrics.get("kernel_execute_ns"))
        .or_else(|| metrics.get("wall_ns"));
    let wall = metric_p50_f64(active_gpu)?;
    let baseline = metric_p50_f64(metrics.get("baseline_wall_ns"))?;
    (wall > 0.0).then_some(baseline / wall)
}

fn metric_p50_f64(metric: Option<&Value>) -> Option<f64> {
    let metric = metric?;
    metric
        .get("p50")
        .and_then(Value::as_f64)
        .or_else(|| {
            metric
                .get("p50")
                .and_then(Value::as_u64)
                .map(|value| value as f64)
        })
        .or_else(|| metric.as_f64())
        .or_else(|| metric.as_u64().map(|value| value as f64))
}

pub(crate) fn benchmark_case_has_cpu_sota_contract(
    case: &Value,
    backend_id: Option<&str>,
    required_speedup: f64,
) -> bool {
    case.get("contract")
        .and_then(|contract| contract.get("baselines"))
        .and_then(Value::as_array)
        .is_some_and(|baselines| {
            baselines.iter().any(|baseline| {
                baseline.get("class").and_then(Value::as_str) == Some("CpuSota")
                    && baseline
                        .get("min_speedup_x")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0)
                        >= required_speedup
                    && baseline_applies_to_backend(baseline, backend_id)
            })
        })
}

pub(crate) fn baseline_applies_to_backend(baseline: &Value, backend_id: Option<&str>) -> bool {
    let Some(backend_ids) = baseline.get("backend_ids").and_then(Value::as_array) else {
        return true;
    };
    if backend_ids.is_empty() {
        return true;
    }
    let Some(backend_id) = backend_id else {
        return false;
    };
    backend_ids
        .iter()
        .any(|candidate| candidate.as_str() == Some(backend_id))
}

pub(crate) fn backend_suite_inventory_issues(suite: &Value) -> Vec<BackendSuiteInventoryIssue> {
    let artifact_count = suite_array_len(suite, "artifacts");
    let status_count = suite_array_len(suite, "artifact_statuses");
    let artifact_counts = suite_artifact_path_counts(suite);
    let status_counts = suite_status_path_counts(suite);
    let status_family_counts = suite_status_family_counts(suite);
    let artifact_paths = artifact_counts.keys().cloned().collect::<BTreeSet<_>>();
    let status_paths = status_counts.keys().cloned().collect::<BTreeSet<_>>();
    let mut issues = Vec::new();

    if artifact_count != status_count {
        issues.push(BackendSuiteInventoryIssue::CountMismatch {
            artifact_count,
            status_count,
        });
    }
    if let Some(family_count) = suite.get("family_count").and_then(Value::as_u64) {
        if family_count as usize != artifact_count {
            issues.push(
                BackendSuiteInventoryIssue::DeclaredFamilyArtifactCountMismatch {
                    family_count,
                    artifact_count,
                },
            );
        }
        if family_count as usize != status_family_counts.len() {
            issues.push(
                BackendSuiteInventoryIssue::DeclaredFamilyStatusCountMismatch {
                    family_count,
                    status_family_count: status_family_counts.len(),
                },
            );
        }
    }
    for (path, count) in artifact_counts {
        if count > 1 {
            issues.push(BackendSuiteInventoryIssue::DuplicateArtifact { path });
        }
    }
    for (path, count) in status_counts {
        if count > 1 {
            issues.push(BackendSuiteInventoryIssue::DuplicateStatus { path });
        }
    }
    for (family_id, count) in status_family_counts {
        if count > 1 {
            issues.push(BackendSuiteInventoryIssue::DuplicateFamily { family_id, count });
        }
    }
    for path in artifact_paths.difference(&status_paths) {
        issues.push(BackendSuiteInventoryIssue::MissingStatus { path: path.clone() });
    }
    for path in status_paths.difference(&artifact_paths) {
        issues.push(BackendSuiteInventoryIssue::MissingArtifact { path: path.clone() });
    }
    issues
}

pub(crate) fn backend_suite_matrix_coverage_issues(
    matrix: &Value,
    suite: &Value,
) -> Vec<BackendSuiteMatrixCoverageIssue> {
    let matrix_family_ids = matrix
        .get("families")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|family| family.get("id").and_then(non_empty_str))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let suite_family_ids = suite_status_family_counts(suite)
        .into_keys()
        .collect::<BTreeSet<_>>();
    let mut issues = Vec::new();

    if matrix_family_ids.len() != suite_family_ids.len() {
        issues.push(BackendSuiteMatrixCoverageIssue::FamilyCountMismatch {
            matrix_family_count: matrix_family_ids.len(),
            suite_family_count: suite_family_ids.len(),
        });
    }
    for family_id in matrix_family_ids.difference(&suite_family_ids) {
        issues.push(BackendSuiteMatrixCoverageIssue::MissingMatrixFamily {
            family_id: family_id.clone(),
        });
    }
    for family_id in suite_family_ids.difference(&matrix_family_ids) {
        issues.push(BackendSuiteMatrixCoverageIssue::ExtraSuiteFamily {
            family_id: family_id.clone(),
        });
    }
    issues
}

pub(crate) fn describe_backend_suite_matrix_coverage_issue(
    issue: &BackendSuiteMatrixCoverageIssue,
) -> String {
    match issue {
        BackendSuiteMatrixCoverageIssue::FamilyCountMismatch {
            matrix_family_count,
            suite_family_count,
        } => format!(
            "covers {suite_family_count} workload family/families, but release-workload-matrix lists {matrix_family_count}"
        ),
        BackendSuiteMatrixCoverageIssue::MissingMatrixFamily { family_id } => {
            format!("is missing release-workload-matrix family `{family_id}`")
        }
        BackendSuiteMatrixCoverageIssue::ExtraSuiteFamily { family_id } => {
            format!("contains family `{family_id}` absent from release-workload-matrix")
        }
    }
}

pub(crate) fn describe_backend_suite_inventory_issue(issue: &BackendSuiteInventoryIssue) -> String {
    match issue {
        BackendSuiteInventoryIssue::CountMismatch {
            artifact_count,
            status_count,
        } => {
            format!("inventory count mismatch: artifacts={artifact_count}, artifact_statuses={status_count}")
        }
        BackendSuiteInventoryIssue::DeclaredFamilyArtifactCountMismatch {
            family_count,
            artifact_count,
        } => {
            format!("family_count={family_count}, but artifacts has {artifact_count} row(s)")
        }
        BackendSuiteInventoryIssue::DeclaredFamilyStatusCountMismatch {
            family_count,
            status_family_count,
        } => {
            format!(
                "family_count={family_count}, but artifact_statuses has {status_family_count} unique family_id row(s)"
            )
        }
        BackendSuiteInventoryIssue::MissingStatus { path } => {
            format!("lists artifact `{path}` without matching artifact_statuses entry")
        }
        BackendSuiteInventoryIssue::MissingArtifact { path } => {
            format!("has artifact_statuses path `{path}` absent from artifacts")
        }
        BackendSuiteInventoryIssue::DuplicateArtifact { path } => {
            format!("has duplicate artifact `{path}`")
        }
        BackendSuiteInventoryIssue::DuplicateStatus { path } => {
            format!("has duplicate artifact_statuses path `{path}`")
        }
        BackendSuiteInventoryIssue::DuplicateFamily { family_id, count } => {
            format!("has {count} artifact_statuses rows for family `{family_id}`")
        }
    }
}

pub(crate) fn backend_suite_parity_issues(
    cuda_suite: &Value,
    wgpu_suite: &Value,
) -> Vec<BackendSuiteParityIssue> {
    let cuda_count = suite_artifact_status_count(cuda_suite);
    let wgpu_count = suite_artifact_status_count(wgpu_suite);
    let cuda_pairs = suite_family_case_pairs(cuda_suite);
    let wgpu_pairs = suite_family_case_pairs(wgpu_suite);
    let mut issues = Vec::new();
    if let Some(issue) = backend_suite_backend_issue(cuda_suite, "cuda") {
        issues.push(BackendSuiteParityIssue::CudaBackendIdentity { issue });
    }
    if let Some(issue) = backend_suite_backend_issue(wgpu_suite, "wgpu") {
        issues.push(BackendSuiteParityIssue::WgpuBackendIdentity { issue });
    }
    if cuda_count != wgpu_count || cuda_pairs.len() != wgpu_pairs.len() {
        issues.push(BackendSuiteParityIssue::CountMismatch {
            cuda_count,
            wgpu_count,
        });
    }
    for ((family_id, requested_case_id), count) in suite_family_case_pair_counts(cuda_suite) {
        if count > 1 {
            issues.push(BackendSuiteParityIssue::DuplicateCudaPair {
                family_id,
                requested_case_id,
                count,
            });
        }
    }
    for ((family_id, requested_case_id), count) in suite_family_case_pair_counts(wgpu_suite) {
        if count > 1 {
            issues.push(BackendSuiteParityIssue::DuplicateWgpuPair {
                family_id,
                requested_case_id,
                count,
            });
        }
    }
    let cuda_paths = suite_all_artifact_paths(cuda_suite);
    let wgpu_paths = suite_all_artifact_paths(wgpu_suite);
    for path in cuda_paths.intersection(&wgpu_paths) {
        issues.push(BackendSuiteParityIssue::SharedArtifactPath { path: path.clone() });
    }
    for (family_id, requested_case_id) in cuda_pairs.difference(&wgpu_pairs) {
        issues.push(BackendSuiteParityIssue::MissingWgpuPair {
            family_id: family_id.clone(),
            requested_case_id: requested_case_id.clone(),
        });
    }
    for (family_id, requested_case_id) in wgpu_pairs.difference(&cuda_pairs) {
        issues.push(BackendSuiteParityIssue::MissingCudaPair {
            family_id: family_id.clone(),
            requested_case_id: requested_case_id.clone(),
        });
    }
    let cuda_statuses = suite_statuses_by_family_case_pair(cuda_suite);
    let wgpu_statuses = suite_statuses_by_family_case_pair(wgpu_suite);
    for pair in cuda_pairs.intersection(&wgpu_pairs) {
        let Some(cuda_status) = cuda_statuses.get(pair) else {
            continue;
        };
        let Some(wgpu_status) = wgpu_statuses.get(pair) else {
            continue;
        };
        for field in [
            "case_count",
            "failed_count",
            "nonmatching_case_backend_count",
            "cpu_sota_100x_contract_cases",
            "cpu_sota_100x_passing_cases",
        ] {
            let cuda_value = cuda_status.get(field).and_then(Value::as_u64);
            let wgpu_value = wgpu_status.get(field).and_then(Value::as_u64);
            if cuda_value != wgpu_value {
                issues.push(BackendSuiteParityIssue::StatusFieldMismatch {
                    family_id: pair.0.clone(),
                    requested_case_id: pair.1.clone(),
                    field,
                    cuda_value,
                    wgpu_value,
                });
            }
        }
        for field in ["source_tree_fingerprint"] {
            let cuda_value = cuda_status
                .get(field)
                .and_then(non_empty_str)
                .map(str::to_string);
            let wgpu_value = wgpu_status
                .get(field)
                .and_then(non_empty_str)
                .map(str::to_string);
            if cuda_value != wgpu_value {
                issues.push(BackendSuiteParityIssue::StatusStringFieldMismatch {
                    family_id: pair.0.clone(),
                    requested_case_id: pair.1.clone(),
                    field,
                    cuda_value,
                    wgpu_value,
                });
            }
        }
        let cuda_blockers = suite_status_blockers(cuda_status);
        let wgpu_blockers = suite_status_blockers(wgpu_status);
        if cuda_blockers != wgpu_blockers {
            issues.push(BackendSuiteParityIssue::StatusBlockersMismatch {
                family_id: pair.0.clone(),
                requested_case_id: pair.1.clone(),
                cuda_blockers,
                wgpu_blockers,
            });
        }
    }
    issues
}

fn suite_status_blockers(status: &Value) -> Option<Vec<String>> {
    status
        .get("blockers")
        .and_then(Value::as_array)
        .map(|blockers| {
            blockers
                .iter()
                .map(|blocker| {
                    blocker
                        .as_str()
                        .unwrap_or("<non-string blocker>")
                        .to_string()
                })
                .collect()
        })
}

pub(crate) fn benchmark_evidence_blocker_issues(evidence: &str, value: &Value) -> Vec<String> {
    let mut blockers = Vec::new();
    let Some(artifact_blockers) = value.get("blockers").and_then(Value::as_array) else {
        blockers.push(format!("`{evidence}` is missing blockers array"));
        return blockers;
    };
    for (index, blocker) in artifact_blockers.iter().enumerate() {
        let blocker = blocker.as_str().unwrap_or("<non-string blocker>");
        blockers.push(format!("`{evidence}` blocker[{index}]: {blocker}"));
    }
    collect_benchmark_suite_status_blocker_issues(evidence, value, &mut blockers);
    blockers
}

fn collect_benchmark_suite_status_blocker_issues(
    evidence: &str,
    value: &Value,
    blockers: &mut Vec<String>,
) {
    let Some(statuses) = value.get("artifact_statuses") else {
        if expected_backend_for_suite_evidence(evidence).is_some() {
            blockers.push(format!("`{evidence}` is missing artifact_statuses array"));
        }
        return;
    };
    let Some(statuses) = statuses.as_array() else {
        blockers.push(format!("`{evidence}` artifact_statuses must be an array"));
        return;
    };
    for (status_index, status) in statuses.iter().enumerate() {
        let status_path = status
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let Some(status_blockers) = status.get("blockers").and_then(Value::as_array) else {
            blockers.push(format!(
                "`{evidence}` artifact_statuses[{status_index}] `{status_path}` is missing blockers array"
            ));
            continue;
        };
        for (blocker_index, blocker) in status_blockers.iter().enumerate() {
            let blocker = blocker.as_str().unwrap_or("<non-string blocker>");
            blockers.push(format!(
                "`{evidence}` artifact_statuses[{status_index}] `{status_path}` blocker[{blocker_index}]: {blocker}"
            ));
        }
    }
}

pub(crate) fn expected_backend_for_suite_evidence(evidence: &str) -> Option<&'static str> {
    if evidence.ends_with("cuda-release-suite.json") {
        Some("cuda")
    } else if evidence.ends_with("wgpu-fallback-suite.json") {
        Some("wgpu")
    } else {
        None
    }
}

pub(crate) fn backend_suite_backend_issue(
    suite: &Value,
    expected_backend: &str,
) -> Option<BackendSuiteBackendIssue> {
    match suite.get("backend").and_then(non_empty_str) {
        None => Some(BackendSuiteBackendIssue::Missing {
            expected_backend: expected_backend.to_string(),
        }),
        Some(actual_backend) if actual_backend != expected_backend => {
            Some(BackendSuiteBackendIssue::Mismatch {
                expected_backend: expected_backend.to_string(),
                actual_backend: actual_backend.to_string(),
            })
        }
        Some(_) => None,
    }
}

pub(crate) fn backend_consistency_issues(report: &Value) -> Vec<BackendConsistencyIssue> {
    let Some(expected_backend) = report
        .get("selected_backend")
        .and_then(Value::as_str)
        .filter(|backend| !backend.trim().is_empty())
    else {
        return Vec::new();
    };
    let Some(cases) = report.get("cases").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut issues = Vec::new();
    let mut case_id_counts = BTreeMap::new();
    for (case_index, case) in cases.iter().enumerate() {
        let case_id = case.get("id").and_then(non_empty_str).map(str::to_string);
        if case_id.is_none() {
            issues.push(BackendConsistencyIssue::MissingCaseId { case_index });
        }
        if let Some(case_id) = &case_id {
            *case_id_counts.entry(case_id.clone()).or_insert(0) += 1;
        }
        let case_id = case_id.unwrap_or_else(|| "<unknown>".to_string());
        match case
            .get("backend_id")
            .and_then(Value::as_str)
            .filter(|backend| !backend.trim().is_empty())
        {
            Some(actual_backend) if actual_backend == expected_backend => {}
            Some(actual_backend) => issues.push(BackendConsistencyIssue::CaseBackendMismatch {
                case_id,
                expected_backend: expected_backend.to_string(),
                actual_backend: actual_backend.to_string(),
            }),
            None => issues.push(BackendConsistencyIssue::MissingCaseBackend {
                case_id,
                expected_backend: expected_backend.to_string(),
            }),
        }
    }
    for (case_id, count) in case_id_counts {
        if count > 1 {
            issues.push(BackendConsistencyIssue::DuplicateCaseId { case_id, count });
        }
    }
    issues
}

pub(crate) fn contract_backend_issues(report: &Value) -> Vec<ContractBackendIssue> {
    let report_backend = report.get("selected_backend").and_then(non_empty_str);
    let Some(cases) = report.get("cases").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut issues = Vec::new();
    for case in cases {
        let case_id = case_id(case);
        let Some(backend_id) = case
            .get("backend_id")
            .and_then(non_empty_str)
            .or(report_backend)
        else {
            continue;
        };
        let Some(contract) = case.get("contract").filter(|contract| !contract.is_null()) else {
            continue;
        };
        let Some(baselines) = contract.get("baselines").and_then(Value::as_array) else {
            issues.push(ContractBackendIssue::MissingBaselines {
                case_id,
                backend_id: backend_id.to_string(),
            });
            continue;
        };
        if baselines.is_empty() {
            issues.push(ContractBackendIssue::MissingBaselines {
                case_id,
                backend_id: backend_id.to_string(),
            });
            continue;
        }
        let applies = baselines
            .iter()
            .any(|baseline| baseline_applies_to_backend(baseline, Some(backend_id)));
        if !applies {
            issues.push(ContractBackendIssue::NoApplicableBaseline {
                case_id,
                backend_id: backend_id.to_string(),
            });
        }
    }
    issues
}

pub(crate) fn cuda_telemetry_label_issues(report: &Value) -> Vec<CudaTelemetryLabelIssue> {
    if report.get("selected_backend").and_then(Value::as_str) != Some("cuda") {
        return Vec::new();
    }
    let Some(cases) = report.get("cases").and_then(Value::as_array) else {
        return Vec::new();
    };

    const CHECKS: &[(&str, &[&str])] = &[
        (
            "cuda-ptx-source-cache",
            &[
                "cuda_ptx_source_cache_entries",
                "cuda_ptx_source_cache_hits",
                "cuda_ptx_source_cache_misses",
            ],
        ),
        ("cuda-graph-replay", &["cuda_graph_launches"]),
        (
            "cuda-graph-materialized-output-cache",
            &["cuda_graph_materialized_cache_hits"],
        ),
        (
            "cuda-transfer-operation-telemetry",
            &[
                "cuda_host_upload_operations",
                "cuda_device_readback_operations",
            ],
        ),
        (
            "cuda-resident-borrowed-escape-hatch",
            &["cuda_resident_borrowed_fallback_dispatches"],
        ),
    ];

    cases
        .iter()
        .flat_map(|case| {
            let metrics = case.get("metrics").and_then(Value::as_object);
            let case_id = case_id(case);
            CHECKS.iter().filter_map(move |(label, counters)| {
                let counters_active =
                    metric_value_any(metrics, counters).is_some_and(|value| value > 0.0);
                let label_present = optimization_passes_contain(case, label);
                match (counters_active, label_present) {
                    (true, false) => Some(CudaTelemetryLabelIssue::MissingLabel {
                        case_id: case_id.clone(),
                        label,
                    }),
                    (false, true) => Some(CudaTelemetryLabelIssue::LabelWithoutCounters {
                        case_id: case_id.clone(),
                        label,
                    }),
                    _ => None,
                }
            })
        })
        .collect()
}

pub(crate) fn cuda_forbidden_telemetry_issues(report: &Value) -> Vec<CudaForbiddenTelemetryIssue> {
    if report.get("selected_backend").and_then(Value::as_str) != Some("cuda") {
        return Vec::new();
    }
    let Some(cases) = report.get("cases").and_then(Value::as_array) else {
        return Vec::new();
    };

    cases
        .iter()
        .filter_map(|case| {
            let metrics = case.get("metrics").and_then(Value::as_object);
            let observed_p50 =
                metric_value_any(metrics, &["cuda_resident_borrowed_fallback_dispatches"])?;
            (observed_p50 > 0.0).then(
                || CudaForbiddenTelemetryIssue::ResidentBorrowedEscapeHatch {
                    case_id: case_id(case),
                    observed_p50,
                },
            )
        })
        .collect()
}

pub(crate) fn launch_plan_label_issues(
    case: &Value,
    metrics: Option<&Map<String, Value>>,
) -> Vec<LaunchPlanLabelIssue> {
    let Some(launch_count) =
        metric_value_any(metrics, &["kernel_launches", "launch_count", "launches"])
    else {
        return Vec::new();
    };
    let has_single = optimization_passes_contain(case, "single-dispatch-launch-plan");
    let has_multi = optimization_passes_contain(case, "multi-dispatch-launch-plan");
    let mut issues = Vec::new();
    if launch_count == 1.0 {
        if !has_single {
            issues.push(LaunchPlanLabelIssue::MissingSingle);
        }
        if has_multi {
            issues.push(LaunchPlanLabelIssue::SingleHasMulti);
        }
    } else if launch_count > 1.0 {
        if !has_multi {
            issues.push(LaunchPlanLabelIssue::MissingMulti { launch_count });
        }
        if has_single {
            issues.push(LaunchPlanLabelIssue::MultiHasSingle { launch_count });
        }
    }
    issues
}

fn metric_value_any(metrics: Option<&Map<String, Value>>, fields: &[&str]) -> Option<f64> {
    let metrics = metrics?;
    fields
        .iter()
        .filter_map(|field| metrics.get(*field))
        .find_map(metric_value)
}

fn metric_value(metric: &Value) -> Option<f64> {
    metric
        .get("p50")
        .and_then(Value::as_f64)
        .or_else(|| {
            metric
                .get("p50")
                .and_then(Value::as_u64)
                .map(|value| value as f64)
        })
        .or_else(|| metric.as_f64())
        .or_else(|| metric.as_u64().map(|value| value as f64))
}

fn optimization_passes_contain(case: &Value, expected: &str) -> bool {
    ["optimization_passes_applied", "optimization_passes"]
        .iter()
        .any(|field| {
            case.get(*field)
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|item| item == expected)
                })
        })
}

fn case_id(case: &Value) -> String {
    case.get("id")
        .and_then(Value::as_str)
        .unwrap_or("<unknown>")
        .to_string()
}

fn suite_family_case_pairs(suite: &Value) -> BTreeSet<(String, String)> {
    suite_family_case_pair_counts(suite).into_keys().collect()
}

fn suite_family_case_pair_counts(suite: &Value) -> BTreeMap<(String, String), usize> {
    let mut counts = BTreeMap::new();
    suite
        .get("artifact_statuses")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|status| {
            let family_id = status
                .get("family_id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())?;
            let requested_case_id = status
                .get("requested_case_id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())?;
            Some((family_id.to_string(), requested_case_id.to_string()))
        })
        .for_each(|pair| {
            *counts.entry(pair).or_insert(0) += 1;
        });
    counts
}

fn suite_statuses_by_family_case_pair(suite: &Value) -> BTreeMap<(String, String), &Value> {
    suite
        .get("artifact_statuses")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|status| {
            let family_id = status
                .get("family_id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())?;
            let requested_case_id = status
                .get("requested_case_id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())?;
            Some((
                (family_id.to_string(), requested_case_id.to_string()),
                status,
            ))
        })
        .collect()
}

fn suite_artifact_status_count(suite: &Value) -> usize {
    suite_array_len(suite, "artifact_statuses")
}

fn suite_array_len(suite: &Value, field: &str) -> usize {
    suite
        .get(field)
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn suite_artifact_path_counts(suite: &Value) -> BTreeMap<String, usize> {
    suite
        .get("artifacts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(non_empty_str)
        .fold(BTreeMap::new(), |mut counts, path| {
            *counts.entry(path.to_string()).or_default() += 1;
            counts
        })
}

fn suite_status_path_counts(suite: &Value) -> BTreeMap<String, usize> {
    suite
        .get("artifact_statuses")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|status| status.get("path").and_then(non_empty_str))
        .fold(BTreeMap::new(), |mut counts, path| {
            *counts.entry(path.to_string()).or_default() += 1;
            counts
        })
}

fn suite_status_family_counts(suite: &Value) -> BTreeMap<String, usize> {
    suite
        .get("artifact_statuses")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|status| status.get("family_id").and_then(non_empty_str))
        .fold(BTreeMap::new(), |mut counts, family_id| {
            *counts.entry(family_id.to_string()).or_default() += 1;
            counts
        })
}

fn suite_all_artifact_paths(suite: &Value) -> BTreeSet<String> {
    suite_artifact_path_counts(suite)
        .into_keys()
        .chain(suite_status_path_counts(suite).into_keys())
        .collect()
}

fn non_empty_str(value: &Value) -> Option<&str> {
    value.as_str().filter(|value| !value.trim().is_empty())
}

fn is_blake3_hex_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn current_test_source_fingerprint(workspace_root: &Path) -> String {
        let git = vyre_bench::probes::capture_git_info_at(workspace_root);
        vyre_bench::probes::source_fingerprint(&git)
    }

    #[test]
    fn failed_case_summary_rejects_pass_status_with_invalid_correctness() {
        let case = serde_json::json!({
            "id": "release.condition_eval.1m",
            "status": "pass",
            "correctness": {
                "Invalid": {
                    "reason": "dispatch output mismatch at row 17"
                }
            },
            "performance": {"contract_passed": true}
        });

        assert_eq!(
            benchmark_case_failure_reason(&case),
            Some("dispatch output mismatch at row 17".to_string()),
            "Fix: explicit invalid correctness evidence must not be hidden by a contradictory pass status."
        );
    }

    #[test]
    fn failed_case_summary_rejects_invalid_correctness_with_blank_reason() {
        let case = serde_json::json!({
            "id": "release.condition_eval.1m",
            "status": "pass",
            "correctness": {
                "Invalid": {
                    "reason": "   "
                }
            },
            "performance": {"contract_passed": true}
        });

        assert_eq!(
            benchmark_case_failure_reason(&case),
            Some("invalid correctness".to_string()),
            "Fix: blank invalid-correctness reasons must not let contradictory pass status prove release correctness."
        );
        assert!(
            !benchmark_case_passes_summary_evidence(&case),
            "Fix: invalid correctness must disqualify case summary evidence even when the reason is blank."
        );
    }

    #[test]
    fn failed_case_summary_rejects_pass_status_with_performance_violations() {
        let case = serde_json::json!({
            "id": "release.condition_eval.1m",
            "status": "pass",
            "correctness": {"Valid": {}},
            "performance": {
                "contract_passed": true,
                "violations": [
                    "speedup below CUDA release floor",
                    "p95 latency regression"
                ]
            }
        });

        assert_eq!(
            benchmark_case_failure_reason(&case),
            Some("speedup below CUDA release floor; p95 latency regression".to_string()),
            "Fix: performance violation evidence must stay visible even when status is pass."
        );
    }

    #[test]
    fn failed_case_summary_reports_contract_failed_pass_as_contract_failure() {
        let case = serde_json::json!({
            "id": "release.condition_eval.1m",
            "status": "pass",
            "correctness": {"Valid": {}},
            "performance": {"contract_passed": false}
        });

        assert_eq!(
            benchmark_case_failure_reason(&case),
            Some("performance contract failed".to_string()),
            "Fix: contradictory pass status must not hide contract_passed=false evidence."
        );
    }

    #[test]
    fn failed_case_summary_rejects_missing_pass_status() {
        let case = serde_json::json!({
            "id": "release.condition_eval.1m",
            "correctness": {"Valid": {}},
            "performance": {"contract_passed": true}
        });

        assert_eq!(
            benchmark_case_failure_reason(&case),
            Some("missing pass status".to_string()),
            "Fix: benchmark evidence must require an explicit pass status before a case can prove release performance."
        );
    }

    #[test]
    fn benchmark_report_summary_mismatch_reports_total_pass_fail_drift() {
        let report = serde_json::json!({
            "summary": {"total_cases": 2, "passed": 0, "failed": 0},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "status": "pass",
                    "correctness": {"Valid": {}},
                    "performance": {"contract_passed": true}
                }
            ]
        });

        assert_eq!(
            benchmark_report_summary_case_evidence_mismatch(&report),
            Some(
                "summary total/pass/fail (Some(2)/Some(0)/Some(0)) contradicts case evidence (1/1/0)"
                    .to_string()
            ),
            "Fix: benchmark summary validation must expose stale total_cases and passed counts, not only summary.failed."
        );
        assert!(
            !benchmark_report_summary_matches_case_evidence(&report),
            "Fix: stale benchmark summaries must not be accepted by boolean reuse/gate predicates."
        );
    }

    #[test]
    fn launch_plan_issues_reject_single_label_for_multi_launch_count() {
        let case = serde_json::json!({
            "optimization_passes_applied": ["single-dispatch-launch-plan"],
            "metrics": {
                "kernel_launches": {"p50": 4, "samples": 30}
            }
        });
        let issues =
            launch_plan_label_issues(&case, case.get("metrics").and_then(Value::as_object));

        assert_eq!(
            issues,
            vec![
                LaunchPlanLabelIssue::MissingMulti { launch_count: 4.0 },
                LaunchPlanLabelIssue::MultiHasSingle { launch_count: 4.0 },
            ],
            "Fix: multi-launch evidence must require the multi label and reject the single label."
        );
    }

    #[test]
    fn launch_plan_issues_accept_matching_single_and_multi_counts() {
        for case in [
            serde_json::json!({
                "optimization_passes_applied": ["single-dispatch-launch-plan"],
                "metrics": {"kernel_launches": {"p50": 1, "samples": 30}}
            }),
            serde_json::json!({
                "optimization_passes_applied": ["multi-dispatch-launch-plan"],
                "metrics": {"launch_count": 4}
            }),
        ] {
            let issues =
                launch_plan_label_issues(&case, case.get("metrics").and_then(Value::as_object));
            assert!(
                issues.is_empty(),
                "Fix: matching launch-plan label/count evidence should pass: {issues:?}"
            );
        }
    }

    #[test]
    fn source_fingerprint_rejects_dirty_without_worktree_digest() {
        assert_eq!(
            source_fingerprint_issues("git:abc123:dirty=true"),
            vec![SourceFingerprintIssue::DirtyMissingWorktree {
                source_fingerprint: "git:abc123:dirty=true".to_string(),
            }],
            "Fix: dirty benchmark evidence must not collapse distinct dirty worktrees."
        );
    }

    #[test]
    fn source_fingerprint_rejects_unknown_dirty_state_and_digest() {
        assert_eq!(
            source_fingerprint_issues("git:abc123:dirty=unknown"),
            vec![SourceFingerprintIssue::DirtyUnknownState {
                source_fingerprint: "git:abc123:dirty=unknown".to_string(),
            }],
            "Fix: release evidence must fail closed when git status provenance is unavailable."
        );
        assert_eq!(
            source_fingerprint_issues("git:abc123:dirty=true:worktree=unknown"),
            vec![SourceFingerprintIssue::DirtyUnknownWorktree {
                source_fingerprint: "git:abc123:dirty=true:worktree=unknown".to_string(),
            }],
            "Fix: dirty source fingerprints must carry the actual worktree digest."
        );
    }

    #[test]
    fn source_fingerprint_accepts_clean_and_precise_dirty_git_fingerprints() {
        assert!(source_fingerprint_issues("git:abc123:dirty=false").is_empty());
        assert!(
            source_fingerprint_issues(&format!(
                "git:abc123:dirty=true:worktree={}",
                "a".repeat(64)
            ))
            .is_empty(),
            "Fix: precise dirty source fingerprints must remain valid release evidence."
        );
    }

    #[test]
    fn source_fingerprint_freshness_rejects_non_current_evidence() {
        assert_eq!(
            source_fingerprint_freshness_issues(
                "git:old:dirty=false",
                "git:new:dirty=true:worktree=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ),
            vec![SourceFingerprintFreshnessIssue::Mismatch {
                source_fingerprint: "git:old:dirty=false".to_string(),
                current_source_fingerprint:
                    "git:new:dirty=true:worktree=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        .to_string(),
            }],
            "Fix: release evidence must be regenerated after source changes, not carried forward by matching old artifact metadata."
        );
    }

    #[test]
    fn source_fingerprint_freshness_accepts_current_evidence() {
        let fingerprint =
            "git:abc:dirty=true:worktree=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        assert!(
            source_fingerprint_freshness_issues(fingerprint, fingerprint).is_empty(),
            "Fix: current source evidence should not be rejected by the freshness gate."
        );
    }

    #[test]
    fn cuda_release_axes_reject_stale_and_weak_source_artifact_provenance() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for release axes provenance test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temp workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temp benchmark evidence directory.");
        let stale_artifact = "release/evidence/benchmarks/workload-stale.json";
        std::fs::write(
            dir.path().join(stale_artifact),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "source_tree_fingerprint": "source-tree-v1:stale",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [{"id": "release.stale", "status": "pass"}]
            }))
            .expect("Fix: serialize stale source artifact."),
        )
        .expect("Fix: write stale source artifact.");
        let weak_artifact = "release/evidence/benchmarks/workload-weak.json";
        std::fs::write(
            dir.path().join(weak_artifact),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "source_fingerprint": "git:abc123:dirty=true",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [{"id": "release.weak", "status": "pass"}]
            }))
            .expect("Fix: serialize weak source artifact."),
        )
        .expect("Fix: write weak source artifact.");
        let axes = serde_json::json!({
            "source_artifacts": [stale_artifact, weak_artifact]
        });
        let cuda_suite = serde_json::json!({
            "artifacts": [stale_artifact, weak_artifact]
        });

        let issues = cuda_release_axes_source_artifact_issues(dir.path(), &axes, &cuda_suite);

        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/workload-stale.json` source_tree_fingerprint `source-tree-v1:stale` does not match current workspace source"
            )),
            "Fix: release-axis source artifacts must be fresh against the current workspace; issues={issues:?}"
        );
        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/workload-stale.json` has no source_fingerprint"
            )),
            "Fix: release-axis source artifacts must preserve explicit source_fingerprint provenance; issues={issues:?}"
        );
        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/workload-weak.json` source_fingerprint `git:abc123:dirty=true` is dirty but has no worktree digest"
            )),
            "Fix: release-axis source artifacts must reject legacy dirty source fingerprints; issues={issues:?}"
        );
        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/workload-weak.json` has no source_tree_fingerprint"
            )),
            "Fix: release-axis source artifacts must preserve source_tree_fingerprint provenance; issues={issues:?}"
        );
    }

    #[test]
    fn cpu_sota_100x_source_artifacts_reject_weak_and_stale_provenance() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for CPU-SOTA source artifact test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temp workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temp benchmark evidence directory.");
        let aggregate_source_fingerprint = current_test_source_fingerprint(dir.path());
        let aggregate_source_tree_fingerprint =
            vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let weak_artifact = "release/evidence/benchmarks/cuda-weak-source.json";
        std::fs::write(
            dir.path().join(weak_artifact),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "source_fingerprint": "git:abc123:dirty=true",
                "source_tree_fingerprint": &aggregate_source_tree_fingerprint,
                "summary": {"total_cases": 0, "passed": 0, "failed": 0},
                "cases": []
            }))
            .expect("Fix: serialize weak CPU-SOTA source artifact."),
        )
        .expect("Fix: write weak CPU-SOTA source artifact.");
        let stale_artifact = "release/evidence/benchmarks/cuda-stale-source-tree.json";
        std::fs::write(
            dir.path().join(stale_artifact),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "source_fingerprint": &aggregate_source_fingerprint,
                "source_tree_fingerprint": "source-tree-v1:stale",
                "summary": {"total_cases": 0, "passed": 0, "failed": 0},
                "cases": []
            }))
            .expect("Fix: serialize stale CPU-SOTA source artifact."),
        )
        .expect("Fix: write stale CPU-SOTA source artifact.");
        let proof = serde_json::json!({
            "source_fingerprint": aggregate_source_fingerprint,
            "source_tree_fingerprint": aggregate_source_tree_fingerprint,
            "source_artifacts": [weak_artifact, stale_artifact]
        });

        let issues = cpu_sota_100x_source_artifact_issues(dir.path(), &proof);

        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/cuda-weak-source.json` source_fingerprint `git:abc123:dirty=true` is dirty but has no worktree digest"
            )),
            "Fix: CPU-SOTA aggregate source artifacts must reject weak dirty source_fingerprint provenance; issues={issues:?}"
        );
        assert!(
            !issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/cuda-weak-source.json` source_fingerprint `git:abc123:dirty=true` does not match aggregate source"
            )),
            "Fix: CPU-SOTA aggregate source artifacts must rely on source_tree_fingerprint for source identity instead of raw evidence commit equality; issues={issues:?}"
        );
        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/cuda-stale-source-tree.json` source_tree_fingerprint `source-tree-v1:stale` does not match aggregate source tree"
            )),
            "Fix: CPU-SOTA aggregate source artifacts must match the aggregate source tree fingerprint; issues={issues:?}"
        );
        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/cuda-stale-source-tree.json` source_tree_fingerprint `source-tree-v1:stale` does not match current workspace source"
            )),
            "Fix: CPU-SOTA aggregate source artifacts must be fresh against the current workspace; issues={issues:?}"
        );
    }

    #[test]
    fn cpu_sota_100x_source_artifacts_reject_backend_drift_and_borrowed_cuda_telemetry() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for CPU-SOTA backend drift test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temp workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temp benchmark evidence directory.");
        let aggregate_source_fingerprint = current_test_source_fingerprint(dir.path());
        let aggregate_source_tree_fingerprint =
            vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let artifact = "release/evidence/benchmarks/cuda-cpu-sota-drift.json";
        std::fs::write(
            dir.path().join(artifact),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "source_fingerprint": &aggregate_source_fingerprint,
                "source_tree_fingerprint": &aggregate_source_tree_fingerprint,
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.cpu-sota-drift",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "optimization_passes_applied": ["cuda-resident-borrowed-escape-hatch"],
                        "metrics": {
                            "cuda_resident_borrowed_fallback_dispatches": {"p50": 3.0}
                        }
                    }
                ]
            }))
            .expect("Fix: serialize drifted CPU-SOTA source artifact."),
        )
        .expect("Fix: write drifted CPU-SOTA source artifact.");
        let proof = serde_json::json!({
            "source_fingerprint": aggregate_source_fingerprint,
            "source_tree_fingerprint": aggregate_source_tree_fingerprint,
            "source_artifacts": [artifact]
        });

        let issues = cpu_sota_100x_source_artifact_issues(dir.path(), &proof);

        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/cuda-cpu-sota-drift.json` case `release.cpu-sota-drift` backend_id `wgpu` does not match selected_backend `cuda`"
            )),
            "Fix: CPU-SOTA aggregate source artifacts must reject case-level backend drift before proof counts can imply CUDA coverage; issues={issues:?}"
        );
        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/cuda-cpu-sota-drift.json` case `release.cpu-sota-drift` has cuda_resident_borrowed_fallback_dispatches p50=3"
            ) && issue.contains("CPU-SOTA aggregate proof must use native resident dispatch")),
            "Fix: CPU-SOTA aggregate source artifacts must reject borrowed resident CUDA dispatch evidence; issues={issues:?}"
        );
    }

    #[test]
    fn cpu_sota_100x_source_artifacts_reject_wrong_backend_contracts() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for CPU-SOTA contract backend test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temp workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temp benchmark evidence directory.");
        let aggregate_source_fingerprint = current_test_source_fingerprint(dir.path());
        let aggregate_source_tree_fingerprint =
            vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let artifact = "release/evidence/benchmarks/cuda-wrong-contract.json";
        std::fs::write(
            dir.path().join(artifact),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "source_fingerprint": &aggregate_source_fingerprint,
                "source_tree_fingerprint": &aggregate_source_tree_fingerprint,
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.cpu-sota-wrong-contract",
                        "backend_id": "cuda",
                        "status": "pass",
                        "contract": {
                            "baselines": [
                                {
                                    "class": "CpuSota",
                                    "backend_ids": ["wgpu"],
                                    "min_speedup_x": 100.0
                                }
                            ]
                        },
                        "metrics": {
                            "wall_ns": {"p50": 10},
                            "baseline_wall_ns": {"p50": 2000}
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize wrong-contract CPU-SOTA source artifact."),
        )
        .expect("Fix: write wrong-contract CPU-SOTA source artifact.");
        let proof = serde_json::json!({
            "source_fingerprint": aggregate_source_fingerprint,
            "source_tree_fingerprint": aggregate_source_tree_fingerprint,
            "source_artifacts": [artifact]
        });

        let issues = cpu_sota_100x_source_artifact_issues(dir.path(), &proof);

        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/cuda-wrong-contract.json` case `release.cpu-sota-wrong-contract` backend `cuda` has no applicable performance contract baseline"
            )),
            "Fix: CPU-SOTA aggregate source artifacts must reject CUDA cases whose performance contract only applies to WGPU; issues={issues:?}"
        );
    }

    #[test]
    fn cuda_release_axes_reject_case_backend_drift_inside_cuda_source_artifact() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for release axes backend drift test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temp workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temp benchmark evidence directory.");
        let source_fingerprint = current_test_source_fingerprint(dir.path());
        let source_tree_fingerprint = vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let artifact = "release/evidence/benchmarks/workload-backend-drift.json";
        std::fs::write(
            dir.path().join(artifact),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "source_fingerprint": source_fingerprint,
                "source_tree_fingerprint": source_tree_fingerprint,
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.backend-drift",
                        "backend_id": "wgpu",
                        "status": "pass"
                    }
                ]
            }))
            .expect("Fix: serialize backend drift source artifact."),
        )
        .expect("Fix: write backend drift source artifact.");
        let axes = serde_json::json!({
            "source_artifacts": [artifact]
        });
        let cuda_suite = serde_json::json!({
            "backend": "cuda",
            "artifacts": [artifact]
        });

        let issues = cuda_release_axes_source_artifact_issues(dir.path(), &axes, &cuda_suite);

        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/workload-backend-drift.json` case `release.backend-drift` backend_id `wgpu` does not match selected_backend `cuda`"
            )),
            "Fix: release-axis CUDA source artifact validation must reject case-level backend drift, not only artifact-level selected_backend; issues={issues:?}"
        );
    }

    #[test]
    fn cuda_release_axes_reject_borrowed_resident_fallback_source_artifact() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for release axes CUDA telemetry test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temp workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temp benchmark evidence directory.");
        let source_fingerprint = current_test_source_fingerprint(dir.path());
        let source_tree_fingerprint = vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let artifact = "release/evidence/benchmarks/workload-borrowed-resident.json";
        std::fs::write(
            dir.path().join(artifact),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "source_fingerprint": source_fingerprint,
                "source_tree_fingerprint": source_tree_fingerprint,
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.borrowed-resident",
                        "backend_id": "cuda",
                        "status": "pass",
                        "optimization_passes_applied": ["cuda-resident-borrowed-escape-hatch"],
                        "metrics": {
                            "wall_ns": {"p50": 17_000},
                            "cold_compile_ns": {"p50": 2_000_000},
                            "wall_gb_s_x1000": {"p50": 4_000},
                            "memory_total_mib": {"p50": 24_576},
                            "cuda_resident_borrowed_fallback_dispatches": {"p50": 2.0}
                        }
                    }
                ]
            }))
            .expect("Fix: serialize borrowed resident source artifact."),
        )
        .expect("Fix: write borrowed resident source artifact.");
        let axes = serde_json::json!({
            "warm_us_per_file": 17.0,
            "cold_pipeline_build_ms": 2.0,
            "gbs_scan_throughput": 4.0,
            "ulp_drift_max": 0,
            "max_vram_mib": 24_576,
            "source_artifacts": [artifact]
        });
        let cuda_suite = serde_json::json!({
            "backend": "cuda",
            "artifacts": [artifact]
        });

        let issues = cuda_release_axes_source_artifact_issues(dir.path(), &axes, &cuda_suite);

        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/workload-borrowed-resident.json` case `release.borrowed-resident` has cuda_resident_borrowed_fallback_dispatches p50=2"
            ) && issue.contains("canonical CUDA release axes must use native resident dispatch")),
            "Fix: canonical CUDA release axes must reject source artifacts measured through the borrowed resident fallback escape hatch; issues={issues:?}"
        );
    }

    #[test]
    fn cuda_release_axes_reject_source_artifacts_missing_axis_metrics() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for release axes metric test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temp workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temp benchmark evidence directory.");
        let source_fingerprint = current_test_source_fingerprint(dir.path());
        let source_tree_fingerprint = vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let artifact = "release/evidence/benchmarks/workload-missing-axis-metrics.json";
        std::fs::write(
            dir.path().join(artifact),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "source_fingerprint": source_fingerprint,
                "source_tree_fingerprint": source_tree_fingerprint,
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.missing-axis-metrics",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"p50": 10}
                        }
                    }
                ]
            }))
            .expect("Fix: serialize missing metric source artifact."),
        )
        .expect("Fix: write missing metric source artifact.");
        let axes = serde_json::json!({
            "source_artifacts": [artifact]
        });
        let cuda_suite = serde_json::json!({
            "backend": "cuda",
            "artifacts": [artifact]
        });

        let issues = cuda_release_axes_source_artifact_issues(dir.path(), &axes, &cuda_suite);

        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/workload-missing-axis-metrics.json` has no positive p50 cold/compile metric for cold_pipeline_build_ms"
            )),
            "Fix: release-axis source artifacts must individually prove cold/compile metrics, not rely on another artifact; issues={issues:?}"
        );
        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/workload-missing-axis-metrics.json` has no positive p50 throughput metric for gbs_scan_throughput"
            )),
            "Fix: release-axis source artifacts must individually prove throughput metrics, not rely on another artifact; issues={issues:?}"
        );
        assert!(
            issues.iter().any(|issue| issue.contains(
                "source_artifact `release/evidence/benchmarks/workload-missing-axis-metrics.json` has no GPU memory_total_mib evidence for max_vram_mib"
            )),
            "Fix: release-axis source artifacts must individually prove GPU memory evidence, not rely on another artifact; issues={issues:?}"
        );
    }

    #[test]
    fn cuda_release_axes_require_scalar_axes_from_source_artifacts() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for release axes scalar presence test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temp workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temp benchmark evidence directory.");
        let source_fingerprint = current_test_source_fingerprint(dir.path());
        let source_tree_fingerprint = vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let mut artifacts = Vec::new();
        for index in 1..=12 {
            let artifact = format!("release/evidence/benchmarks/workload-{index:02}.json");
            std::fs::write(
                dir.path().join(&artifact),
                serde_json::to_string_pretty(&serde_json::json!({
                    "selected_backend": "cuda",
                    "source_fingerprint": &source_fingerprint,
                    "source_tree_fingerprint": &source_tree_fingerprint,
                    "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                    "environment": {
                        "gpu_devices": [{"memory_total_mib": 24576}]
                    },
                    "cases": [
                        {
                            "id": format!("release.scalar-required.{index}"),
                            "backend_id": "cuda",
                            "status": "pass",
                            "metrics": {
                                "wall_ns": {"p50": 17_000},
                                "cold_compile_ns": {"p50": 2_000_000},
                                "wall_gb_s_x1000": {"p50": 4_000}
                            },
                            "correctness": {
                                "Toleranced": {"max_observed_ulp": 3}
                            }
                        }
                    ]
                }))
                .expect("Fix: serialize scalar presence source artifact."),
            )
            .expect("Fix: write scalar presence source artifact.");
            artifacts.push(artifact);
        }
        let axes = serde_json::json!({
            "source_artifacts": artifacts
        });
        let cuda_suite = serde_json::json!({
            "backend": "cuda",
            "artifacts": artifacts
        });

        let issues = cuda_release_axes_source_artifact_issues(dir.path(), &axes, &cuda_suite);

        for axis in [
            "warm_us_per_file",
            "cold_pipeline_build_ms",
            "gbs_scan_throughput",
            "ulp_drift_max",
            "max_vram_mib",
        ] {
            assert!(
                issues.iter().any(|issue| issue.contains(&format!(
                    "bench-release-axes {axis} is missing or not numeric"
                ))),
                "Fix: release axes must require scalar `{axis}` once source artifacts prove it; issues={issues:?}"
            );
        }
    }

    #[test]
    fn cuda_release_axes_reject_axis_values_that_drift_from_source_artifacts() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for release axes scalar drift test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temp workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temp benchmark evidence directory.");
        let source_fingerprint = current_test_source_fingerprint(dir.path());
        let source_tree_fingerprint = vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let mut artifacts = Vec::new();
        for index in 1..=12 {
            let artifact = format!("release/evidence/benchmarks/workload-{index:02}.json");
            std::fs::write(
                dir.path().join(&artifact),
                serde_json::to_string_pretty(&serde_json::json!({
                    "selected_backend": "cuda",
                    "source_fingerprint": &source_fingerprint,
                    "source_tree_fingerprint": &source_tree_fingerprint,
                    "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                    "environment": {
                        "gpu_devices": [{"memory_total_mib": 24576}]
                    },
                    "cases": [
                        {
                            "id": format!("release.scalar-drift.{index}"),
                            "backend_id": "cuda",
                            "status": "pass",
                            "metrics": {
                                "wall_ns": {"p50": 17_000},
                                "cold_compile_ns": {"p50": 2_000_000},
                                "wall_gb_s_x1000": {"p50": 4_000}
                            },
                            "correctness": {
                                "Toleranced": {"max_observed_ulp": 0}
                            }
                        }
                    ]
                }))
                .expect("Fix: serialize scalar drift source artifact."),
            )
            .expect("Fix: write scalar drift source artifact.");
            artifacts.push(artifact);
        }
        let axes = serde_json::json!({
            "warm_us_per_file": 17.0,
            "cold_pipeline_build_ms": 2.0,
            "gbs_scan_throughput": 999.0,
            "ulp_drift_max": 0,
            "max_vram_mib": 24576,
            "source_artifacts": artifacts
        });
        let cuda_suite = serde_json::json!({
            "backend": "cuda",
            "artifacts": artifacts
        });

        let issues = cuda_release_axes_source_artifact_issues(dir.path(), &axes, &cuda_suite);

        assert!(
            issues.iter().any(|issue| issue.contains(
                "bench-release-axes gbs_scan_throughput=999 does not match source artifacts 4"
            )),
            "Fix: release-axis scalar values must be recomputed from source artifacts instead of trusting stale axes JSON; issues={issues:?}"
        );
    }

    #[test]
    fn cuda_release_axes_reject_suite_status_inventory_drift() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for release axes suite inventory test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temp workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temp benchmark evidence directory.");
        let source_fingerprint = current_test_source_fingerprint(dir.path());
        let source_tree_fingerprint = vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let mut artifacts = Vec::new();
        for index in 1..=12 {
            let artifact = format!("release/evidence/benchmarks/workload-{index:02}.json");
            std::fs::write(
                dir.path().join(&artifact),
                serde_json::to_string_pretty(&serde_json::json!({
                    "selected_backend": "cuda",
                    "source_fingerprint": &source_fingerprint,
                    "source_tree_fingerprint": &source_tree_fingerprint,
                    "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                    "environment": {
                        "gpu_devices": [{"memory_total_mib": 24576}]
                    },
                    "cases": [
                        {
                            "id": format!("release.inventory-drift.{index}"),
                            "backend_id": "cuda",
                            "status": "pass",
                            "metrics": {
                                "wall_ns": {"p50": 17_000},
                                "cold_compile_ns": {"p50": 2_000_000},
                                "wall_gb_s_x1000": {"p50": 4_000}
                            },
                            "correctness": {
                                "Toleranced": {"max_observed_ulp": 0}
                            }
                        }
                    ]
                }))
                .expect("Fix: serialize suite inventory source artifact."),
            )
            .expect("Fix: write suite inventory source artifact.");
            artifacts.push(artifact);
        }
        let mut status_artifacts = artifacts.clone();
        status_artifacts[11] = "release/evidence/benchmarks/wgpu-workload-12.json".to_string();
        let artifact_statuses = status_artifacts
            .iter()
            .enumerate()
            .map(|(index, artifact)| {
                serde_json::json!({
                    "path": artifact,
                    "family_id": format!("family-{index:02}"),
                    "requested_case_id": format!("release.inventory-drift.{index}")
                })
            })
            .collect::<Vec<_>>();
        let axes = serde_json::json!({
            "warm_us_per_file": 17.0,
            "cold_pipeline_build_ms": 2.0,
            "gbs_scan_throughput": 4.0,
            "ulp_drift_max": 0,
            "max_vram_mib": 24576,
            "source_artifacts": artifacts.clone()
        });
        let cuda_suite = serde_json::json!({
            "backend": "cuda",
            "artifacts": artifacts,
            "artifact_statuses": artifact_statuses
        });

        let issues = cuda_release_axes_source_artifact_issues(dir.path(), &axes, &cuda_suite);

        assert!(
            issues.iter().any(|issue| issue.contains(
                "cuda-release-suite lists artifact `release/evidence/benchmarks/workload-12.json` without matching artifact_statuses entry"
            )),
            "Fix: release axes must reject CUDA suite artifacts that lack matching status rows; issues={issues:?}"
        );
        assert!(
            issues.iter().any(|issue| issue.contains(
                "cuda-release-suite has artifact_statuses path `release/evidence/benchmarks/wgpu-workload-12.json` absent from artifacts"
            )),
            "Fix: release axes must reject stale or cross-backend suite status rows before bench-release consumes clean axes; issues={issues:?}"
        );
    }

    #[test]
    fn current_source_fingerprint_resolves_from_release_evidence_path() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for evidence source fingerprint test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temp workspace manifest.");
        std::fs::create_dir_all(dir.path().join("release/evidence/benchmarks"))
            .expect("Fix: create temp release evidence directory.");
        let evidence = dir.path().join("release/evidence/benchmarks/workload.json");

        let fingerprint = current_source_fingerprint_for_evidence_path(&evidence)
            .expect("Fix: resolve workspace source fingerprint from nested release evidence path.");

        assert!(
            fingerprint.starts_with("crate:"),
            "Fix: non-git test workspaces should still produce deterministic crate source provenance, got {fingerprint}."
        );
    }

    #[test]
    fn report_freshness_fingerprint_prefers_source_tree_scope() {
        let report = serde_json::json!({
            "source_fingerprint": "git:abc:dirty=false",
            "source_tree_fingerprint": "source-tree-v1:def",
        });

        assert_eq!(
            report_freshness_fingerprint(&report),
            Some(("source_tree_fingerprint", "source-tree-v1:def")),
            "Fix: current-source gates must prefer evidence-stable source tree provenance over commit-shaped legacy provenance."
        );
    }

    #[test]
    fn source_fingerprint_rejects_malformed_dirty_worktree_digest() {
        assert_eq!(
            source_fingerprint_issues("git:abc123:dirty=true:worktree=not-a-digest"),
            vec![SourceFingerprintIssue::DirtyInvalidWorktree {
                source_fingerprint: "git:abc123:dirty=true:worktree=not-a-digest".to_string(),
                worktree: "not-a-digest".to_string(),
            }],
            "Fix: dirty source fingerprints must carry a stable 64-hex digest."
        );
    }

    #[test]
    fn benchmark_source_provenance_rejects_artifact_paths_without_source_fingerprint() {
        let report = serde_json::json!({
            "source_artifacts": ["release/evidence/benchmarks/cuda.json"]
        });

        assert!(
            !benchmark_report_has_source_provenance(&report),
            "Fix: source_artifact paths identify evidence inputs; they must not satisfy benchmark source provenance without source_fingerprint."
        );
    }

    #[test]
    fn benchmark_source_provenance_rejects_git_commit_without_source_fingerprint() {
        assert!(
            !benchmark_report_has_source_provenance(&serde_json::json!({
                "git": {"commit": "abcdef"}
            })),
            "Fix: git.commit metadata is not a freshness-checked source_fingerprint and must not satisfy benchmark source provenance."
        );
    }

    #[test]
    fn benchmark_source_provenance_accepts_explicit_source_fingerprint() {
        assert!(
            benchmark_report_has_source_provenance(&serde_json::json!({
                "source_fingerprint": "git:0123456789abcdef0123456789abcdef01234567:dirty=false",
                "source_artifacts": ["release/evidence/benchmarks/cuda.json"],
                "git": {"commit": "abcdef"}
            })),
            "Fix: explicit source_fingerprint must satisfy benchmark source provenance."
        );
    }

    #[test]
    fn benchmark_source_artifact_count_ignores_blank_entries() {
        let report = serde_json::json!({
            "source_artifacts": [
                "",
                null,
                "release/evidence/benchmarks/cuda-a.json",
                "   ",
                "release/evidence/benchmarks/cuda-a.json",
                "release/evidence/benchmarks/cuda-b.json"
            ]
        });

        assert_eq!(
            benchmark_source_artifact_count(&report),
            2,
            "Fix: source_artifact counts must count only unique usable non-empty string entries."
        );
        assert_eq!(
            benchmark_source_artifact_paths(&report),
            BTreeSet::from([
                "release/evidence/benchmarks/cuda-a.json".to_string(),
                "release/evidence/benchmarks/cuda-b.json".to_string(),
            ]),
            "Fix: source_artifact path extraction must expose the same unique usable paths used by release gates."
        );
    }

    #[test]
    fn benchmark_source_artifact_path_rejects_absolute_existing_file() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for source artifact path test.");
        let artifact = dir.path().join("release/evidence/benchmarks/source.json");
        std::fs::create_dir_all(
            artifact
                .parent()
                .expect("Fix: source artifact fixture must have a parent directory."),
        )
        .expect("Fix: create temporary source artifact directory.");
        std::fs::write(&artifact, "{}").expect("Fix: write source artifact fixture.");

        assert_eq!(
            benchmark_source_artifact_path_issue(dir.path(), &artifact.display().to_string()),
            Some(BenchmarkArtifactPathIssue::AbsolutePath),
            "Fix: existing absolute source_artifact paths must not pass release evidence validation."
        );
    }

    #[test]
    fn benchmark_source_artifact_path_rejects_parent_traversal() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for source artifact traversal test.");

        assert_eq!(
            benchmark_source_artifact_path_issue(
                dir.path(),
                "release/evidence/benchmarks/../../Cargo.toml"
            ),
            Some(BenchmarkArtifactPathIssue::ParentTraversal),
            "Fix: source_artifact validation must reject parent traversal before resolving files."
        );
    }

    #[test]
    fn benchmark_source_artifact_path_rejects_non_release_relative_path() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for non-release artifact path test.");

        assert_eq!(
            benchmark_source_artifact_path_issue(dir.path(), "evidence/benchmarks/source.json"),
            Some(BenchmarkArtifactPathIssue::NonReleasePath),
            "Fix: source_artifact validation must keep benchmark evidence references under release/."
        );
    }

    #[test]
    fn benchmark_source_artifact_path_accepts_release_file_inside_workspace() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for valid source artifact path test.");
        let artifact = dir.path().join("release/evidence/benchmarks/source.json");
        std::fs::create_dir_all(
            artifact
                .parent()
                .expect("Fix: source artifact fixture must have a parent directory."),
        )
        .expect("Fix: create temporary source artifact directory.");
        std::fs::write(&artifact, "{}").expect("Fix: write source artifact fixture.");

        assert_eq!(
            benchmark_source_artifact_path_issue(
                dir.path(),
                "release/evidence/benchmarks/source.json"
            ),
            None,
            "Fix: release/evidence source artifacts inside the workspace must remain valid."
        );
    }

    #[cfg(unix)]
    #[test]
    fn benchmark_source_artifact_path_rejects_symlink_escape() {
        let workspace = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for symlink source artifact test.");
        let outside = tempfile::TempDir::new()
            .expect("Fix: create external directory for symlink source artifact test.");
        let outside_artifact = outside.path().join("source.json");
        std::fs::write(&outside_artifact, "{}").expect("Fix: write external source artifact.");
        let link = workspace
            .path()
            .join("release/evidence/benchmarks/source.json");
        std::fs::create_dir_all(
            link.parent()
                .expect("Fix: symlink artifact fixture must have a parent directory."),
        )
        .expect("Fix: create temporary symlink source artifact directory.");
        std::os::unix::fs::symlink(&outside_artifact, &link)
            .expect("Fix: create source artifact symlink.");

        let Some(BenchmarkArtifactPathIssue::OutsideWorkspace { .. }) =
            benchmark_source_artifact_path_issue(
                workspace.path(),
                "release/evidence/benchmarks/source.json",
            )
        else {
            panic!("Fix: source_artifact validation must reject symlink escapes.");
        };
    }

    #[test]
    fn benchmark_duplicate_source_artifact_paths_report_repeated_usable_entries() {
        let report = serde_json::json!({
            "source_artifacts": [
                "",
                null,
                "release/evidence/benchmarks/cuda-a.json",
                "release/evidence/benchmarks/cuda-b.json",
                "release/evidence/benchmarks/cuda-a.json",
                "release/evidence/benchmarks/cuda-b.json",
                "release/evidence/benchmarks/cuda-c.json"
            ]
        });

        assert_eq!(
            benchmark_source_artifact_entry_count(&report),
            5,
            "Fix: raw source_artifact entry counts must ignore blank/non-string entries but preserve duplicate evidence attempts."
        );
        assert_eq!(
            benchmark_duplicate_source_artifact_paths(&report),
            BTreeSet::from([
                "release/evidence/benchmarks/cuda-a.json".to_string(),
                "release/evidence/benchmarks/cuda-b.json".to_string(),
            ]),
            "Fix: aggregate gates must identify duplicated source_artifact paths instead of letting them inflate proof counts."
        );
    }

    #[test]
    fn duplicate_nonblank_string_array_values_reports_repeated_entries() {
        let report = serde_json::json!({
            "cpu_sota_100x_contract_cases": [
                "release.condition_eval.1m",
                "release.entropy_window.1m",
                "release.condition_eval.1m",
                " ",
                null,
                "release.entropy_window.1m"
            ]
        });

        assert_eq!(
            duplicate_nonblank_string_array_values(&report, "cpu_sota_100x_contract_cases"),
            BTreeSet::from([
                "release.condition_eval.1m".to_string(),
                "release.entropy_window.1m".to_string(),
            ]),
            "Fix: release aggregate proof arrays must expose duplicate nonblank ids without counting blank placeholders."
        );
    }

    #[test]
    fn duplicate_nonblank_object_array_field_values_reports_repeated_entries() {
        let report = serde_json::json!({
            "families": [
                {"family": "algebraic"},
                {"family": "predicate"},
                {"family": "algebraic"},
                {"family": " "},
                {"family": null},
                {"family": "predicate"}
            ]
        });

        assert_eq!(
            duplicate_nonblank_object_array_field_values(&report, "families", "family"),
            BTreeSet::from(["algebraic".to_string(), "predicate".to_string()]),
            "Fix: release manifest object arrays must expose duplicate nonblank ids without counting blank placeholders."
        );
    }

    #[test]
    fn backend_consistency_rejects_case_backend_drift() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "cases": [
                {"id": "same", "backend_id": "cuda"},
                {"id": "fallback", "backend_id": "wgpu"},
                {"id": "missing"}
            ]
        });

        assert_eq!(
            backend_consistency_issues(&report),
            vec![
                BackendConsistencyIssue::CaseBackendMismatch {
                    case_id: "fallback".to_string(),
                    expected_backend: "cuda".to_string(),
                    actual_backend: "wgpu".to_string(),
                },
                BackendConsistencyIssue::MissingCaseBackend {
                    case_id: "missing".to_string(),
                    expected_backend: "cuda".to_string(),
                },
            ],
            "Fix: report-level backend selection must be proven by every benchmark case."
        );
    }

    #[test]
    fn backend_consistency_rejects_blank_case_identity() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "cases": [
                {"id": "   ", "backend_id": "cuda"},
                {"backend_id": "cuda"}
            ]
        });

        assert_eq!(
            backend_consistency_issues(&report),
            vec![
                BackendConsistencyIssue::MissingCaseId { case_index: 0 },
                BackendConsistencyIssue::MissingCaseId { case_index: 1 },
            ],
            "Fix: backend consistency must require nonblank case ids before benchmark rows can prove release backend identity."
        );
    }

    #[test]
    fn backend_consistency_rejects_duplicate_case_identity() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "cases": [
                {"id": "release.condition_eval.1m", "backend_id": "cuda"},
                {"id": "release.condition_eval.1m", "backend_id": "cuda"},
                {"id": "release.entropy_window.1m", "backend_id": "cuda"}
            ]
        });

        assert_eq!(
            backend_consistency_issues(&report),
            vec![BackendConsistencyIssue::DuplicateCaseId {
                case_id: "release.condition_eval.1m".to_string(),
                count: 2,
            }],
            "Fix: duplicate benchmark case ids must not prove distinct release cases."
        );
    }

    #[test]
    fn backend_consistency_allows_non_benchmark_manifest_without_selected_backend() {
        let manifest = serde_json::json!({
            "cases": [
                {"id": "manifest-row"}
            ]
        });

        assert!(
            backend_consistency_issues(&manifest).is_empty(),
            "Fix: backend consistency applies to benchmark reports that declare selected_backend."
        );
    }

    #[test]
    fn contract_backend_issues_reject_cuda_only_contract_on_wgpu_case() {
        let report = serde_json::json!({
            "selected_backend": "wgpu",
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "wgpu",
                    "contract": {
                        "primitive": "condition eval",
                        "baselines": [
                            {"backend_ids": ["cuda"], "min_speedup_x": 100.0}
                        ]
                    }
                }
            ]
        });

        assert_eq!(
            contract_backend_issues(&report),
            vec![ContractBackendIssue::NoApplicableBaseline {
                case_id: "release.condition_eval.1m".to_string(),
                backend_id: "wgpu".to_string(),
            }],
            "Fix: WGPU benchmark evidence must not pass a CUDA-only performance contract by omission."
        );
    }

    #[test]
    fn contract_backend_issues_accept_backend_agnostic_contract() {
        let report = serde_json::json!({
            "selected_backend": "wgpu",
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "wgpu",
                    "contract": {
                        "primitive": "condition eval",
                        "baselines": [
                            {"backend_ids": [], "min_speedup_x": 2.0}
                        ]
                    }
                }
            ]
        });

        assert!(
            contract_backend_issues(&report).is_empty(),
            "Fix: backend-agnostic contracts must remain valid for fallback backends."
        );
    }

    #[test]
    fn cpu_sota_contract_requires_matching_backend_id() {
        let case = serde_json::json!({
            "contract": {
                "baselines": [
                    {
                        "class": "CpuSota",
                        "backend_ids": ["cuda"],
                        "min_speedup_x": 100.0
                    }
                ]
            }
        });

        assert!(
            benchmark_case_has_cpu_sota_contract(&case, Some("cuda"), 100.0),
            "Fix: CUDA should count CUDA-scoped CpuSota contracts."
        );
        assert!(
            !benchmark_case_has_cpu_sota_contract(&case, Some("wgpu"), 100.0),
            "Fix: WGPU must not inherit CUDA-scoped CpuSota contract counters."
        );
    }

    #[test]
    fn cpu_sota_100x_case_counts_require_pass_summary_evidence() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "pass",
                    "contract": {
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "metrics": {
                        "wall_ns": {"p50": 10},
                        "baseline_wall_ns": {"p50": 2000}
                    },
                    "performance": {"contract_passed": true, "speedup_x": 200.0}
                },
                {
                    "id": "release.entropy_window.1m",
                    "backend_id": "cuda",
                    "status": "fail",
                    "contract": {
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "metrics": {
                        "wall_ns": {"p50": 10},
                        "baseline_wall_ns": {"p50": 2000}
                    },
                    "performance": {"contract_passed": true, "speedup_x": 200.0}
                },
                {
                    "id": "release.wgpu-drift.1m",
                    "backend_id": "wgpu",
                    "status": "pass",
                    "contract": {
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "performance": {"contract_passed": true, "speedup_x": 200.0}
                }
            ]
        });

        assert_eq!(
            cpu_sota_100x_case_counts(&report),
            (2, 1),
            "Fix: derived CPU-SOTA 100x counts must share one backend-aware, pass-evidence-aware primitive."
        );
    }

    #[test]
    fn cpu_sota_100x_case_counts_require_measured_speedup_evidence() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "cases": [
                {
                    "id": "release.claimed-speedup.1m",
                    "backend_id": "cuda",
                    "status": "pass",
                    "contract": {
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "metrics": {
                        "wall_ns": {"p50": 100},
                        "baseline_wall_ns": {"p50": 1000}
                    },
                    "performance": {"contract_passed": true, "speedup_x": 200.0}
                }
            ]
        });

        assert_eq!(
            cpu_sota_100x_case_counts(&report),
            (1, 0),
            "Fix: CPU-SOTA passing counts must be backed by measured baseline_wall_ns / wall_ns speedup, not only performance.speedup_x claims."
        );
    }

    #[test]
    fn cpu_sota_100x_case_counts_use_runner_active_gpu_metric_order() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "cases": [
                {
                    "id": "release.dispatch-timed.1m",
                    "backend_id": "cuda",
                    "status": "pass",
                    "contract": {
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "metrics": {
                        "dispatch_ns": {"p50": 10},
                        "wall_ns": {"p50": 2000},
                        "baseline_wall_ns": {"p50": 1500}
                    },
                    "performance": {"contract_passed": true, "speedup_x": 150.0}
                }
            ]
        });

        assert_eq!(
            cpu_sota_100x_case_counts(&report),
            (1, 1),
            "Fix: CPU-SOTA proof counts must mirror benchmark contract evaluation and prefer dispatch_ns before wall_ns."
        );
    }

    #[test]
    fn contract_backend_issues_reject_empty_baseline_list() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "contract": {
                        "primitive": "condition eval",
                        "baselines": []
                    }
                }
            ]
        });

        assert_eq!(
            contract_backend_issues(&report),
            vec![ContractBackendIssue::MissingBaselines {
                case_id: "release.condition_eval.1m".to_string(),
                backend_id: "cuda".to_string(),
            }],
            "Fix: a performance contract with no baselines must not prove release performance."
        );
    }

    #[test]
    fn cuda_telemetry_labels_track_active_counters() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "cases": [
                {
                    "id": "active-unlabeled",
                    "metrics": {"cuda_ptx_source_cache_misses": {"p50": 1}},
                    "optimization_passes_applied": ["cuda-explicit-backend-selection"]
                },
                {
                    "id": "inactive-labeled",
                    "metrics": {
                        "cuda_ptx_source_cache_entries": {"p50": 0},
                        "cuda_ptx_source_cache_hits": {"p50": 0},
                        "cuda_ptx_source_cache_misses": {"p50": 0}
                    },
                    "optimization_passes_applied": ["cuda-ptx-source-cache"]
                },
                {
                    "id": "active-labeled",
                    "metrics": {"cuda_ptx_source_cache_hits": {"p50": 2}},
                    "optimization_passes_applied": ["cuda-ptx-source-cache"]
                },
                {
                    "id": "graph-unlabeled",
                    "metrics": {"cuda_graph_launches": {"p50": 3}},
                    "optimization_passes_applied": ["cuda-explicit-backend-selection"]
                },
                {
                    "id": "transfer-false-label",
                    "metrics": {
                        "cuda_host_upload_operations": {"p50": 0},
                        "cuda_device_readback_operations": {"p50": 0}
                    },
                    "optimization_passes_applied": ["cuda-transfer-operation-telemetry"]
                },
                {
                    "id": "resident-escape-unlabeled",
                    "metrics": {"cuda_resident_borrowed_fallback_dispatches": {"p50": 1}},
                    "optimization_passes_applied": ["cuda-explicit-backend-selection"]
                },
                {
                    "id": "resident-escape-false-label",
                    "metrics": {"cuda_resident_borrowed_fallback_dispatches": {"p50": 0}},
                    "optimization_passes_applied": ["cuda-resident-borrowed-escape-hatch"]
                }
            ]
        });

        assert_eq!(
            cuda_telemetry_label_issues(&report),
            vec![
                CudaTelemetryLabelIssue::MissingLabel {
                    case_id: "active-unlabeled".to_string(),
                    label: "cuda-ptx-source-cache",
                },
                CudaTelemetryLabelIssue::LabelWithoutCounters {
                    case_id: "inactive-labeled".to_string(),
                    label: "cuda-ptx-source-cache",
                },
                CudaTelemetryLabelIssue::MissingLabel {
                    case_id: "graph-unlabeled".to_string(),
                    label: "cuda-graph-replay",
                },
                CudaTelemetryLabelIssue::LabelWithoutCounters {
                    case_id: "transfer-false-label".to_string(),
                    label: "cuda-transfer-operation-telemetry",
                },
                CudaTelemetryLabelIssue::MissingLabel {
                    case_id: "resident-escape-unlabeled".to_string(),
                    label: "cuda-resident-borrowed-escape-hatch",
                },
                CudaTelemetryLabelIssue::LabelWithoutCounters {
                    case_id: "resident-escape-false-label".to_string(),
                    label: "cuda-resident-borrowed-escape-hatch",
                },
            ],
            "Fix: CUDA release telemetry labels must match measured backend counters."
        );
    }

    #[test]
    fn cuda_forbidden_telemetry_rejects_resident_borrowed_escape_hatch() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "cases": [
                {
                    "id": "native-resident",
                    "metrics": {"cuda_resident_borrowed_fallback_dispatches": {"p50": 0}}
                },
                {
                    "id": "borrowed-escape",
                    "metrics": {"cuda_resident_borrowed_fallback_dispatches": {"p50": 2}}
                }
            ]
        });

        assert_eq!(
            cuda_forbidden_telemetry_issues(&report),
            vec![CudaForbiddenTelemetryIssue::ResidentBorrowedEscapeHatch {
                case_id: "borrowed-escape".to_string(),
                observed_p50: 2.0,
            }],
            "Fix: CUDA benchmark evidence must not pass when resident dispatch used the host-buffer escape hatch."
        );
    }

    #[test]
    fn backend_suite_parity_rejects_missing_family_case_pairs() {
        let cuda = serde_json::json!({
            "backend": "cuda",
            "artifact_statuses": [
                {"family_id": "condition-eval", "requested_case_id": "release.condition_eval.1m"},
                {"family_id": "entropy-window", "requested_case_id": "release.entropy_window.1m"}
            ]
        });
        let wgpu = serde_json::json!({
            "backend": "wgpu",
            "artifact_statuses": [
                {"family_id": "condition-eval", "requested_case_id": "release.condition_eval.1m"},
                {"family_id": "ifds-witness", "requested_case_id": "release.ifds_witness.1m"}
            ]
        });

        assert_eq!(
            backend_suite_parity_issues(&cuda, &wgpu),
            vec![
                BackendSuiteParityIssue::MissingWgpuPair {
                    family_id: "entropy-window".to_string(),
                    requested_case_id: "release.entropy_window.1m".to_string(),
                },
                BackendSuiteParityIssue::MissingCudaPair {
                    family_id: "ifds-witness".to_string(),
                    requested_case_id: "release.ifds_witness.1m".to_string(),
                },
            ],
            "Fix: CUDA and WGPU release suites must cover the same family/case contract."
        );
    }

    #[test]
    fn backend_suite_parity_rejects_status_field_drift_for_matching_pairs() {
        let cuda = serde_json::json!({
            "backend": "cuda",
            "artifact_statuses": [
                {
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m",
                    "case_count": 1,
                    "failed_count": 0,
                    "nonmatching_case_backend_count": 0,
                    "source_fingerprint": "git:cuda-source:dirty=false",
                    "source_tree_fingerprint": "source-tree-v1:shared"
                }
            ]
        });
        let wgpu = serde_json::json!({
            "backend": "wgpu",
            "artifact_statuses": [
                {
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m",
                    "case_count": 0,
                    "failed_count": 1,
                    "nonmatching_case_backend_count": 0,
                    "source_fingerprint": "git:wgpu-source:dirty=false",
                    "source_tree_fingerprint": "source-tree-v1:shared"
                }
            ]
        });

        assert_eq!(
            backend_suite_parity_issues(&cuda, &wgpu),
            vec![
                BackendSuiteParityIssue::StatusFieldMismatch {
                    family_id: "condition-eval".to_string(),
                    requested_case_id: "release.condition_eval.1m".to_string(),
                    field: "case_count",
                    cuda_value: Some(1),
                    wgpu_value: Some(0),
                },
                BackendSuiteParityIssue::StatusFieldMismatch {
                    family_id: "condition-eval".to_string(),
                    requested_case_id: "release.condition_eval.1m".to_string(),
                    field: "failed_count",
                    cuda_value: Some(0),
                    wgpu_value: Some(1),
                }
            ],
            "Fix: WGPU parity must compare proof strength for matching suite rows while tolerating evidence-only commit fingerprint drift."
        );
    }

    #[test]
    fn backend_suite_parity_rejects_source_tree_drift_not_evidence_commit_drift() {
        let cuda = serde_json::json!({
            "backend": "cuda",
            "artifact_statuses": [
                {
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m",
                    "source_fingerprint": "git:cuda-evidence-commit:dirty=false",
                    "source_tree_fingerprint": "source-tree-v1:cuda"
                }
            ]
        });
        let wgpu = serde_json::json!({
            "backend": "wgpu",
            "artifact_statuses": [
                {
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m",
                    "source_fingerprint": "git:wgpu-evidence-commit:dirty=false",
                    "source_tree_fingerprint": "source-tree-v1:wgpu"
                }
            ]
        });

        assert_eq!(
            backend_suite_parity_issues(&cuda, &wgpu),
            vec![
                BackendSuiteParityIssue::StatusStringFieldMismatch {
                    family_id: "condition-eval".to_string(),
                    requested_case_id: "release.condition_eval.1m".to_string(),
                    field: "source_tree_fingerprint",
                    cuda_value: Some("source-tree-v1:cuda".to_string()),
                    wgpu_value: Some("source-tree-v1:wgpu".to_string()),
                },
            ],
            "Fix: WGPU parity must reject source tree drift without treating benchmark evidence commits as backend source drift."
        );
    }

    #[test]
    fn backend_suite_parity_rejects_status_blocker_drift_for_matching_pairs() {
        let cuda = serde_json::json!({
            "backend": "cuda",
            "artifact_statuses": [
                {
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m",
                    "blockers": []
                }
            ]
        });
        let wgpu = serde_json::json!({
            "backend": "wgpu",
            "artifact_statuses": [
                {
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m",
                    "blockers": ["case `release.condition_eval.1m` failed: WGPU output drift"]
                }
            ]
        });

        assert_eq!(
            backend_suite_parity_issues(&cuda, &wgpu),
            vec![BackendSuiteParityIssue::StatusBlockersMismatch {
                family_id: "condition-eval".to_string(),
                requested_case_id: "release.condition_eval.1m".to_string(),
                cuda_blockers: Some(Vec::new()),
                wgpu_blockers: Some(vec![
                    "case `release.condition_eval.1m` failed: WGPU output drift".to_string()
                ]),
            }],
            "Fix: WGPU parity must reject matching suite rows with different blocker state."
        );
    }

    #[test]
    fn backend_suite_parity_rejects_cpu_sota_count_drift() {
        let cuda = serde_json::json!({
            "backend": "cuda",
            "artifact_statuses": [
                {
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m",
                    "cpu_sota_100x_contract_cases": 1,
                    "cpu_sota_100x_passing_cases": 1
                }
            ]
        });
        let wgpu = serde_json::json!({
            "backend": "wgpu",
            "artifact_statuses": [
                {
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m",
                    "cpu_sota_100x_contract_cases": 0,
                    "cpu_sota_100x_passing_cases": 0
                }
            ]
        });

        assert_eq!(
            backend_suite_parity_issues(&cuda, &wgpu),
            vec![
                BackendSuiteParityIssue::StatusFieldMismatch {
                    family_id: "condition-eval".to_string(),
                    requested_case_id: "release.condition_eval.1m".to_string(),
                    field: "cpu_sota_100x_contract_cases",
                    cuda_value: Some(1),
                    wgpu_value: Some(0),
                },
                BackendSuiteParityIssue::StatusFieldMismatch {
                    family_id: "condition-eval".to_string(),
                    requested_case_id: "release.condition_eval.1m".to_string(),
                    field: "cpu_sota_100x_passing_cases",
                    cuda_value: Some(1),
                    wgpu_value: Some(0),
                },
            ],
            "Fix: WGPU/CUDA parity must compare CPU-SOTA proof strength for matching suite rows."
        );
    }

    #[test]
    fn backend_suite_backend_identity_comes_from_release_suite_name() {
        assert_eq!(
            expected_backend_for_suite_evidence(
                "release/evidence/benchmarks/cuda-release-suite.json"
            ),
            Some("cuda"),
            "Fix: CUDA suite filenames define the required backend identity."
        );
        assert_eq!(
            expected_backend_for_suite_evidence(
                "release/evidence/benchmarks/wgpu-fallback-suite.json"
            ),
            Some("wgpu"),
            "Fix: WGPU fallback suite filenames define the required backend identity."
        );
    }

    #[test]
    fn backend_suite_backend_identity_rejects_missing_or_mismatched_field() {
        assert_eq!(
            backend_suite_backend_issue(&serde_json::json!({}), "cuda"),
            Some(BackendSuiteBackendIssue::Missing {
                expected_backend: "cuda".to_string(),
            }),
            "Fix: suite backend identity must be explicit, not inferred from artifact rows."
        );
        assert_eq!(
            backend_suite_backend_issue(&serde_json::json!({"backend": "wgpu"}), "cuda"),
            Some(BackendSuiteBackendIssue::Mismatch {
                expected_backend: "cuda".to_string(),
                actual_backend: "wgpu".to_string(),
            }),
            "Fix: a CUDA release suite must not self-report a WGPU backend."
        );
        assert_eq!(
            backend_suite_backend_issue(&serde_json::json!({"backend": "cuda"}), "cuda"),
            None,
            "Fix: matching suite backend identity should pass."
        );
    }

    #[test]
    fn backend_suite_parity_rejects_mislabeled_suite_backends() {
        let cuda = serde_json::json!({
            "backend": "wgpu",
            "artifact_statuses": [
                {"family_id": "condition-eval", "requested_case_id": "release.condition_eval.1m"}
            ]
        });
        let wgpu = serde_json::json!({
            "artifact_statuses": [
                {"family_id": "condition-eval", "requested_case_id": "release.condition_eval.1m"}
            ]
        });

        assert_eq!(
            backend_suite_parity_issues(&cuda, &wgpu),
            vec![
                BackendSuiteParityIssue::CudaBackendIdentity {
                    issue: BackendSuiteBackendIssue::Mismatch {
                        expected_backend: "cuda".to_string(),
                        actual_backend: "wgpu".to_string(),
                    },
                },
                BackendSuiteParityIssue::WgpuBackendIdentity {
                    issue: BackendSuiteBackendIssue::Missing {
                        expected_backend: "wgpu".to_string(),
                    },
                },
            ],
            "Fix: WGPU/CUDA parity must reject mislabeled peer suite identities, not only row-level family/case coverage."
        );
    }

    #[test]
    fn backend_suite_parity_rejects_duplicate_family_case_pairs_with_equal_counts() {
        let cuda = serde_json::json!({
            "backend": "cuda",
            "artifact_statuses": [
                {
                    "path": "release/evidence/benchmarks/cuda-condition-a.json",
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m"
                },
                {
                    "path": "release/evidence/benchmarks/cuda-condition-b.json",
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m"
                }
            ]
        });
        let wgpu = serde_json::json!({
            "backend": "wgpu",
            "artifact_statuses": [
                {
                    "path": "release/evidence/benchmarks/wgpu-condition-a.json",
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m"
                },
                {
                    "path": "release/evidence/benchmarks/wgpu-condition-b.json",
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m"
                }
            ]
        });

        assert_eq!(
            backend_suite_parity_issues(&cuda, &wgpu),
            vec![
                BackendSuiteParityIssue::DuplicateCudaPair {
                    family_id: "condition-eval".to_string(),
                    requested_case_id: "release.condition_eval.1m".to_string(),
                    count: 2,
                },
                BackendSuiteParityIssue::DuplicateWgpuPair {
                    family_id: "condition-eval".to_string(),
                    requested_case_id: "release.condition_eval.1m".to_string(),
                    count: 2,
                },
            ],
            "Fix: WGPU parity must reject duplicate family/case rows even when CUDA and WGPU counts match."
        );
    }

    #[test]
    fn backend_suite_inventory_rejects_missing_cross_entries() {
        let suite = serde_json::json!({
            "artifacts": [
                "release/evidence/benchmarks/cuda/condition.json",
                "release/evidence/benchmarks/cuda/entropy.json"
            ],
            "artifact_statuses": [
                {"path": "release/evidence/benchmarks/cuda/condition.json"},
                {"path": "release/evidence/benchmarks/cuda/ifds.json"}
            ]
        });

        assert_eq!(
            backend_suite_inventory_issues(&suite),
            vec![
                BackendSuiteInventoryIssue::MissingStatus {
                    path: "release/evidence/benchmarks/cuda/entropy.json".to_string(),
                },
                BackendSuiteInventoryIssue::MissingArtifact {
                    path: "release/evidence/benchmarks/cuda/ifds.json".to_string(),
                },
            ],
            "Fix: suite artifacts and artifact_statuses must describe the same file set."
        );
    }

    #[test]
    fn backend_suite_inventory_rejects_duplicate_paths_and_count_drift() {
        let suite = serde_json::json!({
            "artifacts": [
                "release/evidence/benchmarks/cuda/condition.json",
                "release/evidence/benchmarks/cuda/condition.json"
            ],
            "artifact_statuses": [
                {"path": "release/evidence/benchmarks/cuda/condition.json"}
            ]
        });

        assert_eq!(
            backend_suite_inventory_issues(&suite),
            vec![
                BackendSuiteInventoryIssue::CountMismatch {
                    artifact_count: 2,
                    status_count: 1,
                },
                BackendSuiteInventoryIssue::DuplicateArtifact {
                    path: "release/evidence/benchmarks/cuda/condition.json".to_string(),
                },
            ],
            "Fix: duplicate suite inventory entries must not prove artifact coverage."
        );
    }

    #[test]
    fn backend_suite_inventory_rejects_duplicate_family_coverage() {
        let suite = serde_json::json!({
            "artifacts": [
                "release/evidence/benchmarks/cuda/condition-fast.json",
                "release/evidence/benchmarks/cuda/condition-slow.json"
            ],
            "artifact_statuses": [
                {
                    "path": "release/evidence/benchmarks/cuda/condition-fast.json",
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m"
                },
                {
                    "path": "release/evidence/benchmarks/cuda/condition-slow.json",
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.10m"
                }
            ]
        });

        assert_eq!(
            backend_suite_inventory_issues(&suite),
            vec![BackendSuiteInventoryIssue::DuplicateFamily {
                family_id: "condition-eval".to_string(),
                count: 2,
            }],
            "Fix: backend suite family_count must represent unique workload families, not repeated family rows."
        );
    }

    #[test]
    fn backend_suite_inventory_rejects_declared_family_count_drift() {
        let artifacts = (0..12)
            .map(|index| format!("release/evidence/benchmarks/cuda/workload-{index}.json"))
            .collect::<Vec<_>>();
        let artifact_statuses = artifacts
            .iter()
            .enumerate()
            .map(|(index, path)| {
                serde_json::json!({
                    "path": path,
                    "family_id": format!("workload-{index}"),
                    "requested_case_id": format!("release.workload_{index}.1m")
                })
            })
            .collect::<Vec<_>>();
        let suite = serde_json::json!({
            "family_count": 13,
            "artifacts": artifacts,
            "artifact_statuses": artifact_statuses
        });

        assert_eq!(
            backend_suite_inventory_issues(&suite),
            vec![
                BackendSuiteInventoryIssue::DeclaredFamilyArtifactCountMismatch {
                    family_count: 13,
                    artifact_count: 12,
                },
                BackendSuiteInventoryIssue::DeclaredFamilyStatusCountMismatch {
                    family_count: 13,
                    status_family_count: 12,
                },
            ],
            "Fix: backend suite family_count must be derived from suite rows, not trusted as a stale release total."
        );
    }

    #[test]
    fn backend_suite_matrix_coverage_rejects_missing_optional_workloads() {
        let matrix = serde_json::json!({
            "families": [
                {"id": "condition-eval"},
                {"id": "compound-fused-filter"},
                {"id": "adaptive-routing"}
            ]
        });
        let suite = serde_json::json!({
            "artifact_statuses": [
                {"family_id": "condition-eval"}
            ]
        });

        assert_eq!(
            backend_suite_matrix_coverage_issues(&matrix, &suite),
            vec![
                BackendSuiteMatrixCoverageIssue::FamilyCountMismatch {
                    matrix_family_count: 3,
                    suite_family_count: 1,
                },
                BackendSuiteMatrixCoverageIssue::MissingMatrixFamily {
                    family_id: "adaptive-routing".to_string(),
                },
                BackendSuiteMatrixCoverageIssue::MissingMatrixFamily {
                    family_id: "compound-fused-filter".to_string(),
                },
            ],
            "Fix: backend suites must cover every release workload matrix family, including optional acceleration workloads."
        );
    }

    #[test]
    fn backend_suite_matrix_coverage_rejects_extra_suite_family() {
        let matrix = serde_json::json!({
            "families": [
                {"id": "condition-eval"}
            ]
        });
        let suite = serde_json::json!({
            "artifact_statuses": [
                {"family_id": "condition-eval"},
                {"family_id": "stale-family"}
            ]
        });

        assert_eq!(
            backend_suite_matrix_coverage_issues(&matrix, &suite),
            vec![
                BackendSuiteMatrixCoverageIssue::FamilyCountMismatch {
                    matrix_family_count: 1,
                    suite_family_count: 2,
                },
                BackendSuiteMatrixCoverageIssue::ExtraSuiteFamily {
                    family_id: "stale-family".to_string(),
                },
            ],
            "Fix: backend suites must not carry stale family rows outside the release workload matrix."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_stale_artifact_metadata() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "source_fingerprint": "git:old:dirty=false",
            "source_tree_fingerprint": "source-tree-v1:old",
            "selected_backend": "cuda",
            "case_count": 2,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m"
        });
        let artifact = serde_json::json!({
            "source_fingerprint": "git:new:dirty=false",
            "source_tree_fingerprint": "source-tree-v1:new",
            "selected_backend": "wgpu",
            "summary": {"total_cases": 1, "passed": 0, "failed": 1},
            "cases": [
                {"id": "release.other.1m"}
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![
                BackendSuiteArtifactStatusIssue::SourceFingerprintMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    status_source_fingerprint: "git:old:dirty=false".to_string(),
                    artifact_source_fingerprint: "git:new:dirty=false".to_string(),
                },
                BackendSuiteArtifactStatusIssue::SourceTreeFingerprintMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    status_source_tree_fingerprint: "source-tree-v1:old".to_string(),
                    artifact_source_tree_fingerprint: "source-tree-v1:new".to_string(),
                },
                BackendSuiteArtifactStatusIssue::SelectedBackendMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    status_selected_backend: "cuda".to_string(),
                    artifact_selected_backend: "wgpu".to_string(),
                },
                BackendSuiteArtifactStatusIssue::CaseCountMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    status_case_count: 2,
                    artifact_case_count: 1,
                },
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    field: "nonmatching_case_backend_count",
                    status_value: 0,
                    artifact_value: 1,
                },
                BackendSuiteArtifactStatusIssue::FailedCountMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    status_failed_count: 0,
                    artifact_failed_count: 1,
                },
                BackendSuiteArtifactStatusIssue::MissingRequestedCase {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    requested_case_id: "release.condition_eval.1m".to_string(),
                },
            ],
            "Fix: backend suite status rows must be proven against the listed artifact JSON."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_duplicate_requested_case_rows() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "requested_case_id": "release.condition_eval.1m",
            "case_count": 3,
            "failed_count": 0
        });
        let artifact = serde_json::json!({
            "summary": {"total_cases": 3, "passed": 3, "failed": 0},
            "cases": [
                {"id": "release.condition_eval.1m", "backend_id": "cuda", "status": "pass"},
                {"id": "release.condition_eval.1m", "backend_id": "cuda", "status": "pass"},
                {"id": "release.other.1m", "backend_id": "cuda", "status": "pass"}
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![BackendSuiteArtifactStatusIssue::DuplicateRequestedCase {
                path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                requested_case_id: "release.condition_eval.1m".to_string(),
                count: 2,
            }],
            "Fix: suite status requested_case_id must identify exactly one benchmark row inside the artifact."
        );
    }

    #[test]
    fn backend_suite_artifact_status_accepts_matching_metadata() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "source_fingerprint": "git:abc:dirty=false",
            "source_tree_fingerprint": "source-tree-v1:abc",
            "selected_backend": "cuda",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m"
        });
        let artifact = serde_json::json!({
            "source_fingerprint": "git:abc:dirty=false",
            "source_tree_fingerprint": "source-tree-v1:abc",
            "selected_backend": "cuda",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "cases": [
                {"id": "release.condition_eval.1m", "backend_id": "cuda", "status": "pass"}
            ]
        });

        assert!(
            backend_suite_artifact_status_issues(&status, &artifact).is_empty(),
            "Fix: matching suite status and artifact JSON should pass."
        );
    }

    #[test]
    fn backend_suite_artifact_status_accepts_float_metric_percentiles() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "selected_backend": "cuda",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m",
            "min_wall_samples": 30,
            "min_wall_p50": 12,
            "min_wall_p95": 20,
            "min_wall_p99": 30
        });
        let artifact = serde_json::json!({
            "selected_backend": "cuda",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "pass",
                    "metrics": {
                        "wall_ns": {"samples": 30, "p50": 12.75, "p95": 20.25, "p99": 30.875}
                    }
                }
            ]
        });

        assert!(
            backend_suite_artifact_status_issues(&status, &artifact).is_empty(),
            "Fix: backend suite status verification must parse benchmark float percentiles the same way suite generation does."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_backend_mismatch_counter_drift() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "selected_backend": "cuda",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m"
        });
        let artifact = serde_json::json!({
            "selected_backend": "cuda",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "cases": [
                {"id": "release.condition_eval.1m", "backend_id": "wgpu", "status": "pass"}
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                field: "nonmatching_case_backend_count",
                status_value: 0,
                artifact_value: 1,
            }],
            "Fix: backend suite status rows must not hide case-level backend drift."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_summary_failed_count_hidden_by_pass_status() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "selected_backend": "cuda",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m"
        });
        let artifact = serde_json::json!({
            "selected_backend": "cuda",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "pass",
                    "correctness": {
                        "Invalid": {
                            "reason": "CUDA/WGPU output mismatch at row 17"
                        }
                    }
                }
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json"
                        .to_string(),
                    field: "summary.passed",
                    status_value: 1,
                    artifact_value: 0,
                },
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json"
                        .to_string(),
                    field: "summary.failed",
                    status_value: 0,
                    artifact_value: 1,
                },
                BackendSuiteArtifactStatusIssue::FailedCountMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json"
                        .to_string(),
                    status_failed_count: 0,
                    artifact_failed_count: 1,
                },
            ],
            "Fix: suite status must not trust summary.failed when case evidence exposes a contradictory benchmark failure."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_stale_artifact_summary_passed_count() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "selected_backend": "cuda",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m"
        });
        let artifact = serde_json::json!({
            "selected_backend": "cuda",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "pass",
                    "performance": {"contract_passed": false}
                }
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json"
                        .to_string(),
                    field: "summary.passed",
                    status_value: 1,
                    artifact_value: 0,
                },
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json"
                        .to_string(),
                    field: "summary.failed",
                    status_value: 0,
                    artifact_value: 1,
                },
                BackendSuiteArtifactStatusIssue::FailedCountMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json"
                        .to_string(),
                    status_failed_count: 0,
                    artifact_failed_count: 1,
                },
            ],
            "Fix: suite artifact validation must reject stale summary.passed counts derived without case contract evidence."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_stale_artifact_summary_total_cases() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "selected_backend": "cuda",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m"
        });
        let artifact = serde_json::json!({
            "selected_backend": "cuda",
            "summary": {"total_cases": 2, "passed": 1, "failed": 0},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "pass"
                }
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                field: "summary.total_cases",
                status_value: 2,
                artifact_value: 1,
            }],
            "Fix: suite artifact validation must reject summary.total_cases drift even when status case_count matches the cases array."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_missing_artifact_summary_passed_count() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "selected_backend": "cuda",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m"
        });
        let artifact = serde_json::json!({
            "selected_backend": "cuda",
            "summary": {"total_cases": 1, "failed": 0},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "pass"
                }
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![BackendSuiteArtifactStatusIssue::MissingField {
                path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                field: "summary.passed",
            }],
            "Fix: suite artifact validation must require the full summary total/pass/fail triplet, not accept partial failed-only summaries."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_unproven_case_pass_status() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "selected_backend": "cuda",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m"
        });
        let artifact = serde_json::json!({
            "selected_backend": "cuda",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda"
                }
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json"
                        .to_string(),
                    field: "summary.passed",
                    status_value: 1,
                    artifact_value: 0,
                },
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json"
                        .to_string(),
                    field: "summary.failed",
                    status_value: 0,
                    artifact_value: 1,
                },
                BackendSuiteArtifactStatusIssue::FailedCountMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json"
                        .to_string(),
                    status_failed_count: 0,
                    artifact_failed_count: 1,
                },
            ],
            "Fix: suite artifact validation must count only explicitly passing cases as release evidence."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_omitted_artifact_backed_fields() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "requested_case_id": "release.condition_eval.1m"
        });
        let artifact = serde_json::json!({
            "source_fingerprint": "git:abc:dirty=false",
            "source_tree_fingerprint": "source-tree-v1:abc",
            "selected_backend": "cuda",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "environment": {
                "cpu_model": "AMD Ryzen 9 9950X 16-Core Processor",
                "gpu_devices": [
                    {
                        "name": "NVIDIA GeForce RTX 5090",
                        "memory_total_mib": 32607,
                        "compute_capability_major": 12,
                        "compute_capability_minor": 0
                    }
                ],
                "nvidia_driver_version": "570.211.01",
                "nvidia_cuda_version": "12.8"
            },
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "pass",
                    "metrics": {
                        "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                        "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002},
                        "kernel_launches": {"samples": 30, "p50": 1}
                    },
                    "contract": {
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "performance": {"contract_passed": true, "speedup_x": 120.0}
                }
            ]
        });

        let missing_fields = backend_suite_artifact_status_issues(&status, &artifact)
            .into_iter()
            .filter_map(|issue| match issue {
                BackendSuiteArtifactStatusIssue::MissingField { field, .. } => Some(field),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            missing_fields,
            vec![
                "source_fingerprint",
                "source_tree_fingerprint",
                "selected_backend",
                "case_count",
                "nonmatching_case_backend_count",
                "failed_count",
                "min_wall_samples",
                "min_baseline_wall_samples",
                "min_wall_p50",
                "min_wall_p95",
                "min_wall_p99",
                "min_baseline_wall_p50",
                "min_baseline_wall_p95",
                "min_baseline_wall_p99",
                "min_kernel_launches",
                "gpu_memory_total_mib",
                "gpu_compute_capability_major",
                "gpu_compute_capability_minor",
                "host_cpu_model",
                "gpu_model",
                "nvidia_driver_version",
                "nvidia_cuda_version",
                "cpu_sota_100x_contract_cases",
                "cpu_sota_100x_passing_cases",
            ],
            "Fix: backend suite status rows must not omit artifact-backed proof fields."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_inflated_metric_minima() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json",
            "selected_backend": "wgpu",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m",
            "min_wall_samples": 35,
            "min_wall_p50": 100,
            "min_kernel_launches": 1
        });
        let artifact = serde_json::json!({
            "selected_backend": "wgpu",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "wgpu",
                    "status": "pass",
                    "metrics": {
                        "wall_ns": {"samples": 20, "p50": 150},
                        "kernel_launches": {"p50": 0}
                    }
                }
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json"
                        .to_string(),
                    field: "min_wall_samples",
                    status_value: 35,
                    artifact_value: 20,
                },
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json"
                        .to_string(),
                    field: "min_wall_p50",
                    status_value: 100,
                    artifact_value: 150,
                },
                BackendSuiteArtifactStatusIssue::MissingField {
                    path: "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json"
                        .to_string(),
                    field: "min_wall_p95",
                },
                BackendSuiteArtifactStatusIssue::MissingField {
                    path: "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json"
                        .to_string(),
                    field: "min_wall_p99",
                },
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json"
                        .to_string(),
                    field: "min_kernel_launches",
                    status_value: 1,
                    artifact_value: 0,
                },
            ],
            "Fix: backend suite status metric minima must be recomputed from the artifact JSON, not trusted as independent proof."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_provenance_drift() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "selected_backend": "cuda",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m",
            "host_cpu_model": "different CPU",
            "gpu_model": "different GPU",
            "gpu_memory_total_mib": 1,
            "gpu_compute_capability_major": 7,
            "gpu_compute_capability_minor": 5,
            "nvidia_driver_version": "000.000",
            "nvidia_cuda_version": "0.0"
        });
        let artifact = serde_json::json!({
            "selected_backend": "cuda",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "environment": {
                "cpu_model": "AMD Ryzen 9 9950X 16-Core Processor",
                "gpu_devices": [
                    {
                        "name": "NVIDIA GeForce RTX 5090",
                        "memory_total_mib": 32607,
                        "compute_capability_major": 12,
                        "compute_capability_minor": 0
                    }
                ],
                "nvidia_driver_version": "570.211.01",
                "nvidia_cuda_version": "12.8"
            },
            "cases": [
                {"id": "release.condition_eval.1m", "backend_id": "cuda", "status": "pass"}
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    field: "gpu_memory_total_mib",
                    status_value: 1,
                    artifact_value: 32607,
                },
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    field: "gpu_compute_capability_major",
                    status_value: 7,
                    artifact_value: 12,
                },
                BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    field: "gpu_compute_capability_minor",
                    status_value: 5,
                    artifact_value: 0,
                },
                BackendSuiteArtifactStatusIssue::StringFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    field: "host_cpu_model",
                    status_value: "different CPU".to_string(),
                    artifact_value: "AMD Ryzen 9 9950X 16-Core Processor".to_string(),
                },
                BackendSuiteArtifactStatusIssue::StringFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    field: "gpu_model",
                    status_value: "different GPU".to_string(),
                    artifact_value: "NVIDIA GeForce RTX 5090".to_string(),
                },
                BackendSuiteArtifactStatusIssue::StringFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    field: "nvidia_driver_version",
                    status_value: "000.000".to_string(),
                    artifact_value: "570.211.01".to_string(),
                },
                BackendSuiteArtifactStatusIssue::StringFieldMismatch {
                    path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    field: "nvidia_cuda_version",
                    status_value: "0.0".to_string(),
                    artifact_value: "12.8".to_string(),
                },
            ],
            "Fix: backend suite status provenance must be proven by the artifact environment."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_blank_environment_provenance() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "selected_backend": "cuda",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m",
            "host_cpu_model": "AMD Ryzen 9 9950X 16-Core Processor",
            "gpu_model": "NVIDIA GeForce RTX 5090",
            "gpu_memory_total_mib": 32607,
            "gpu_compute_capability_major": 12,
            "gpu_compute_capability_minor": 0,
            "nvidia_driver_version": "570.211.01",
            "nvidia_cuda_version": "12.8"
        });
        let artifact = serde_json::json!({
            "selected_backend": "cuda",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "environment": {
                "cpu_model": "   ",
                "gpu_devices": [
                    {
                        "name": "\t",
                        "memory_total_mib": 32607,
                        "compute_capability_major": 12,
                        "compute_capability_minor": 0
                    }
                ],
                "nvidia_driver_version": " ",
                "nvidia_cuda_version": "\n"
            },
            "cases": [
                {"id": "release.condition_eval.1m", "backend_id": "cuda", "status": "pass"}
            ]
        });

        let missing_fields = backend_suite_artifact_status_issues(&status, &artifact)
            .into_iter()
            .filter_map(|issue| match issue {
                BackendSuiteArtifactStatusIssue::MissingField { field, .. } => Some(field),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            missing_fields,
            vec![
                "host_cpu_model",
                "gpu_model",
                "nvidia_driver_version",
                "nvidia_cuda_version"
            ],
            "Fix: whitespace-only benchmark artifact environment provenance must be treated as missing evidence."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_unproven_contract_counts() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/wgpu-workload-06-quantified-condition-loops.json",
            "selected_backend": "wgpu",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.quantified_condition_loops.1m",
            "cpu_sota_100x_contract_cases": 1,
            "cpu_sota_100x_passing_cases": 1
        });
        let artifact = serde_json::json!({
            "selected_backend": "wgpu",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "cases": [
                {
                    "id": "release.quantified_condition_loops.1m",
                    "backend_id": "wgpu",
                    "status": "pass",
                    "contract": null,
                    "performance": {"contract_passed": true, "speedup_x": 1000.0}
                }
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![
                BackendSuiteArtifactStatusIssue::CpuSota100xContractCaseCountMismatch {
                    path: "release/evidence/benchmarks/wgpu-workload-06-quantified-condition-loops.json".to_string(),
                    status_contract_cases: 1,
                    artifact_contract_cases: 0,
                },
                BackendSuiteArtifactStatusIssue::CpuSota100xPassingCaseCountMismatch {
                    path: "release/evidence/benchmarks/wgpu-workload-06-quantified-condition-loops.json".to_string(),
                    status_passing_cases: 1,
                    artifact_passing_cases: 0,
                },
            ],
            "Fix: backend suite status must not claim CPU-SOTA 100x contract proof absent from the artifact JSON."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_wrong_backend_contract_counts() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json",
            "selected_backend": "wgpu",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m",
            "cpu_sota_100x_contract_cases": 1,
            "cpu_sota_100x_passing_cases": 1
        });
        let artifact = serde_json::json!({
            "selected_backend": "wgpu",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "wgpu",
                    "status": "pass",
                    "contract": {
                        "primitive": "condition eval",
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "performance": {"contract_passed": true, "speedup_x": 120.0}
                }
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![
                BackendSuiteArtifactStatusIssue::CpuSota100xContractCaseCountMismatch {
                    path: "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json"
                        .to_string(),
                    status_contract_cases: 1,
                    artifact_contract_cases: 0,
                },
                BackendSuiteArtifactStatusIssue::CpuSota100xPassingCaseCountMismatch {
                    path: "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json"
                        .to_string(),
                    status_passing_cases: 1,
                    artifact_passing_cases: 0,
                },
            ],
            "Fix: WGPU suite status must not count a CUDA-only CpuSota baseline as WGPU proof."
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_unproven_cpu_sota_pass_status() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json",
            "selected_backend": "wgpu",
            "case_count": 1,
            "failed_count": 1,
            "nonmatching_case_backend_count": 0,
            "requested_case_id": "release.condition_eval.1m",
            "cpu_sota_100x_contract_cases": 1,
            "cpu_sota_100x_passing_cases": 1
        });
        let artifact = serde_json::json!({
            "selected_backend": "wgpu",
            "summary": {"total_cases": 1, "passed": 0, "failed": 1},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "wgpu",
                    "contract": {
                        "primitive": "condition eval",
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["wgpu"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "performance": {"contract_passed": true, "speedup_x": 120.0}
                }
            ]
        });

        assert_eq!(
            backend_suite_artifact_status_issues(&status, &artifact),
            vec![BackendSuiteArtifactStatusIssue::CpuSota100xPassingCaseCountMismatch {
                path: "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json"
                    .to_string(),
                status_passing_cases: 1,
                artifact_passing_cases: 0,
            }],
            "Fix: CPU-SOTA suite status must not count contract_passed speedup evidence without an explicit passing case status."
        );
    }

    #[test]
    fn backend_suite_artifact_status_accepts_proven_contract_counts() {
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json",
            "selected_backend": "wgpu",
            "case_count": 1,
            "failed_count": 0,
            "nonmatching_case_backend_count": 0,
            "min_wall_samples": 30,
            "min_wall_p50": 10,
            "min_wall_p95": 11,
            "min_wall_p99": 12,
            "min_baseline_wall_samples": 30,
            "min_baseline_wall_p50": 1200,
            "min_baseline_wall_p95": 1201,
            "min_baseline_wall_p99": 1202,
            "requested_case_id": "release.condition_eval.1m",
            "cpu_sota_100x_contract_cases": 1,
            "cpu_sota_100x_passing_cases": 1
        });
        let artifact = serde_json::json!({
            "selected_backend": "wgpu",
            "summary": {"total_cases": 1, "passed": 1, "failed": 0},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "wgpu",
                    "status": "pass",
                    "contract": {
                        "primitive": "condition eval",
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda", "wgpu"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "metrics": {
                        "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                        "baseline_wall_ns": {"samples": 30, "p50": 1200, "p95": 1201, "p99": 1202}
                    },
                    "performance": {"contract_passed": true, "speedup_x": 120.0}
                }
            ]
        });

        assert!(
            backend_suite_artifact_status_issues(&status, &artifact).is_empty(),
            "Fix: suite status rows with contract counters should pass only when artifact cases prove the same counters."
        );
    }

    #[test]
    fn backend_suite_parity_rejects_count_drift_even_with_duplicate_metadata() {
        let cuda = serde_json::json!({
            "backend": "cuda",
            "artifact_statuses": [
                {"family_id": "condition-eval", "requested_case_id": "release.condition_eval.1m"}
            ]
        });
        let wgpu = serde_json::json!({
            "backend": "wgpu",
            "artifact_statuses": [
                {"family_id": "condition-eval", "requested_case_id": "release.condition_eval.1m"},
                {"family_id": "condition-eval", "requested_case_id": "release.condition_eval.1m"}
            ]
        });

        assert_eq!(
            backend_suite_parity_issues(&cuda, &wgpu),
            vec![
                BackendSuiteParityIssue::CountMismatch {
                    cuda_count: 1,
                    wgpu_count: 2,
                },
                BackendSuiteParityIssue::DuplicateWgpuPair {
                    family_id: "condition-eval".to_string(),
                    requested_case_id: "release.condition_eval.1m".to_string(),
                    count: 2,
                },
            ],
            "Fix: duplicate suite metadata should not silently prove artifact-count parity."
        );
    }

    #[test]
    fn backend_suite_parity_rejects_shared_artifact_paths() {
        let cuda = serde_json::json!({
            "backend": "cuda",
            "artifacts": ["release/evidence/benchmarks/workload-01-condition-eval.json"],
            "artifact_statuses": [
                {
                    "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m"
                }
            ]
        });
        let wgpu = serde_json::json!({
            "backend": "wgpu",
            "artifacts": ["release/evidence/benchmarks/workload-01-condition-eval.json"],
            "artifact_statuses": [
                {
                    "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
                    "family_id": "condition-eval",
                    "requested_case_id": "release.condition_eval.1m"
                }
            ]
        });

        assert_eq!(
            backend_suite_parity_issues(&cuda, &wgpu),
            vec![BackendSuiteParityIssue::SharedArtifactPath {
                path: "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
            }],
            "Fix: WGPU fallback evidence must not reuse or overwrite CUDA release benchmark artifacts."
        );
    }
}
