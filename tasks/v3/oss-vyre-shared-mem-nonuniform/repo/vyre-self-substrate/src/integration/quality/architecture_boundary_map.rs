//! Architecture boundary-map validation for Vyre, dataflow consumers, and parser tooling.

use std::collections::BTreeSet;

/// One owned architectural duty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchitectureBoundary<'a> {
    /// Duty name, such as parsing, graph formation, scheduling, or dispatch.
    pub duty: &'a str,
    /// Owning crate or subsystem.
    pub owner: &'a str,
    /// Public module that owns the contract.
    pub module: &'a str,
}

/// Boundary-map validation proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchitectureBoundaryMapProof {
    /// Number of validated boundaries.
    pub boundary_count: usize,
}

/// Committed architecture artifact proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchitectureBoundaryArtifactProof {
    /// Number of parser/distributed-frontend components validated.
    pub parser_component_count: usize,
    /// Number of CUDA release-path markers validated.
    pub cuda_marker_count: usize,
    /// Number of Dataflow analysis rows validated.
    pub analysis_count: usize,
    /// Number of contributor topology rows validated.
    pub modular_directory_count: usize,
}

/// Boundary-map validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchitectureBoundaryMapError {
    /// Boundary list is empty.
    EmptyMap,
    /// Required metadata is empty.
    EmptyMetadata {
        /// Duty name.
        duty: String,
        /// Field.
        field: &'static str,
    },
    /// Required duty is missing.
    MissingDuty {
        /// Missing duty.
        duty: &'static str,
    },
    /// Duty has more than one owner.
    DuplicateDutyOwner {
        /// Duty.
        duty: String,
    },
    /// Committed architecture artifact is missing required evidence.
    ArtifactMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed architecture artifact missed a release threshold.
    ArtifactThresholdMiss {
        /// Field.
        field: &'static str,
        /// Observed value.
        observed: usize,
        /// Required value.
        required: usize,
    },
}

impl std::fmt::Display for ArchitectureBoundaryMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMap => write!(
                f,
                "architecture boundary map is empty. Fix: define owners for parsing, graph formation, lowering, scheduling, dispatch, and validation."
            ),
            Self::EmptyMetadata { duty, field } => write!(
                f,
                "architecture boundary `{duty}` has empty {field}. Fix: every duty needs owner and module."
            ),
            Self::MissingDuty { duty } => write!(
                f,
                "architecture boundary map is missing duty `{duty}`. Fix: make crate ownership explicit for contributors."
            ),
            Self::DuplicateDutyOwner { duty } => write!(
                f,
                "architecture boundary duty `{duty}` has multiple owners. Fix: choose one owner and route other crates through its public contract."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "architecture boundary artifact is missing {evidence}. Fix: publish committed ownership evidence for parser components, Dataflow analysis dataflow, CUDA dispatch, and contributor topology."
            ),
            Self::ArtifactThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "architecture boundary artifact {field}={observed} missed required {required}. Fix: expand committed architecture evidence until every release surface has one clear owner."
            ),
        }
    }
}

impl std::error::Error for ArchitectureBoundaryMapError {}

const REQUIRED_DUTIES: &[&str] = &[
    "parsing",
    "graph-formation",
    "lowering",
    "scheduling",
    "dispatch",
    "validation",
];

