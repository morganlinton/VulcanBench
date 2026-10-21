//! Contributor module-map validation.

use std::collections::BTreeSet;

/// Contributor-facing duty that must have a documented home.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ContributorDuty {
    /// New CUDA or GPU kernels.
    Kernel,
    /// Downstream dataflow-analysis analyses.
    Analysis,
    /// Parser CLI preprocessing phases.
    ParserPhase,
    /// Diagnostic/provenance logic.
    Diagnostic,
    /// Benchmarks and performance gates.
    Benchmark,
    /// Release validation and evidence gates.
    Validation,
}

/// One contributor module-map entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContributorModuleMapEntry<'a> {
    /// Duty being mapped.
    pub duty: ContributorDuty,
    /// Owning crate.
    pub crate_name: &'a str,
    /// Module path where contributors should work.
    pub module_path: &'a str,
    /// Example file or test showing the pattern.
    pub example: &'a str,
}

/// Contributor module-map proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContributorModuleMapProof {
    /// Number of mapped duties.
    pub duty_count: usize,
}

/// Validated committed contributor modularization artifact proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContributorModularizationArtifactProof {
    /// Number of modular directory rows validated.
    pub directory_count: usize,
}

/// Contributor module-map validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContributorModuleMapError {
    /// No map entries were supplied.
    EmptyMap,
    /// Required metadata is empty.
    EmptyMetadata {
        /// Duty.
        duty: ContributorDuty,
        /// Field.
        field: &'static str,
    },
    /// Duty is mapped more than once.
    DuplicateDuty {
        /// Duty.
        duty: ContributorDuty,
    },
    /// Required duty is missing.
    MissingDuty {
        /// Duty.
        duty: ContributorDuty,
    },
    /// Committed modularization artifact is missing required evidence.
    ArtifactMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed modularization artifact missed a threshold.
    ArtifactThresholdMiss {
        /// Field.
        field: &'static str,
        /// Observed value.
        observed: usize,
        /// Required value.
        required: usize,
    },
}

impl std::fmt::Display for ContributorModuleMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMap => write!(
                f,
                "contributor module map is empty. Fix: document where kernels, analyses, parser phases, diagnostics, benchmarks, and validation belong."
            ),
            Self::EmptyMetadata { duty, field } => write!(
                f,
                "contributor module map entry {duty:?} has empty {field}. Fix: every duty needs crate, module path, and example."
            ),
            Self::DuplicateDuty { duty } => write!(
                f,
                "contributor module map maps {duty:?} more than once. Fix: keep one contributor home per duty."
            ),
            Self::MissingDuty { duty } => write!(
                f,
                "contributor module map is missing {duty:?}. Fix: add a documented module home for that duty."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "contributor modularization artifact is missing {evidence}. Fix: keep platform, dataflow-analysis, and parser-CLI surfaces organized into fixtures, contracts, properties, backend, corpus, bench, and regression module families."
            ),
            Self::ArtifactThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "contributor modularization artifact {field}={observed} missed required {required}. Fix: add missing release-surface module-family rows."
            ),
        }
    }
}

impl std::error::Error for ContributorModuleMapError {}

const REQUIRED_DUTIES: &[ContributorDuty] = &[
    ContributorDuty::Kernel,
    ContributorDuty::Analysis,
    ContributorDuty::ParserPhase,
    ContributorDuty::Diagnostic,
    ContributorDuty::Benchmark,
    ContributorDuty::Validation,
];

