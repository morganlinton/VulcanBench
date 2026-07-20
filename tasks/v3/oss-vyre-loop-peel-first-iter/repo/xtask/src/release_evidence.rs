//! Generate cheap structural release evidence artifacts.
//!
//! Long-running artifacts remain explicit: benchmark suites and full
//! Linux corpus parsing are not launched here.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

mod artifact_status;
mod evidence_index;
mod expected_artifacts;

use crate::artifact_paths::{
    PLAN_PROGRESS_ARTIFACT, RESEARCH_AUDIT_ARTIFACT,
    LEGO_AUDIT_DUPLICATES_ARTIFACT, REGISTERED_OP_DUPLICATES_ARTIFACT,
    SOURCE_SIMILAR_DUPLICATES_ARTIFACT,
};
use artifact_status::{
    artifact_blocker_suffix, generator_command, inspect_expected_artifacts,
    inspect_expected_artifacts_with_mode,
    release_artifact_status_has_failure, ReleaseEvidenceArtifactStatus,
};
use expected_artifacts::{
    build_expected_artifact_registry, write_expected_artifact_registry,
    ReleaseExpectedArtifactCommand, ReleaseExpectedArtifactRegistry,
    COMMAND_MODE_EXTERNAL_ARTIFACTS_ONLY, COMMAND_MODE_SPAWNED, EXPECTED_ARTIFACT_REGISTRY,
    RELEASE_EVIDENCE_EXPECTED_ARTIFACTS, RELEASE_EVIDENCE_GENERATOR_COMMAND,
    RELEASE_EVIDENCE_RUN_ARTIFACT,
};
use evidence_index::{build_evidence_index, ReleaseEvidenceIndex};
pub(crate) use expected_artifacts::expected_artifacts_for_command;

const RELEASE_EVIDENCE_RUN_SCHEMA_VERSION: u32 = 4;

const COMMANDS: &[EvidenceCommand] = &[
    EvidenceCommand::required(&["docs-matrix"]),
    EvidenceCommand::required(&["version-matrix"]),
    EvidenceCommand::required(&["backend-matrix"]),
    EvidenceCommand::required(&["conformance-matrix"]),
    EvidenceCommand::required(&["release-workload-matrix", "--enforce"]),
    EvidenceCommand::external_required(&["release-benchmarks", "--backend", "cuda"]),
    EvidenceCommand::required(&["hygiene-matrix"]),
    EvidenceCommand::required(&["test-matrix"]),
    EvidenceCommand::required(&["metadata-matrix"]),
    EvidenceCommand::required(&["feature-matrix"]),
    EvidenceCommand::required(&["optimization-corpus"]),
    EvidenceCommand::required(&["optimization-matrix"]),
    EvidenceCommand::required(&["parser-coherence"]),
    EvidenceCommand::required(&["weir-matrix"]),
    EvidenceCommand::required(&[
        "source-similar",
        "--duplicate-report-json",
        SOURCE_SIMILAR_DUPLICATES_ARTIFACT,
    ]),
    EvidenceCommand::required(&[
        "whats-similar",
        "--all",
        "--duplicate-report-json",
        REGISTERED_OP_DUPLICATES_ARTIFACT,
    ]),
    EvidenceCommand::required(&[
        "lego-audit",
        "--report-only",
        "--duplicate-report-json",
        LEGO_AUDIT_DUPLICATES_ARTIFACT,
    ]),
    EvidenceCommand::required(&[
        "acceleration-plan-gate",
        "--progress-json",
        PLAN_PROGRESS_ARTIFACT,
    ]),
    EvidenceCommand::required(&[
        "research-audit",
        "--output",
        RESEARCH_AUDIT_ARTIFACT,
    ]),
];

struct EvidenceCommand {
    args: &'static [&'static str],
    required: bool,
    run: bool,
}