/// Validate contributor-facing architecture duty ownership.
pub fn validate_architecture_boundary_map(
    boundaries: &[ArchitectureBoundary<'_>],
) -> Result<ArchitectureBoundaryMapProof, ArchitectureBoundaryMapError> {
    if boundaries.is_empty() {
        return Err(ArchitectureBoundaryMapError::EmptyMap);
    }

    let mut duties = BTreeSet::new();
    for boundary in boundaries {
        for (field, value) in [
            ("duty", boundary.duty),
            ("owner", boundary.owner),
            ("module", boundary.module),
        ] {
            if value.trim().is_empty() {
                return Err(ArchitectureBoundaryMapError::EmptyMetadata {
                    duty: boundary.duty.to_owned(),
                    field,
                });
            }
        }
        if !duties.insert(boundary.duty) {
            return Err(ArchitectureBoundaryMapError::DuplicateDutyOwner {
                duty: boundary.duty.to_owned(),
            });
        }
    }

    for duty in REQUIRED_DUTIES {
        if !duties.contains(duty) {
            return Err(ArchitectureBoundaryMapError::MissingDuty { duty });
        }
    }

    Ok(ArchitectureBoundaryMapProof {
        boundary_count: boundaries.len(),
    })
}

/// Validate committed architecture-boundary evidence across parser, dataflow, CUDA, and tests.
pub fn validate_committed_architecture_boundary_artifacts(
    distributed_parser_map: &str,
    frontend_c_contracts: &str,
    vyrec_cli_contracts: &str,
    contracts: &str,
    consumer_contracts: &str,
    grammar_contracts: &str,
    backend_matrix: &str,
    analysis_matrix: &str,
    modularization_map: &str,
    distributed_parser_doc: &str,
) -> Result<ArchitectureBoundaryArtifactProof, ArchitectureBoundaryMapError> {
    for (artifact, evidence, needle) in [
        (
            distributed_parser_map,
            "distributed parser schema",
            "\"schema_version\": 1",
        ),
        (distributed_parser_map, "zero parser blockers", "\"blockers\": []"),
        (
            distributed_parser_map,
            "vyre-frontend-c parser owner",
            "\"id\": \"vyre-frontend-c\"",
        ),
        (
            distributed_parser_map,
            "vyrec CLI parser owner",
            "\"id\": \"vyrec\"",
        ),
        (
            distributed_parser_map,
            "Dataflow analysis dataflow parser owner",
            "\"id\": \"dataflow-consumer\"",
        ),
        (
            distributed_parser_map,
            "Downstream analyzer consumer owner",
            "\"role\": \"Security compiler consumer integration surface\"",
        ),
        (
            distributed_parser_map,
            "grammar generator owner",
            "\"role\": \"Shared grammar generation substrate\"",
        ),
        (
            distributed_parser_map,
            "empty ownership marker lists",
            "\"unresolved_ownership_markers\": []",
        ),
        (
            distributed_parser_map,
            "empty missing contract topics",
            "\"missing_contract_topics\": []",
        ),
        (
            distributed_parser_map,
            "empty missing test categories",
            "\"missing_test_categories\": []",
        ),
        (
            distributed_parser_map,
            "test evidence tree",
            "\"tree\": \"tests\"",
        ),
        (
            distributed_parser_map,
            "benchmark evidence tree",
            "\"tree\": \"benches\"",
        ),
        (distributed_parser_map, "fuzz evidence tree", "\"tree\": \"fuzz\""),
        (
            frontend_c_contracts,
            "frontend C contract owner",
            "\"component_id\": \"vyre-frontend-c\"",
        ),
        (
            frontend_c_contracts,
            "frontend preprocessor contract",
            "\"preprocessor\"",
        ),
        (
            frontend_c_contracts,
            "frontend GNU contract",
            "\"gnu\"",
        ),
        (
            frontend_c_contracts,
            "frontend unsupported-feature contract",
            "\"unsupported\"",
        ),
        (
            vyrec_cli_contracts,
            "vyrec CUDA CLI contract",
            "\"cuda\"",
        ),
        (
            vyrec_cli_contracts,
            "vyrec actionable diagnostics contract",
            "\"fix:\"",
        ),
        (
            contracts,
            "Dataflow analysis alias/reaching/callgraph contract",
            "\"alias\"",
        ),
        (
            contracts,
            "Dataflow reaching contract",
            "\"reaching\"",
        ),
        (
            contracts,
            "Dataflow analysis callgraph contract",
            "\"callgraph\"",
        ),
        (
            consumer_contracts,
            "Downstream analyzer consumer contract",
            "\"role\": \"Security compiler consumer integration surface\"",
        ),
        (
            grammar_contracts,
            "grammar generator contract",
            "\"generate\"",
        ),
        (
            backend_matrix,
            "CUDA-first backend matrix",
            "\"cuda_first\": true",
        ),
        (
            backend_matrix,
            "CUDA preferred backend",
            "\"preferred_backend_id\": \"cuda\"",
        ),
        (
            backend_matrix,
            "GPU-only preferred backend",
            "\"preferred_backend_gpu_only\": true",
        ),
        (
            backend_matrix,
            "RTX 5090 release probe",
            "NVIDIA GeForce RTX 5090",
        ),
        (
            backend_matrix,
            "CUDA resident dispatch",
            "\"id\": \"cuda-resident-dispatch\"",
        ),
        (
            backend_matrix,
            "CUDA graph launch",
            "\"id\": \"cuda-graph-launch\"",
        ),
        (
            backend_matrix,
            "CUDA module cache",
            "\"id\": \"cuda-module-cache\"",
        ),
        (
            backend_matrix,
            "CUDA PTX source cache",
            "\"id\": \"cuda-ptx-source-cache\"",
        ),
        (
            backend_matrix,
            "WGPU fallback owner",
            "\"wgpu_fallback_present\": true",
        ),
        (
            backend_matrix,
            "no hidden fallback findings",
            "\"hidden_fallback_findings\": []",
        ),
        (
            analysis_matrix,
            "Dataflow analysis SSA analysis owner",
            "\"id\": \"ssa\"",
        ),
        (
            analysis_matrix,
            "Dataflow analysis points-to analysis owner",
            "\"id\": \"points_to\"",
        ),
        (
            analysis_matrix,
            "Dataflow analysis IFDS analysis owner",
            "\"id\": \"ifds\"",
        ),
        (
            analysis_matrix,
            "Dataflow analysis callgraph analysis owner",
            "\"id\": \"callgraph\"",
        ),
        (
            analysis_matrix,
            "Dataflow analysis liveness analysis owner",
            "\"id\": \"live\"",
        ),
        (
            analysis_matrix,
            "Dataflow analysis slice analysis owner",
            "\"id\": \"slice\"",
        ),
        (
            analysis_matrix,
            "no missing Dataflow analysis APIs",
            "\"missing_api_items\": []",
        ),
        (
            analysis_matrix,
            "no unresolved Dataflow analysis markers",
            "\"unresolved_markers\": []",
        ),
        (
            modularization_map,
            "Vyre contributor topology",
            "\"surface\": \"vyre\"",
        ),
        (
            modularization_map,
            "dataflow contributor topology",
            concat!("\"surface\": \"", "we", "ir", "\""),
        ),
        (
            modularization_map,
            "Vyrec parser CLI contributor topology",
            "\"surface\": \"vyrec\"",
        ),
        (
            modularization_map,
            "backend test topology",
            "\"layer\": \"backends\"",
        ),
        (
            modularization_map,
            "zero topology blockers",
            "\"blockers\": []",
        ),
        (
            distributed_parser_doc,
            "distributed parser coherence title",
            "# Distributed parser coherence proof",
        ),
        (
            distributed_parser_doc,
            "explicit distributed parser ownership contract",
            "Parser boundaries must be coherent even though the parser implementation is distributed.",
        ),
        (
            distributed_parser_doc,
            "contract artifact list",
            "Required generated evidence:",
        ),
    ] {
        artifact_contains(artifact, evidence, needle)?;
    }

    for contract in [
        frontend_c_contracts,
        vyrec_cli_contracts,
        contracts,
        consumer_contracts,
        grammar_contracts,
    ] {
        for (evidence, needle) in [
            ("contract schema", "\"schema_version\": 1"),
            ("contract blockers inventory", "\"blockers\""),
            (
                "contract ownership markers",
                "\"unresolved_ownership_markers\": []",
            ),
            ("contract tests tree", "\"tree\": \"tests\""),
            ("contract benches tree", "\"tree\": \"benches\""),
            ("contract fuzz tree", "\"tree\": \"fuzz\""),
        ] {
            artifact_contains(contract, evidence, needle)?;
        }
    }

    let parser_component_count = distributed_parser_map.matches("\"id\": ").count();
    let cuda_marker_count = backend_matrix.matches("\"id\": \"cuda").count();
    let analysis_count = analysis_matrix.matches("\"id\": ").count();
    let modular_directory_count = modularization_map.matches("\"surface\": ").count();

    artifact_at_least("parser components", parser_component_count, 5)?;
    artifact_at_least("CUDA backend markers", cuda_marker_count, 7)?;
    artifact_at_least("Dataflow analysis rows", analysis_count, 20)?;
    artifact_at_least("modular directory rows", modular_directory_count, 21)?;

    Ok(ArchitectureBoundaryArtifactProof {
        parser_component_count,
        cuda_marker_count,
        analysis_count,
        modular_directory_count,
    })
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), ArchitectureBoundaryMapError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(ArchitectureBoundaryMapError::ArtifactMissingEvidence { evidence })
    }
}

