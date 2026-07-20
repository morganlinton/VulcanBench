//! Final release checklist validation.

use std::collections::BTreeSet;

/// Required release checklist class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReleaseChecklistClass {
    /// cargo_full checks and tests.
    CargoFull,
    /// GPU tests and explicit probes.
    GpuTests,
    /// Fuzz and hostile input evidence.
    FuzzHostile,
    /// Gap findings are tracked or fixed.
    GapFindings,
    /// Benchmarks and thresholds.
    Benchmarks,
    /// Release and contributor docs.
    Docs,
    /// Crate metadata readiness.
    CrateMetadata,
    /// Public API and doctest review.
    PublicApiReview,
    /// Deep personal review.
    DeepReview,
}

/// One release checklist evidence item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseChecklistEvidence<'a> {
    /// Checklist class.
    pub class: ReleaseChecklistClass,
    /// Exact command or artifact path.
    pub evidence: &'a str,
    /// Whether this checklist class is green.
    pub green: bool,
}

/// Release checklist proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseChecklistProof {
    /// Number of required checklist classes.
    pub class_count: usize,
}

/// Validated final release evidence run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseEvidenceRunProof {
    /// Required command count recorded in the run artifact.
    pub required_command_count: u64,
}

/// Release checklist validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseChecklistError {
    /// No evidence supplied.
    EmptyEvidence,
    /// Evidence field is empty.
    EmptyEvidenceField {
        /// Checklist class.
        class: ReleaseChecklistClass,
    },
    /// A required class is missing.
    MissingClass {
        /// Missing class.
        class: ReleaseChecklistClass,
    },
    /// A checklist class is not green.
    NotGreen {
        /// Checklist class.
        class: ReleaseChecklistClass,
    },
    /// cargo_full class lacks cargo_full command evidence.
    CargoFullMissingCommand,
    /// GPU class lacks probe or CUDA evidence.
    GpuMissingProbeEvidence,
    /// Final evidence-run artifact is missing required evidence.
    EvidenceRunMissingField {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Final evidence-run artifact contains a nonzero failure count.
    EvidenceRunHasFailures {
        /// Failure field.
        field: &'static str,
        /// Observed value.
        observed: u64,
    },
}

impl std::fmt::Display for ReleaseChecklistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyEvidence => write!(
                f,
                "release checklist evidence is empty. Fix: collect cargo_full, GPU, fuzz, gap, benchmark, docs, metadata, API, and deep-review evidence before publish."
            ),
            Self::EmptyEvidenceField { class } => write!(
                f,
                "release checklist {class:?} has empty evidence. Fix: record exact command or artifact path."
            ),
            Self::MissingClass { class } => write!(
                f,
                "release checklist is missing {class:?}. Fix: make that release gate green before publishing."
            ),
            Self::NotGreen { class } => write!(
                f,
                "release checklist {class:?} is not green. Fix: do not publish until the class is green."
            ),
            Self::CargoFullMissingCommand => write!(
                f,
                "release checklist CargoFull evidence does not contain ./cargo_full. Fix: use cargo_full for release validation."
            ),
            Self::GpuMissingProbeEvidence => write!(
                f,
                "release checklist GpuTests evidence does not contain CUDA/NVIDIA/nvidia-smi details. Fix: record explicit GPU probe evidence."
            ),
            Self::EvidenceRunMissingField { evidence } => write!(
                f,
                "release evidence run is missing {evidence}. Fix: regenerate final release evidence run before publishing."
            ),
            Self::EvidenceRunHasFailures { field, observed } => write!(
                f,
                "release evidence run has {field}={observed}. Fix: every required command and artifact must be green before publishing."
            ),
        }
    }
}

impl std::error::Error for ReleaseChecklistError {}

const REQUIRED_CLASSES: &[ReleaseChecklistClass] = &[
    ReleaseChecklistClass::CargoFull,
    ReleaseChecklistClass::GpuTests,
    ReleaseChecklistClass::FuzzHostile,
    ReleaseChecklistClass::GapFindings,
    ReleaseChecklistClass::Benchmarks,
    ReleaseChecklistClass::Docs,
    ReleaseChecklistClass::CrateMetadata,
    ReleaseChecklistClass::PublicApiReview,
    ReleaseChecklistClass::DeepReview,
];