#[derive(Debug, Serialize)]
struct ReleaseEvidenceRun {
    schema_version: u32,
    total_commands: usize,
    successful_commands: usize,
    command_failures: usize,
    artifact_failures: usize,
    command_count: usize,
    required_command_count: usize,
    report_only_command_count: usize,
    commands: Vec<ReleaseEvidenceCommandRecord>,
    final_artifacts: Vec<ReleaseEvidenceArtifactStatus>,
    evidence_index: ReleaseEvidenceIndex,
    expected_artifact_registry: ReleaseExpectedArtifactRegistry,
    blockers: Vec<String>,
    reports: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ReleaseEvidenceCommandRecord {
    args: Vec<&'static str>,
    required: bool,
    expected_artifacts: Vec<&'static str>,
    status: String,
    exit_code: Option<i32>,
    artifact_statuses: Vec<ReleaseEvidenceArtifactStatus>,
}

impl EvidenceCommand {
    const fn required(args: &'static [&'static str]) -> Self {
        Self {
            args,
            required: true,
            run: true,
        }
    }

    const fn external_required(args: &'static [&'static str]) -> Self {
        Self {
            args,
            required: true,
            run: false,
        }
    }
}

pub(crate) fn run(_args: &[String]) {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let mut failures = Vec::new();
    let xtask = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("release-evidence: failed to locate current xtask binary: {error}");
            std::process::exit(1);
        }
    };
    let mut reports = Vec::new();
    let mut records = Vec::new();
    for command in COMMANDS {
        let status = command.run.then(|| {
            Command::new(&xtask)
                .args(command.args)
                .current_dir(&workspace_root)
                .status()
        });
        let expected = expected_artifacts(command.args);
        if command.required && expected.is_empty() {
            failures.push(format!(
                "`xtask {}` is required but declares no expected artifacts",
                command.args.join(" ")
            ));
        }
        let command_mode = if command.run {
            COMMAND_MODE_SPAWNED
        } else {
            COMMAND_MODE_EXTERNAL_ARTIFACTS_ONLY
        };
        let artifact_statuses =
            inspect_expected_artifacts_with_mode(&workspace_root, command.args, expected, command_mode);
        let status_text = command_status_text(status.as_ref());
        let exit_code = status
            .as_ref()
            .and_then(|status| status.as_ref().ok())
            .and_then(std::process::ExitStatus::code);
        for artifact in &artifact_statuses {
            if release_artifact_status_has_failure(artifact) {
                let finding = format!(
                    "`xtask {}` expected `{}` but it was missing, empty, unreadable, or missing provenance{}",
                    command.args.join(" "),
                    artifact.path,
                    artifact_blocker_suffix(artifact)
                );
                if command.required {
                    failures.push(finding);
                } else {
                    reports.push(finding);
                }
            }
        }
        records.push(ReleaseEvidenceCommandRecord {
            args: command.args.to_vec(),
            required: command.required,
            expected_artifacts: expected.to_vec(),
            status: status_text,
            exit_code,
            artifact_statuses,
        });
        match status {
            Some(Ok(status)) if status.success() => {}
            Some(Ok(status)) if command.required => failures.push(format!(
                "`xtask {}` failed with {status}",
                command.args.join(" ")
            )),
            Some(Ok(status)) => reports.push(format!(
                "`xtask {}` reported {status}; artifact was still written for review",
                command.args.join(" ")
            )),
            Some(Err(error)) if command.required => failures.push(format!(
                "failed to run `xtask {}`: {error}",
                command.args.join(" ")
            )),
            Some(Err(error)) => reports.push(format!(
                "failed to run report-only `xtask {}`: {error}",
                command.args.join(" ")
            )),
            None => reports.push(format!(
                "`xtask {}` was not run by release-evidence; existing explicit artifacts were inspected",
                command.args.join(" ")
            )),
        }
    }
    let final_artifact_failures =
        write_release_evidence_run(&workspace_root, records, &failures, &reports);
    failures.extend(final_artifact_failures);
    if failures.is_empty() {
        for report in &reports {
            eprintln!("release-evidence: {report}");
        }
        println!(
            "release-evidence: structural evidence generated; run `cargo_full run --bin xtask -- release-benchmarks --backend cuda` separately to refresh benchmark artifacts"
        );
    } else {
        eprintln!("release-evidence: {} blocker(s):", failures.len());
        for failure in &failures {
            eprintln!("  - {failure}");
        }
        std::process::exit(1);
    }
}