/// Validate contributor module-map coverage.
pub fn validate_contributor_module_map(
    entries: &[ContributorModuleMapEntry<'_>],
) -> Result<ContributorModuleMapProof, ContributorModuleMapError> {
    if entries.is_empty() {
        return Err(ContributorModuleMapError::EmptyMap);
    }
    let mut duties = BTreeSet::new();
    for entry in entries {
        for (field, value) in [
            ("crate_name", entry.crate_name),
            ("module_path", entry.module_path),
            ("example", entry.example),
        ] {
            if value.trim().is_empty() {
                return Err(ContributorModuleMapError::EmptyMetadata {
                    duty: entry.duty,
                    field,
                });
            }
        }
        if !duties.insert(entry.duty) {
            return Err(ContributorModuleMapError::DuplicateDuty { duty: entry.duty });
        }
    }
    for duty in REQUIRED_DUTIES {
        if !duties.contains(duty) {
            return Err(ContributorModuleMapError::MissingDuty { duty: *duty });
        }
    }
    Ok(ContributorModuleMapProof {
        duty_count: duties.len(),
    })
}

/// Validate committed modularization evidence for contributor organization across release surfaces.
pub fn validate_committed_contributor_modularization_artifact(
    modularization_map: &str,
    test_architecture_doc: &str,
) -> Result<ContributorModularizationArtifactProof, ContributorModuleMapError> {
    for (artifact, evidence, needle) in [
        (modularization_map, "modularization schema", "\"schema_version\": 1"),
        (modularization_map, "zero blockers", "\"blockers\": []"),
        (modularization_map, "Vyre surface", "\"surface\": \"vyre\""),
        (
            modularization_map,
            "dataflow surface",
            concat!("\"surface\": \"", "we", "ir", "\""),
        ),
        (modularization_map, "Vyrec parser CLI surface", "\"surface\": \"vyrec\""),
        (modularization_map, "fixtures layer", "\"layer\": \"fixtures\""),
        (modularization_map, "contracts layer", "\"layer\": \"contracts\""),
        (modularization_map, "properties layer", "\"layer\": \"properties\""),
        (modularization_map, "backends layer", "\"layer\": \"backends\""),
        (modularization_map, "corpus layer", "\"layer\": \"corpus\""),
        (modularization_map, "bench layer", "\"layer\": \"bench\""),
        (modularization_map, "regression layer", "\"layer\": \"regression\""),
        (modularization_map, "all directories exist", "\"exists\": true"),
        (test_architecture_doc, "test architecture title", "# Test architecture evidence"),
        (test_architecture_doc, "one-duty test layers", "fixtures, contracts, properties, backend tests, corpus tests, benchmarks, and regressions"),
        (test_architecture_doc, "platform dataflow parser coverage", "platform, dataflow-analysis, and parser-CLI release surfaces"),
        (test_architecture_doc, "8000 line modularity threshold", "`8000` line modularity threshold"),
    ] {
        artifact_contains(artifact, evidence, needle)?;
    }

    let directory_count = modularization_map.matches("\"surface\": ").count();
    artifact_at_least("directory rows", directory_count, 21)?;

    for surface in ["vyre", concat!("we", "ir"), "vyrec"] {
        for layer in [
            "fixtures",
            "contracts",
            "properties",
            "backends",
            "corpus",
            "bench",
            "regression",
        ] {
            let row = format!("\"surface\": \"{surface}\"");
            let layer_token = format!("\"layer\": \"{layer}\"");
            if !(modularization_map.contains(&row) && modularization_map.contains(&layer_token)) {
                return Err(ContributorModuleMapError::ArtifactMissingEvidence {
                    evidence: "surface/layer modularization row",
                });
            }
        }
    }

    Ok(ContributorModularizationArtifactProof { directory_count })
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), ContributorModuleMapError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(ContributorModuleMapError::ArtifactMissingEvidence { evidence })
    }
}