fn artifact_at_least(
    field: &'static str,
    observed: usize,
    required: usize,
) -> Result<(), ArchitectureBoundaryMapError> {
    if observed >= required {
        Ok(())
    } else {
        Err(ArchitectureBoundaryMapError::ArtifactThresholdMiss {
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
    fn boundary_map_accepts_single_owner_per_required_duty() {
        let proof = validate_architecture_boundary_map(&boundaries())
            .expect("Fix: complete boundary map should pass");

        assert_eq!(proof.boundary_count, 6);
    }

    #[test]
    fn boundary_map_rejects_missing_required_duties() {
        let mut boundaries = boundaries();
        boundaries.pop();

        assert_eq!(
            validate_architecture_boundary_map(&boundaries)
                .expect_err("missing validation duty should fail"),
            ArchitectureBoundaryMapError::MissingDuty { duty: "validation" }
        );
    }

    #[test]
    fn boundary_map_rejects_duplicate_duty_ownership() {
        let mut boundaries = boundaries();
        boundaries.push(ArchitectureBoundary {
            duty: "dispatch",
            owner: "vyrec",
            module: "vyrec::dispatch",
        });

        assert_eq!(
            validate_architecture_boundary_map(&boundaries)
                .expect_err("duplicate duty should fail"),
            ArchitectureBoundaryMapError::DuplicateDutyOwner {
                duty: "dispatch".to_owned(),
            }
        );
    }

    #[test]
    fn boundary_artifacts_accept_committed_architecture_evidence() {
        let proof = committed_architecture_artifact_proof()
            .expect("Fix: committed architecture evidence should prove release boundaries");

        assert!(proof.parser_component_count >= 5);
        assert!(proof.cuda_marker_count >= 7);
        assert!(proof.analysis_count >= 20);
        assert!(proof.modular_directory_count >= 21);
    }

    #[test]
    fn boundary_artifacts_reject_missing_cuda_release_path() {
        let backend_matrix =
            include_str!("../../../../release/evidence/backends/backend-matrix.json").replace(
                "\"preferred_backend_id\": \"cuda\"",
                "\"preferred_backend_id\": \"wgpu\"",
            );

        assert_eq!(
            validate_committed_architecture_boundary_artifacts(
                include_str!(
                    "../../../../release/evidence/parser/distributed-parser-boundary-map.json"
                ),
                include_str!("../../../../release/evidence/parser/vyre-frontend-c-contracts.json"),
                include_str!("../../../../release/evidence/parser/vyrec-cli-contracts.json"),
                include_str!(
                    "../../../../release/evidence/parser/dataflow-consumer-contracts.json"
                ),
                include_str!("../../../../release/evidence/parser/surgec-contracts.json"),
                include_str!(
                    "../../../../release/evidence/parser/surgec-grammar-gen-contracts.json"
                ),
                &backend_matrix,
                include_str!("../../../../release/evidence/dataflow/analysis-api-matrix.json"),
                include_str!("../../../../release/evidence/tests/modularization-map.json"),
                include_str!("../../../../release/evidence/docs/distributed-parser-coherence.md"),
            )
            .expect_err("release boundary proof must not accept WGPU as preferred path"),
            ArchitectureBoundaryMapError::ArtifactMissingEvidence {
                evidence: "CUDA preferred backend",
            }
        );
    }

    #[test]
    fn boundary_artifacts_reject_missing_dataflow_api_ownership() {
        let analysis =
            include_str!("../../../../release/evidence/dataflow/analysis-api-matrix.json")
                .replace("\"id\": \"points_to\"", "\"id\": \"points_to_removed\"");

        assert_eq!(
            validate_committed_architecture_boundary_artifacts(
                include_str!(
                    "../../../../release/evidence/parser/distributed-parser-boundary-map.json"
                ),
                include_str!("../../../../release/evidence/parser/vyre-frontend-c-contracts.json"),
                include_str!("../../../../release/evidence/parser/vyrec-cli-contracts.json"),
                include_str!(
                    "../../../../release/evidence/parser/dataflow-consumer-contracts.json"
                ),
                include_str!("../../../../release/evidence/parser/surgec-contracts.json"),
                include_str!(
                    "../../../../release/evidence/parser/surgec-grammar-gen-contracts.json"
                ),
                include_str!("../../../../release/evidence/backends/backend-matrix.json"),
                &analysis,
                include_str!("../../../../release/evidence/tests/modularization-map.json"),
                include_str!("../../../../release/evidence/docs/distributed-parser-coherence.md"),
            )
            .expect_err("release boundary proof must not accept missing Dataflow analysis owner"),
            ArchitectureBoundaryMapError::ArtifactMissingEvidence {
                evidence: "Dataflow analysis points-to analysis owner",
            }
        );
    }

    fn boundaries() -> Vec<ArchitectureBoundary<'static>> {
        vec![
            boundary("parsing", "vyrec", "vyrec::parser"),
            boundary("graph-formation", "dataflow", "dataflow::graph_layout"),
            boundary("lowering", "vyre", "vyre_driver::lowering"),
            boundary(
                "scheduling",
                "vyre-cuda",
                "vyre_driver_cuda::megakernel_scheduler",
            ),
            boundary("dispatch", "vyre-cuda", "vyre_driver_cuda::backend"),
            boundary(
                "validation",
                "vyre-self",
                "vyre_self_substrate::release_validation_matrix",
            ),
        ]
    }

    fn boundary(
        duty: &'static str,
        owner: &'static str,
        module: &'static str,
    ) -> ArchitectureBoundary<'static> {
        ArchitectureBoundary {
            duty,
            owner,
            module,
        }
    }

    fn committed_architecture_artifact_proof(
    ) -> Result<ArchitectureBoundaryArtifactProof, ArchitectureBoundaryMapError> {
        validate_committed_architecture_boundary_artifacts(
            include_str!(
                "../../../../release/evidence/parser/distributed-parser-boundary-map.json"
            ),
            include_str!("../../../../release/evidence/parser/vyre-frontend-c-contracts.json"),
            include_str!("../../../../release/evidence/parser/vyrec-cli-contracts.json"),
            include_str!("../../../../release/evidence/parser/dataflow-consumer-contracts.json"),
            include_str!("../../../../release/evidence/parser/surgec-contracts.json"),
            include_str!("../../../../release/evidence/parser/surgec-grammar-gen-contracts.json"),
            include_str!("../../../../release/evidence/backends/backend-matrix.json"),
            include_str!("../../../../release/evidence/dataflow/analysis-api-matrix.json"),
            include_str!("../../../../release/evidence/tests/modularization-map.json"),
            include_str!("../../../../release/evidence/docs/distributed-parser-coherence.md"),
        )
    }
}