fn command_status_text(status: Option<&std::io::Result<std::process::ExitStatus>>) -> String {
    match status {
        Some(Ok(status)) if status.success() => "success".to_string(),
        Some(Ok(status)) => format!("failed: {status}"),
        Some(Err(error)) => format!("spawn error: {error}"),
        None => "external-artifacts-only".to_string(),
    }
}

fn expected_artifacts(args: &[&str]) -> &'static [&'static str] {
    expected_artifacts_for_command(args.first().copied().unwrap_or_default())
}

fn write_release_evidence_run(
    workspace_root: &Path,
    commands: Vec<ReleaseEvidenceCommandRecord>,
    blockers: &[String],
    reports: &[String],
) -> Vec<String> {
    let output = workspace_root.join(RELEASE_EVIDENCE_RUN_ARTIFACT);
    if let Some(parent) = output.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!(
                "release-evidence: failed to create `{}`: {error}",
                parent.display()
            );
            std::process::exit(1);
        }
    }
    let required_command_count = commands.iter().filter(|command| command.required).count();
    let report_only_command_count = commands.len().saturating_sub(required_command_count);
    let successful_commands = commands
        .iter()
        .filter(|command| command.status == "success" || command.status == "external-artifacts-only")
        .count();
    let expected_artifact_registry = build_expected_artifact_registry(
        commands
            .iter()
            .map(|command| {
                ReleaseExpectedArtifactCommand::new_with_mode(
                    generator_command(&command.args),
                    registry_command_mode(&command.status).to_string(),
                    command.required,
                    command
                        .expected_artifacts
                        .iter()
                        .map(|artifact| (*artifact).to_string())
                        .collect(),
                )
            })
            .collect(),
    );
    write_expected_artifact_registry(workspace_root, &expected_artifact_registry);
    let final_artifacts = inspect_expected_artifacts(
        workspace_root,
        &["release-evidence"],
        &[EXPECTED_ARTIFACT_REGISTRY],
    );
    let artifact_failures = commands
        .iter()
        .flat_map(|command| &command.artifact_statuses)
        .chain(final_artifacts.iter())
        .filter(|artifact| release_artifact_status_has_failure(artifact))
        .count();
    let evidence_index = build_evidence_index(
        commands
            .iter()
            .flat_map(|command| command.artifact_statuses.iter())
            .chain(final_artifacts.iter()),
    );
    let final_artifact_failures = final_artifacts
        .iter()
        .filter(|artifact| release_artifact_status_has_failure(artifact))
        .map(|artifact| {
            format!(
                "`{RELEASE_EVIDENCE_GENERATOR_COMMAND}` expected final artifact `{}` but it was missing, empty, unreadable, or missing provenance{}",
                artifact.path,
                artifact_blocker_suffix(artifact)
            )
        })
        .collect::<Vec<_>>();
    let mut combined_blockers = blockers.to_vec();
    combined_blockers.extend(final_artifact_failures.iter().cloned());
    let run = ReleaseEvidenceRun {
        schema_version: RELEASE_EVIDENCE_RUN_SCHEMA_VERSION,
        total_commands: commands.len(),
        successful_commands,
        command_failures: commands.len().saturating_sub(successful_commands),
        artifact_failures,
        command_count: commands.len(),
        required_command_count,
        report_only_command_count,
        commands,
        final_artifacts,
        evidence_index,
        expected_artifact_registry,
        blockers: combined_blockers,
        reports: reports.to_vec(),
    };
    let json = match serde_json::to_string_pretty(&run) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("release-evidence: failed to serialize run evidence: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = fs::write(&output, format!("{json}\n")) {
        eprintln!(
            "release-evidence: failed to write `{}`: {error}",
            output.display()
        );
        std::process::exit(1);
    }
    final_artifact_failures
}