/// Validate the final release checklist before publish sequencing.
pub fn validate_release_checklist(
    evidence: &[ReleaseChecklistEvidence<'_>],
) -> Result<ReleaseChecklistProof, ReleaseChecklistError> {
    if evidence.is_empty() {
        return Err(ReleaseChecklistError::EmptyEvidence);
    }

    let mut classes = BTreeSet::new();
    for item in evidence {
        if item.evidence.trim().is_empty() {
            return Err(ReleaseChecklistError::EmptyEvidenceField { class: item.class });
        }
        if !item.green {
            return Err(ReleaseChecklistError::NotGreen { class: item.class });
        }
        if item.class == ReleaseChecklistClass::CargoFull && !item.evidence.contains("./cargo_full")
        {
            return Err(ReleaseChecklistError::CargoFullMissingCommand);
        }
        if item.class == ReleaseChecklistClass::GpuTests
            && !item.evidence.contains("nvidia-smi")
            && !item.evidence.contains("CUDA")
            && !item.evidence.contains("NVIDIA")
        {
            return Err(ReleaseChecklistError::GpuMissingProbeEvidence);
        }
        classes.insert(item.class);
    }

    for class in REQUIRED_CLASSES {
        if !classes.contains(class) {
            return Err(ReleaseChecklistError::MissingClass { class: *class });
        }
    }

    Ok(ReleaseChecklistProof {
        class_count: classes.len(),
    })
}

/// Validate the committed final release evidence-run artifact.
pub fn validate_release_evidence_run(
    artifact: &str,
) -> Result<ReleaseEvidenceRunProof, ReleaseChecklistError> {
    for (evidence, needle) in [
        ("schema version", "\"schema_version\""),
        ("docs matrix command", "\"docs-matrix\""),
        ("version matrix command", "\"version-matrix\""),
        ("backend matrix command", "\"backend-matrix\""),
        ("conformance matrix command", "\"conformance-matrix\""),
        (
            "release workload matrix command",
            "\"release-workload-matrix\"",
        ),
        ("hygiene matrix command", "\"hygiene-matrix\""),
        ("test matrix command", "\"test-matrix\""),
        ("optimization corpus command", "\"optimization-corpus\""),
        ("optimization matrix command", "\"optimization-matrix\""),
        ("parser coherence command", "\"parser-coherence\""),
        ("C parser corpus command", "\"c-parser-corpus\""),
        (
            "dataflow analysis matrix command",
            "\"dataflow-analysis-matrix\"",
        ),
        ("metadata matrix command", "\"metadata-matrix\""),
        ("feature matrix command", "\"feature-matrix\""),
        (
            "release completion audit command",
            "\"release-completion-audit\"",
        ),
        (
            "CUDA release suite artifact",
            "release/evidence/benchmarks/cuda-release-suite.json",
        ),
        (
            "CUDA PTX pattern artifact",
            "release/evidence/benchmarks/cuda-ptx-patterns.json",
        ),
        (
            "C parser Linux subsystem artifact",
            "release/evidence/parser/c-parser-linux-subsystem.json",
        ),
        (
            "completion audit artifact",
            "release/evidence/final/completion-audit.json",
        ),
        (
            "release tag plan artifact",
            "release/evidence/version/release-tag-plan.json",
        ),
    ] {
        if !artifact.contains(needle) {
            return Err(ReleaseChecklistError::EvidenceRunMissingField { evidence });
        }
    }
    require_artifact_contains_any(
        artifact,
        "successful command status",
        &["\"status\": \"success\"", "\"status\":\"success\""],
    )?;
    require_artifact_contains_any(
        artifact,
        "artifact exists records",
        &["\"exists\": true", "\"exists\":true"],
    )?;

    for field in [
        "command_failures",
        "artifact_failures",
        "report_only_command_count",
    ] {
        let value = artifact_number_field(artifact, field)?;
        if value != 0 {
            return Err(ReleaseChecklistError::EvidenceRunHasFailures {
                field,
                observed: value,
            });
        }
    }
    let required_command_count = artifact_number_field(artifact, "required_command_count")?;
    let successful_commands = artifact_number_field(artifact, "successful_commands")?;
    if successful_commands < required_command_count {
        return Err(ReleaseChecklistError::EvidenceRunHasFailures {
            field: "successful_commands",
            observed: successful_commands,
        });
    }

    Ok(ReleaseEvidenceRunProof {
        required_command_count,
    })
}

fn require_artifact_contains_any(
    artifact: &str,
    evidence: &'static str,
    needles: &[&str],
) -> Result<(), ReleaseChecklistError> {
    if needles.iter().any(|needle| artifact.contains(needle)) {
        Ok(())
    } else {
        Err(ReleaseChecklistError::EvidenceRunMissingField { evidence })
    }
}

fn artifact_number_field(
    artifact: &str,
    field: &'static str,
) -> Result<u64, ReleaseChecklistError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(ReleaseChecklistError::EvidenceRunMissingField { evidence: field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(ReleaseChecklistError::EvidenceRunMissingField { evidence: field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(ReleaseChecklistError::EvidenceRunMissingField { evidence: field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| ReleaseChecklistError::EvidenceRunMissingField { evidence: field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_checklist_accepts_all_green_required_classes() {
        let proof = validate_release_checklist(&evidence())
            .expect("Fix: complete green release checklist should pass");

        assert_eq!(proof.class_count, 9);
    }

    #[test]
    fn release_checklist_rejects_missing_deep_review() {
        let mut evidence = evidence();
        evidence.pop();

        assert_eq!(
            validate_release_checklist(&evidence).expect_err("missing class should fail"),
            ReleaseChecklistError::MissingClass {
                class: ReleaseChecklistClass::DeepReview,
            }
        );
    }

    #[test]
    fn release_checklist_rejects_non_green_and_missing_gpu_probe() {
        let mut not_green = evidence();
        not_green[0].green = false;
        assert_eq!(
            validate_release_checklist(&not_green).expect_err("not-green class should fail"),
            ReleaseChecklistError::NotGreen {
                class: ReleaseChecklistClass::CargoFull,
            }
        );

        let mut no_gpu_probe = evidence();
        no_gpu_probe[1].evidence = "gpu tests passed";
        assert_eq!(
            validate_release_checklist(&no_gpu_probe).expect_err("missing GPU probe should fail"),
            ReleaseChecklistError::GpuMissingProbeEvidence
        );
    }

    #[test]
    fn release_checklist_accepts_complete_final_evidence_run() {
        let proof = validate_release_evidence_run(
            r#"{
              "schema_version": 2,
              "docs-matrix": true,
              "version-matrix": true,
              "backend-matrix": true,
              "conformance-matrix": true,
              "release-workload-matrix": true,
              "hygiene-matrix": true,
              "test-matrix": true,
              "optimization-corpus": true,
              "optimization-matrix": true,
              "parser-coherence": true,
              "c-parser-corpus": true,
              "dataflow-analysis-matrix": true,
              "metadata-matrix": true,
              "feature-matrix": true,
              "release-completion-audit": true,
              "status": "success",
              "exists": true,
              "release/evidence/benchmarks/cuda-release-suite.json": {"exists": true},
              "release/evidence/benchmarks/cuda-ptx-patterns.json": {"exists": true},
              "release/evidence/parser/c-parser-linux-subsystem.json": {"exists": true},
              "release/evidence/final/completion-audit.json": {"exists": true},
              "release/evidence/version/release-tag-plan.json": {"exists": true},
              "command_failures": 0,
              "artifact_failures": 0,
              "report_only_command_count": 0,
              "required_command_count": 15,
              "successful_commands": 15
            }"#,
        )
        .expect("Fix: complete final evidence run should be green");

        assert!(proof.required_command_count >= 10);
    }

    #[test]
    fn release_checklist_rejects_stale_committed_final_evidence_run() {
        let err = validate_release_evidence_run(include_str!(
            "../../../../release/evidence/final/release-evidence-run.json"
        ))
        .expect_err("stale committed final evidence run must not satisfy 100-item checklist");

        assert_eq!(
            err,
            ReleaseChecklistError::EvidenceRunMissingField {
                evidence: "C parser corpus command",
            }
        );
    }

    #[test]
    fn release_checklist_rejects_evidence_run_failures() {
        let err = validate_release_evidence_run(
            r#"{"schema_version":2,"docs-matrix":true,"version-matrix":true,"backend-matrix":true,"conformance-matrix":true,"release-workload-matrix":true,"hygiene-matrix":true,"test-matrix":true,"optimization-corpus":true,"optimization-matrix":true,"parser-coherence":true,"c-parser-corpus":true,"dataflow-analysis-matrix":true,"metadata-matrix":true,"feature-matrix":true,"release-completion-audit":true,"release/evidence/benchmarks/cuda-release-suite.json":true,"release/evidence/benchmarks/cuda-ptx-patterns.json":true,"release/evidence/parser/c-parser-linux-subsystem.json":true,"release/evidence/final/completion-audit.json":true,"release/evidence/version/release-tag-plan.json":true,"status":"success","exists":true,"command_failures":1,"artifact_failures":0,"report_only_command_count":0,"required_command_count":15,"successful_commands":14}"#,
        )
        .expect_err("command failures must fail checklist evidence");

        assert_eq!(
            err,
            ReleaseChecklistError::EvidenceRunHasFailures {
                field: "command_failures",
                observed: 1,
            }
        );
    }

    fn evidence() -> Vec<ReleaseChecklistEvidence<'static>> {
        vec![
            item(
                ReleaseChecklistClass::CargoFull,
                "./cargo_full test -j1 --workspace",
            ),
            item(
                ReleaseChecklistClass::GpuTests,
                "nvidia-smi NVIDIA RTX 5090 CUDA tests green",
            ),
            item(
                ReleaseChecklistClass::FuzzHostile,
                "release/hostile-input.md",
            ),
            item(ReleaseChecklistClass::GapFindings, "release/gaps/vyrec.md"),
            item(
                ReleaseChecklistClass::Benchmarks,
                "release/benchmarks/cuda.md",
            ),
            item(ReleaseChecklistClass::Docs, "release/docs/scope.md"),
            item(
                ReleaseChecklistClass::CrateMetadata,
                "Cargo.toml package metadata",
            ),
            item(
                ReleaseChecklistClass::PublicApiReview,
                "release/public-api.md",
            ),
            item(
                ReleaseChecklistClass::DeepReview,
                "release/reviews/deep-review.md",
            ),
        ]
    }

    fn item(
        class: ReleaseChecklistClass,
        evidence: &'static str,
    ) -> ReleaseChecklistEvidence<'static> {
        ReleaseChecklistEvidence {
            class,
            evidence,
            green: true,
        }
    }
}