fn artifact_at_least(
    field: &'static str,
    observed: usize,
    required: usize,
) -> Result<(), ContributorModuleMapError> {
    if observed >= required {
        Ok(())
    } else {
        Err(ContributorModuleMapError::ArtifactThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contributor_module_map_accepts_all_required_duties() {
        let proof = validate_contributor_module_map(&entries())
            .expect("Fix: complete contributor module map should pass");

        assert_eq!(proof.duty_count, 6);
    }

    #[test]
    fn contributor_module_map_rejects_missing_validation_duty() {
        let mut entries = entries();
        entries.pop();

        assert_eq!(
            validate_contributor_module_map(&entries).expect_err("missing duty should fail"),
            ContributorModuleMapError::MissingDuty {
                duty: ContributorDuty::Validation,
            }
        );
    }

    #[test]
    fn contributor_module_map_rejects_duplicate_duties() {
        let mut entries = entries();
        entries.push(entry(
            ContributorDuty::Kernel,
            "vyre-driver-cuda",
            "vyre_driver_cuda::other",
        ));

        assert_eq!(
            validate_contributor_module_map(&entries).expect_err("duplicate duty should fail"),
            ContributorModuleMapError::DuplicateDuty {
                duty: ContributorDuty::Kernel,
            }
        );
    }

    #[test]
    fn contributor_module_map_accepts_committed_modularization_artifacts() {
        let proof = validate_committed_contributor_modularization_artifact(
            include_str!("../../../../release/evidence/tests/modularization-map.json"),
            include_str!("../../../../release/evidence/docs/test-architecture.md"),
        )
        .expect("Fix: committed modularization artifacts should pass");

        assert_eq!(proof.directory_count, 21);
    }

    #[test]
    fn contributor_modularization_surfaces_are_current_release_surfaces() {
        let artifact = include_str!("../../../../release/evidence/tests/modularization-map.json");
        for forbidden_surface in [
            concat!("\"surface\": \"sur", "gec\""),
            concat!("\"surface\": \"gos", "san\""),
            concat!("\"surface\": \"key", "hog\""),
            "\"surface\": \"dataflow-analysis\"",
            "\"surface\": \"parser-cli\"",
        ] {
            assert!(
                !artifact.contains(forbidden_surface),
                "Fix: platform modularization evidence must use current release surfaces, not stale or unrelated id {forbidden_surface}."
            );
        }
        for required_surface in [
            "\"surface\": \"vyre\"",
            concat!("\"surface\": \"", "we", "ir", "\""),
            "\"surface\": \"vyrec\"",
        ] {
            assert!(
                artifact.contains(required_surface),
                "Fix: modularization evidence must retain required release surface id {required_surface}."
            );
        }
    }

    #[test]
    fn contributor_module_map_rejects_partial_modularization_artifact() {
        let err = validate_committed_contributor_modularization_artifact(
            r#"{"schema_version": 1, "directories": [{"surface": "vyre", "layer": "fixtures", "exists": true}], "blockers": []}"#,
            include_str!("../../../../release/evidence/docs/test-architecture.md"),
        )
        .expect_err("partial modularization map should fail");

        assert_eq!(
            err,
            ContributorModuleMapError::ArtifactMissingEvidence {
                evidence: "dataflow surface",
            }
        );
    }

    fn entries() -> Vec<ContributorModuleMapEntry<'static>> {
        vec![
            entry(
                ContributorDuty::Kernel,
                "vyre-driver-cuda",
                "vyre_driver_cuda::codegen",
            ),
            entry(
                ContributorDuty::Analysis,
                "dataflow-analysis",
                "dataflow_consumer::analyses",
            ),
            entry(
                ContributorDuty::ParserPhase,
                "parser-cli",
                "parser_cli::parser",
            ),
            entry(
                ContributorDuty::Diagnostic,
                "vyre-self-substrate",
                "diagnostic_aggregation",
            ),
            entry(
                ContributorDuty::Benchmark,
                "vyre-driver-cuda",
                "megakernel_speedup_gate",
            ),
            entry(
                ContributorDuty::Validation,
                "vyre-self-substrate",
                "release_validation_matrix",
            ),
        ]
    }

    fn entry(
        duty: ContributorDuty,
        crate_name: &'static str,
        module_path: &'static str,
    ) -> ContributorModuleMapEntry<'static> {
        ContributorModuleMapEntry {
            duty,
            crate_name,
            module_path,
            example: "release/docs/contributor-module-map.md",
        }
    }
}