fn registry_command_mode(status: &str) -> &'static str {
    if status == COMMAND_MODE_EXTERNAL_ARTIFACTS_ONLY {
        COMMAND_MODE_EXTERNAL_ARTIFACTS_ONLY
    } else {
        COMMAND_MODE_SPAWNED
    }
}

#[cfg(test)]
mod tests {
    use super::expected_artifacts::expected_artifact_registry_blockers;
    use super::*;

    #[test]
    fn artifact_status_records_generator_owner_and_fingerprints() {
        let tmp = tempfile::tempdir().unwrap();
        let artifact = tmp.path().join("release/evidence/docs/docs-matrix.json");
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(&artifact, b"{\"blockers\":[]}\n").unwrap();

        let statuses = inspect_expected_artifacts(
            tmp.path(),
            &["docs-matrix"],
            &["release/evidence/docs/docs-matrix.json"],
        );

        assert_eq!(statuses.len(), 1);
        let status = &statuses[0];
        assert_eq!(status.owner_lane, "testing_evidence");
        assert_eq!(status.generator_command, "xtask docs-matrix");
        assert_eq!(status.command_mode, COMMAND_MODE_SPAWNED);
        assert_eq!(status.content_sha256.as_deref().map(str::len), Some(64));
        assert!(status
            .source_fingerprint
            .as_deref()
            .is_some_and(|value| value.starts_with("release-evidence-source:v1:")));
        assert!(status
            .freshness_fingerprint
            .as_deref()
            .is_some_and(|value| value.starts_with("release-evidence-freshness:v1:")));
        assert!(status.blockers.is_empty(), "{:?}", status.blockers);
    }

