use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::acceleration_plan_gate::validate_plan_progress_artifact_bytes;
use crate::artifact_paths::{
    FRONTIER_LEADERBOARD_ARTIFACT, PLAN_PROGRESS_ARTIFACT, RESEARCH_AUDIT_ARTIFACT,
};
use crate::dedup_report::validate_duplicate_family_report_artifact;
use crate::hash::sha256_hex;
use crate::research_audit::validate_research_audit_artifact_bytes;

use super::expected_artifacts::{
    expected_artifact_registry_blockers, COMMAND_MODE_EXTERNAL_ARTIFACTS_ONLY,
    COMMAND_MODE_SPAWNED, EXPECTED_ARTIFACT_REGISTRY,
};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReleaseEvidenceArtifactStatus {
    pub(crate) path: String,
    pub(crate) exists: bool,
    pub(crate) bytes: u64,
    pub(crate) read_error: Option<String>,
    pub(crate) owner_lane: &'static str,
    pub(crate) generator_command: String,
    pub(crate) command_mode: &'static str,
    pub(crate) content_sha256: Option<String>,
    pub(crate) source_fingerprint: Option<String>,
    pub(crate) freshness_fingerprint: Option<String>,
    pub(crate) blockers: Vec<String>,
}

pub(crate) fn inspect_expected_artifacts(
    workspace_root: &Path,
    command_args: &[&'static str],
    expected_artifacts: &[&'static str],
) -> Vec<ReleaseEvidenceArtifactStatus> {
    inspect_expected_artifacts_with_mode(
        workspace_root,
        command_args,
        expected_artifacts,
        COMMAND_MODE_SPAWNED,
    )
}

pub(crate) fn inspect_expected_artifacts_with_mode(
    workspace_root: &Path,
    command_args: &[&'static str],
    expected_artifacts: &[&'static str],
    command_mode: &'static str,
) -> Vec<ReleaseEvidenceArtifactStatus> {
    let owner_lane = owner_lane_for_command(command_args);
    let generator_command = generator_command(command_args);
    expected_artifacts
        .iter()
        .map(|artifact| {
            let path = workspace_root.join(artifact);
            match fs::metadata(&path) {
                Ok(metadata) => {
                    let (content_sha256, read_error, semantic_blockers) = if metadata.is_file() {
                        match fs::read(&path) {
                            Ok(bytes) => (
                                Some(sha256_hex(&bytes)),
                                None,
                                artifact_semantic_blockers(
                                    artifact,
                                    &bytes,
                                    &generator_command,
                                    command_mode,
                                ),
                            ),
                            Err(error) => (None, Some(error.to_string()), Vec::new()),
                        }
                    } else {
                        (
                            None,
                            Some("expected artifact path is not a file".to_string()),
                            Vec::new(),
                        )
                    };
                    let (source_fingerprint, freshness_fingerprint) =
                        artifact_provenance_fingerprints(
                            artifact,
                            &generator_command,
                            metadata.len(),
                            content_sha256.as_deref(),
                        );
                    let mut blockers = artifact_provenance_blockers(
                        metadata.is_file(),
                        metadata.len(),
                        read_error.as_deref(),
                        source_fingerprint.as_deref(),
                        freshness_fingerprint.as_deref(),
                    );
                    blockers.extend(semantic_blockers);
                    ReleaseEvidenceArtifactStatus {
                        path: (*artifact).to_string(),
                        exists: metadata.is_file(),
                        bytes: metadata.len(),
                        read_error,
                        owner_lane,
                        generator_command: generator_command.clone(),
                        command_mode,
                        content_sha256,
                        source_fingerprint,
                        freshness_fingerprint,
                        blockers,
                    }
                }
                Err(error) => ReleaseEvidenceArtifactStatus {
                    path: (*artifact).to_string(),
                    exists: false,
                    bytes: 0,
                    read_error: Some(error.to_string()),
                    owner_lane,
                    generator_command: generator_command.clone(),
                    command_mode,
                    content_sha256: None,
                    source_fingerprint: None,
                    freshness_fingerprint: None,
                    blockers: artifact_provenance_blockers(
                        false,
                        0,
                        Some(&error.to_string()),
                        None,
                        None,
                    ),
                },
            }
        })
        .collect()
}

fn artifact_semantic_blockers(
    artifact: &str,
    bytes: &[u8],
    expected_generator_command: &str,
    command_mode: &str,
) -> Vec<String> {
    let mut blockers = crate::repo_boundary::public_artifact_boundary_blockers(artifact, bytes);
    if artifact.starts_with("release/evidence/dedup/") {
        blockers.extend(validate_duplicate_family_report_artifact(
            bytes,
            expected_generator_command,
        ));
        return blockers;
    }
    if artifact == PLAN_PROGRESS_ARTIFACT {
        blockers.extend(validate_plan_progress_artifact_bytes(bytes));
        return blockers;
    }
    if artifact == EXPECTED_ARTIFACT_REGISTRY {
        blockers.extend(expected_artifact_registry_blockers(bytes));
        return blockers;
    }
    if artifact == RESEARCH_AUDIT_ARTIFACT {
        blockers.extend(validate_research_audit_artifact_bytes(
            bytes,
            expected_generator_command,
        ));
        return blockers;
    }
    if artifact == FRONTIER_LEADERBOARD_ARTIFACT {
        match serde_json::from_slice::<serde_json::Value>(bytes) {
            Ok(value) => blockers.extend(external_benchmark_artifact_freshness_blockers(
                artifact,
                &value,
                expected_generator_command,
                command_mode,
            )),
            Err(error) => blockers.push(format!(
                "external benchmark artifact `{artifact}` is not valid JSON: {error}"
            )),
        }
        blockers.extend(crate::release_benchmarks::validate_frontier_leaderboard_artifact_bytes(
            bytes,
        ));
        return blockers;
    }
    if is_release_benchmark_semantic_artifact(artifact) {
        let value = match serde_json::from_slice::<serde_json::Value>(bytes) {
            Ok(value) => value,
            Err(error) => {
                blockers.push(format!(
                    "benchmark artifact `{artifact}` is not valid JSON: {error}"
                ));
                return blockers;
            }
        };
        blockers.extend(external_benchmark_artifact_freshness_blockers(
            artifact,
            &value,
            expected_generator_command,
            command_mode,
        ));
        blockers.extend(crate::benchmark_evidence_semantics::benchmark_evidence_blocker_issues(
            artifact, &value,
        ));
        return blockers;
    }
    blockers
}

fn external_benchmark_artifact_freshness_blockers(
    artifact: &str,
    value: &serde_json::Value,
    expected_generator_command: &str,
    command_mode: &str,
) -> Vec<String> {
    let mut blockers = Vec::new();
    if !expected_generator_command.starts_with("xtask release-benchmarks") {
        blockers.push(format!(
            "external benchmark artifact `{artifact}` generator_command `{expected_generator_command}` must start with `xtask release-benchmarks`"
        ));
    }
    if command_mode != COMMAND_MODE_EXTERNAL_ARTIFACTS_ONLY {
        blockers.push(format!(
            "external benchmark artifact `{artifact}` command_mode `{command_mode}` must be `{COMMAND_MODE_EXTERNAL_ARTIFACTS_ONLY}` so release-evidence inspects existing benchmark artifacts without spawning release-benchmarks"
        ));
    }
    if value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        == 0
    {
        blockers.push(format!(
            "external benchmark artifact `{artifact}` must declare positive schema_version"
        ));
    }
    if artifact.ends_with("cuda-release-suite.json") {
        blockers.extend(crate::benchmark_evidence_semantics::benchmark_schema_digest_chain_issues(
            artifact,
            value,
            "backend-suite",
        ));
        let chain = value.get("schema_digest_chain");
        for field in ["source_digest", "command_digest", "hardware_digest"] {
            if chain
                .and_then(|chain| chain.get(field))
                .and_then(serde_json::Value::as_str)
                .is_none_or(|digest| digest.trim().is_empty())
            {
                blockers.push(format!(
                    "external benchmark artifact `{artifact}` schema_digest_chain.{field} is blank or missing"
                ));
            }
        }
        let hardware_digest = value
            .get("hardware_digest")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !hardware_digest.starts_with("benchmark-hardware-digest:v1:") {
            blockers.push(format!(
                "external benchmark artifact `{artifact}` hardware_digest must be a benchmark-hardware-digest:v1 digest"
            ));
        }
    }
    blockers
}

fn is_release_benchmark_semantic_artifact(artifact: &str) -> bool {
    matches!(
        artifact,
        "release/evidence/benchmarks/cuda-release-suite.json"
            | "release/evidence/benchmarks/bench-release-axes.json"
            | "release/evidence/benchmarks/cpu-only-100x-proof.json"
            | "release/evidence/optimization/pass-family-benchmark-manifest.json"
    ) || artifact == FRONTIER_LEADERBOARD_ARTIFACT
}

pub(crate) fn generator_command(command_args: &[&str]) -> String {
    let mut command = String::from("xtask");
    for arg in command_args {
        command.push(' ');
        command.push_str(arg);
    }
    command
}

fn owner_lane_for_command(command_args: &[&str]) -> &'static str {
    match command_args.first().copied().unwrap_or_default() {
        "backend-matrix" | "conformance-matrix" => "driver_shared",
        "release-workload-matrix" | "release-benchmarks" => "bench_harness",
        "optimization-corpus" | "optimization-matrix" => "foundation_optimizer",
        "parser-coherence" => "parser_frontend",
        "weir-matrix" => "flow_weir",
        "source-similar" | "whats-similar" | "lego-audit" | "research-audit" => {
            "testing_evidence"
        }
        "hygiene-matrix" | "test-matrix" | "docs-matrix" | "release-evidence" => {
            "testing_evidence"
        }
        "acceleration-plan-gate" => "testing_evidence",
        "version-matrix" | "metadata-matrix" | "feature-matrix" => "coordination",
        _ => "coordination",
    }
}

fn artifact_provenance_fingerprints(
    artifact: &str,
    generator_command: &str,
    bytes: u64,
    content_sha256: Option<&str>,
) -> (Option<String>, Option<String>) {
    let Some(content_sha256) = content_sha256 else {
        return (None, None);
    };
    let source_material = format!(
        "release-evidence-source:v1\ngenerator={generator_command}\nartifact={artifact}\nbytes={bytes}\ncontent_sha256={content_sha256}\n"
    );
    let freshness_material = format!(
        "release-evidence-freshness:v1\nartifact={artifact}\ngenerator={generator_command}\nsource={}\n",
        sha256_hex(source_material.as_bytes())
    );
    (
        Some(format!(
            "release-evidence-source:v1:{}",
            sha256_hex(source_material.as_bytes())
        )),
        Some(format!(
            "release-evidence-freshness:v1:{}",
            sha256_hex(freshness_material.as_bytes())
        )),
    )
}

fn artifact_provenance_blockers(
    exists: bool,
    bytes: u64,
    read_error: Option<&str>,
    source_fingerprint: Option<&str>,
    freshness_fingerprint: Option<&str>,
) -> Vec<String> {
    let mut blockers = Vec::new();
    if !exists {
        blockers.push("artifact is missing or not a file".to_string());
    }
    if bytes == 0 {
        blockers.push("artifact is empty".to_string());
    }
    if let Some(error) = read_error {
        blockers.push(format!("artifact is unreadable: {error}"));
    }
    if source_fingerprint.is_none() {
        blockers.push("artifact is missing source_fingerprint".to_string());
    }
    if freshness_fingerprint.is_none() {
        blockers.push("artifact is missing freshness_fingerprint".to_string());
    }
    blockers
}

pub(crate) fn release_artifact_status_has_failure(status: &ReleaseEvidenceArtifactStatus) -> bool {
    !status.exists
        || status.bytes == 0
        || status.read_error.is_some()
        || status.source_fingerprint.is_none()
        || status.freshness_fingerprint.is_none()
        || !status.blockers.is_empty()
}

pub(crate) fn artifact_blocker_suffix(status: &ReleaseEvidenceArtifactStatus) -> String {
    if status.blockers.is_empty() {
        return status
            .read_error
            .as_ref()
            .map(|error| format!(": {error}"))
            .unwrap_or_default();
    }
    format!(": {}", status.blockers.join("; "))
}