    #[test]
    fn artifact_status_rejects_public_boundary_leaks() {
        let tmp = tempfile::tempdir().unwrap();
        let artifact = tmp.path().join("release/evidence/docs/docs-matrix.json");
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(
            &artifact,
            br#"{"blockers":[],"repositories_public":["santhsecurity/vyre"],"public_repository":"santhsecurity/Santh","path":"/media/mukund-thiru/SanthData/Santh/private.json","command":"gh repo edit Santh --visibility public","env":"VYRE_RELEASE_REPOS=santhsecurity/vyre","provenance":"token=abc"}"#,
        )
        .unwrap();

        let statuses = inspect_expected_artifacts(
            tmp.path(),
            &["docs-matrix"],
            &["release/evidence/docs/docs-matrix.json"],
        );

        let blockers = &statuses[0].blockers;
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("repositories_public")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("private Santh path")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("non-Vyre public repository")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("credential-looking")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("visibility mutation")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("VYRE_RELEASE_REPOS")));
    }

    #[test]
    fn weir_matrix_artifacts_are_owned_by_flow_lane() {
        let tmp = tempfile::tempdir().unwrap();
        for artifact in expected_artifacts(&["weir-matrix"]) {
            let artifact_path = tmp.path().join(artifact);
            std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
            std::fs::write(&artifact_path, b"{\"blockers\":[]}\n").unwrap();
        }

        let statuses = inspect_expected_artifacts(
            tmp.path(),
            &["weir-matrix"],
            expected_artifacts(&["weir-matrix"]),
        );

        assert_eq!(statuses.len(), 4);
        assert!(statuses.iter().any(|status| {
            status.path == "release/evidence/weir/weir-flow-release-contracts.json"
        }));
        for status in &statuses {
            assert_eq!(status.owner_lane, "flow_weir");
            assert_eq!(status.generator_command, "xtask weir-matrix");
            assert_eq!(status.command_mode, COMMAND_MODE_SPAWNED);
            assert!(status.source_fingerprint.is_some());
            assert!(status.freshness_fingerprint.is_some());
            assert!(status.blockers.is_empty(), "{:?}", status.blockers);
        }
    }

    #[test]
    fn acceleration_plan_progress_artifact_is_release_indexed_with_freshness() {
        let tmp = tempfile::tempdir().unwrap();
        let artifact = tmp.path().join(PLAN_PROGRESS_ARTIFACT);
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(&artifact, valid_plan_progress_artifact_fixture()).unwrap();

        let statuses = inspect_expected_artifacts(
            tmp.path(),
            &[
                "acceleration-plan-gate",
                "--progress-json",
                PLAN_PROGRESS_ARTIFACT,
            ],
            expected_artifacts(&["acceleration-plan-gate"]),
        );

        assert_eq!(expected_artifacts(&["acceleration-plan-gate"]), &[PLAN_PROGRESS_ARTIFACT]);
        assert_eq!(statuses.len(), 1);
        let status = &statuses[0];
        assert_eq!(status.owner_lane, "testing_evidence");
        assert_eq!(
            status.generator_command,
            format!("xtask acceleration-plan-gate --progress-json {PLAN_PROGRESS_ARTIFACT}")
        );
        assert_eq!(status.command_mode, COMMAND_MODE_SPAWNED);
        assert!(status
            .freshness_fingerprint
            .as_deref()
            .is_some_and(|value| value.starts_with("release-evidence-freshness:v1:")));
        assert!(status.blockers.is_empty(), "{:?}", status.blockers);
    }

    #[test]
    fn duplicate_family_reports_are_release_indexed() {
        let tmp = tempfile::tempdir().unwrap();
        for artifact in expected_artifacts(&["whats-similar"]) {
            let artifact_path = tmp.path().join(artifact);
            std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
            std::fs::write(
                &artifact_path,
                b"{\"schema_version\":2,\"generator_command\":\"xtask whats-similar --all --duplicate-report-json release/evidence/dedup/registered-op-duplicates.json\",\"family_count\":0,\"families\":[]}\n",
            )
            .unwrap();
        }

        let statuses = inspect_expected_artifacts(
            tmp.path(),
            &[
                "whats-similar",
                "--all",
                "--duplicate-report-json",
                "release/evidence/dedup/registered-op-duplicates.json",
            ],
            expected_artifacts(&["whats-similar"]),
        );

        assert_eq!(
            expected_artifacts(&["source-similar"]),
            &["release/evidence/dedup/source-similar-duplicates.json"]
        );
        assert_eq!(
            expected_artifacts(&["lego-audit"]),
            &["release/evidence/dedup/lego-audit-duplicates.json"]
        );
        assert_eq!(statuses.len(), 1);
        let status = &statuses[0];
        assert_eq!(status.owner_lane, "testing_evidence");
        assert_eq!(
            status.generator_command,
            "xtask whats-similar --all --duplicate-report-json release/evidence/dedup/registered-op-duplicates.json"
        );
        assert_eq!(status.command_mode, COMMAND_MODE_SPAWNED);
        assert!(status.source_fingerprint.is_some());
        assert!(status.freshness_fingerprint.is_some());
        assert!(status.blockers.is_empty(), "{:?}", status.blockers);
    }

    #[test]
    fn duplicate_family_reports_require_schema_v2_and_subject_fingerprints() {
        let tmp = tempfile::tempdir().unwrap();
        let artifact = tmp
            .path()
            .join("release/evidence/dedup/registered-op-duplicates.json");
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(
            &artifact,
            br#"{
  "schema_version": 1,
  "family_count": 1,
  "families": [
    {
      "left": {"id": "left"},
      "right": {"id": "right"}
    }
  ]
}
"#,
        )
        .unwrap();

        let statuses = inspect_expected_artifacts(
            tmp.path(),
            &["whats-similar"],
            &["release/evidence/dedup/registered-op-duplicates.json"],
        );

        let blockers = &statuses[0].blockers;
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("schema_version=2")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("generator_command")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("family[0].family_id")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("family[0].detector")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("left.fingerprint")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("right.fingerprint")));
    }

    #[test]
    fn external_release_benchmark_status_requires_external_mode_and_digest_chain() {
        let tmp = tempfile::tempdir().unwrap();
        let artifact_rel = "release/evidence/benchmarks/cuda-release-suite.json";
        let artifact = tmp.path().join(artifact_rel);
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(
            &artifact,
            br#"{
  "schema_version": 3,
  "blockers": ["stale source fingerprint"],
  "artifact_statuses": []
}
"#,
        )
        .unwrap();

        let spawned_statuses = inspect_expected_artifacts(
            tmp.path(),
            &["release-benchmarks", "--backend", "cuda"],
            &[artifact_rel],
        );
        let spawned_blockers = &spawned_statuses[0].blockers;
        assert!(spawned_blockers
            .iter()
            .any(|blocker| blocker.contains("command_mode `spawned`")));

        let external_statuses = inspect_expected_artifacts_with_mode(
            tmp.path(),
            &["release-benchmarks", "--backend", "cuda"],
            &[artifact_rel],
            COMMAND_MODE_EXTERNAL_ARTIFACTS_ONLY,
        );
        let external_blockers = &external_statuses[0].blockers;
        assert_eq!(
            external_statuses[0].command_mode,
            COMMAND_MODE_EXTERNAL_ARTIFACTS_ONLY
        );
        assert!(external_blockers
            .iter()
            .all(|blocker| !blocker.contains("command_mode `spawned`")));
        assert!(external_blockers
            .iter()
            .any(|blocker| blocker.contains("stale source fingerprint")));
        assert!(external_blockers
            .iter()
            .any(|blocker| blocker.contains("schema_digest_chain.source_digest")));
        assert!(external_blockers
            .iter()
            .any(|blocker| blocker.contains("schema_digest_chain.command_digest")));
        assert!(external_blockers
            .iter()
            .any(|blocker| blocker.contains("schema_digest_chain.hardware_digest")));
        assert!(external_blockers
            .iter()
            .any(|blocker| blocker.contains("hardware_digest")));
    }

    #[test]
    fn plan_progress_artifact_requires_summary_consistency() {
        let tmp = tempfile::tempdir().unwrap();
        let artifact = tmp.path().join(PLAN_PROGRESS_ARTIFACT);
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(
            &artifact,
            b"{\"schema_version\":2,\"row_count\":3,\"dedup_seam_count\":2,\"evidence_path_count\":0,\"axis_row_counts\":{\"coordination\":1},\"research_key_counts\":{},\"rows\":[]}\n",
        )
        .unwrap();

        let statuses = inspect_expected_artifacts(
            tmp.path(),
            &[
                "acceleration-plan-gate",
                "--progress-json",
                PLAN_PROGRESS_ARTIFACT,
            ],
            &[PLAN_PROGRESS_ARTIFACT],
        );

        let blockers = &statuses[0].blockers;
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("schema_version=4")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("dedup_seam_count")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("evidence_path_count")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("axis_row_counts")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("research_key_counts")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("row_count must be at least")));
    }

    fn valid_plan_progress_artifact_fixture() -> String {
        let row_count = crate::vx_plan_table::VX_PLAN_MIN_ROWS;
        let rows = (1..=row_count)
            .map(|index| {
                format!(
                    r#"{{"id":"VX-{index:03}","axis":"coordination","proof_gate":"Gate test rejects malformed rows {index}.","dedup_seam":"Plan progress fixture seam {index}.","status":"active","linked_release_artifact":"{PLAN_PROGRESS_ARTIFACT}"}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"schema_version":4,"row_count":{row_count},"research_grounded_row_count":{row_count},"dedup_seam_count":{row_count},"duplicate_dedup_seam_count":0,"duplicate_dedup_seams":[],"evidence_path_count":42,"duplicate_evidence_path_count":0,"duplicate_evidence_paths":[],"axis_row_counts":{{"coordination":{row_count}}},"research_key_counts":{{"MLIR_PASS":{row_count}}},"rows":[{rows}]}}
"#
        )
    }

    #[test]
    fn expected_artifact_registry_counts_commands_and_artifacts() {
        let registry = build_expected_artifact_registry(vec![ReleaseExpectedArtifactCommand::new(
            "xtask docs-matrix".to_string(),
            true,
            vec![
                "release/evidence/docs/docs-matrix.json",
                "release/evidence/docs/vyre-readme-contracts.json",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        )]);

        assert_eq!(registry.schema_version, 2);
        assert_eq!(registry.command_count, 2);
        assert_eq!(registry.artifact_count, 4);
        assert_eq!(registry.commands[0].generator_command, "xtask docs-matrix");
        assert!(registry.commands[0].required);
        assert_eq!(
            registry.commands[1].expected_artifacts,
            vec![
                RELEASE_EVIDENCE_RUN_ARTIFACT.to_string(),
                EXPECTED_ARTIFACT_REGISTRY.to_string()
            ]
        );
    }

    #[test]
    fn expected_artifact_registry_validation_rejects_drift() {
        let blockers = expected_artifact_registry_blockers(
            b"{\"schema_version\":0,\"command_count\":2,\"artifact_count\":9,\"commands\":[{\"generator_command\":\"xtask docs-matrix\",\"expected_artifacts\":[]}]}\n",
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("schema_version=2")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("command_count")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("required")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("artifact_contracts")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("artifact_count")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains(RELEASE_EVIDENCE_GENERATOR_COMMAND)));
    }

    #[test]
    fn evidence_index_includes_final_release_evidence_artifacts() {
        let final_artifact = ReleaseEvidenceArtifactStatus {
            path: EXPECTED_ARTIFACT_REGISTRY.to_string(),
            exists: true,
            bytes: 64,
            read_error: None,
            owner_lane: "testing_evidence",
            generator_command: RELEASE_EVIDENCE_GENERATOR_COMMAND.to_string(),
            command_mode: COMMAND_MODE_SPAWNED,
            content_sha256: Some("a".repeat(64)),
            source_fingerprint: Some("release-evidence-source:v1:abc".to_string()),
            freshness_fingerprint: Some("release-evidence-freshness:v1:def".to_string()),
            blockers: Vec::new(),
        };

        let final_artifacts = [final_artifact];
        let index = build_evidence_index(final_artifacts.iter());

        assert_eq!(index.artifact_count, 1);
        assert_eq!(index.artifacts[0].path, EXPECTED_ARTIFACT_REGISTRY);
    }

    #[test]
    fn evidence_index_surfaces_missing_provenance_blockers() {
        let record = ReleaseEvidenceCommandRecord {
            args: vec!["docs-matrix"],
            required: true,
            expected_artifacts: vec!["release/evidence/docs/docs-matrix.json"],
            status: "success".to_string(),
            exit_code: Some(0),
            artifact_statuses: vec![ReleaseEvidenceArtifactStatus {
                path: "release/evidence/docs/docs-matrix.json".to_string(),
                exists: true,
                bytes: 12,
                read_error: None,
                owner_lane: "testing_evidence",
                generator_command: "xtask docs-matrix".to_string(),
                command_mode: COMMAND_MODE_SPAWNED,
                content_sha256: None,
                source_fingerprint: None,
                freshness_fingerprint: None,
                blockers: vec!["artifact is missing source_fingerprint".to_string()],
            }],
        };

        let records = [record];
        let index = build_evidence_index(
            records
                .iter()
                .flat_map(|record| record.artifact_statuses.iter()),
        );

        assert_eq!(index.artifact_count, 1);
        assert_eq!(index.blockers.len(), 1);
        assert!(index.blockers[0].contains("source_fingerprint"));
    }
}
