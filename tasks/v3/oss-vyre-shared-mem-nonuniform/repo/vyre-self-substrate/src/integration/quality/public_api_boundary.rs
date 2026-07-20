//! Public API boundary validation.

/// One public export observed from API surface analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicApiExport<'a> {
    /// Crate exposing the symbol.
    pub crate_name: &'a str,
    /// Public symbol path.
    pub symbol: &'a str,
}

/// Public API boundary proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicApiBoundaryProof {
    /// Number of exports scanned.
    pub export_count: usize,
}

/// One source file that owns a public API boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicApiSourceSurface<'a> {
    /// Crate exposing the public surface.
    pub crate_name: &'a str,
    /// Source file contents for the public root.
    pub source: &'a str,
    /// Whether `pub mod pipeline` is an intentional stable contract in this crate.
    pub allow_public_pipeline_module: bool,
}

/// Public API source-boundary proof over committed sources and docs artifacts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicApiSourceBoundaryProof {
    /// Number of source surfaces scanned.
    pub surface_count: usize,
    /// Number of public source lines scanned.
    pub public_line_count: usize,
}

/// Public API boundary validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicApiBoundaryError {
    /// No exports were supplied.
    EmptyExports,
    /// Required metadata is empty.
    EmptyMetadata {
        /// Field name.
        field: &'static str,
    },
    /// Internal implementation detail is public.
    InternalDetailExported {
        /// Crate exposing the symbol.
        crate_name: String,
        /// Public symbol path.
        symbol: String,
        /// Forbidden fragment.
        forbidden_fragment: &'static str,
    },
    /// Committed public API source artifact is missing required evidence.
    ArtifactMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Public API source scan found too little evidence.
    ArtifactThresholdMiss {
        /// Field.
        field: &'static str,
        /// Observed value.
        observed: usize,
        /// Required value.
        required: usize,
    },
}

impl std::fmt::Display for PublicApiBoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyExports => write!(
                f,
                "public API boundary export scan is empty. Fix: inspect public exports before release."
            ),
            Self::EmptyMetadata { field } => write!(
                f,
                "public API boundary evidence has empty {field}. Fix: record crate and public symbol path."
            ),
            Self::InternalDetailExported {
                crate_name,
                symbol,
                forbidden_fragment,
            } => write!(
                f,
                "crate `{crate_name}` exports internal detail `{symbol}` containing `{forbidden_fragment}`. Fix: keep staging buffers, temp graph encodings, and pipeline internals private."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "public API boundary artifact is missing {evidence}. Fix: connect real public source roots and docs artifacts to the boundary gate."
            ),
            Self::ArtifactThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "public API boundary artifact {field}={observed} missed required {required}. Fix: scan every release-facing source root before launch."
            ),
        }
    }
}

impl std::error::Error for PublicApiBoundaryError {}

const FORBIDDEN_PUBLIC_FRAGMENTS: &[&str] = &[
    "staging",
    "temporary",
    "temp_graph",
    "pipeline::",
    "internal",
    "private",
];

/// Validate that public APIs do not expose internal staging or pipeline details.
pub fn validate_public_api_boundary(
    exports: &[PublicApiExport<'_>],
) -> Result<PublicApiBoundaryProof, PublicApiBoundaryError> {
    if exports.is_empty() {
        return Err(PublicApiBoundaryError::EmptyExports);
    }
    for export in exports {
        if export.crate_name.trim().is_empty() {
            return Err(PublicApiBoundaryError::EmptyMetadata {
                field: "crate_name",
            });
        }
        if export.symbol.trim().is_empty() {
            return Err(PublicApiBoundaryError::EmptyMetadata { field: "symbol" });
        }
        let lower = export.symbol.to_ascii_lowercase();
        for forbidden_fragment in FORBIDDEN_PUBLIC_FRAGMENTS {
            if lower.contains(forbidden_fragment) {
                return Err(PublicApiBoundaryError::InternalDetailExported {
                    crate_name: export.crate_name.to_owned(),
                    symbol: export.symbol.to_owned(),
                    forbidden_fragment,
                });
            }
        }
    }

    Ok(PublicApiBoundaryProof {
        export_count: exports.len(),
    })
}

/// Validate real public source roots and docs artifacts for boundary leaks.
pub fn validate_public_api_source_boundaries(
    surfaces: &[PublicApiSourceSurface<'_>],
    vyre_readme_contracts: &str,
    readme_contracts: &str,
    docs_matrix: &str,
) -> Result<PublicApiSourceBoundaryProof, PublicApiBoundaryError> {
    if surfaces.is_empty() {
        return Err(PublicApiBoundaryError::EmptyExports);
    }
    for (artifact, evidence, needle) in [
        (
            vyre_readme_contracts,
            "Vyre README contract schema",
            "\"schema_version\": 2",
        ),
        (
            vyre_readme_contracts,
            "Vyre README zero blockers",
            "\"blockers\": []",
        ),
        (
            vyre_readme_contracts,
            "Vyre README complete tokens",
            "\"missing_tokens\": []",
        ),
        (vyre_readme_contracts, "CUDA public docs token", "\"cuda\""),
        (
            vyre_readme_contracts,
            "Vyre public program token",
            "\"vyre::program\"",
        ),
        (
            readme_contracts,
            "Dataflow consumer README contract schema",
            "\"schema_version\": 2",
        ),
        (
            readme_contracts,
            "Dataflow consumer README zero blockers",
            "\"blockers\": []",
        ),
        (
            readme_contracts,
            "Dataflow consumer README complete tokens",
            "\"missing_tokens\": []",
        ),
        (
            readme_contracts,
            "Dataflow consumer standalone API token",
            "\"dataflow\"",
        ),
        (
            readme_contracts,
            "Dataflow consumer serde feature token",
            "\"serde_evidence\"",
        ),
        (docs_matrix, "docs matrix schema", "\"schema_version\": 5"),
        (
            docs_matrix,
            "docs matrix blockers inventory",
            "\"blockers\"",
        ),
        (
            docs_matrix,
            "docs evidence reference inventory",
            "\"missing_evidence_artifact_refs\"",
        ),
    ] {
        artifact_contains(artifact, evidence, needle)?;
    }

    let mut public_line_count = 0;
    for surface in surfaces {
        if surface.crate_name.trim().is_empty() {
            return Err(PublicApiBoundaryError::EmptyMetadata {
                field: "crate_name",
            });
        }
        if surface.source.trim().is_empty() {
            return Err(PublicApiBoundaryError::EmptyMetadata { field: "source" });
        }
        if surface.crate_name == "vyre-driver-cuda" {
            require_source_contains(surface, "private CUDA pipeline module", "mod pipeline;")?;
            require_source_contains(surface, "private CUDA stream module", "mod stream;")?;
            require_source_contains(surface, "CUDA backend id contract", "CUDA_BACKEND_ID")?;
            require_source_contains(surface, "CUDA backend public contract", "CudaBackend")?;
        }
        if surface.crate_name == "dataflow" {
            require_source_contains(
                surface,
                "CPU parity oracle feature gate",
                "#[cfg(any(test, feature = \"cpu-parity\"))]",
            )?;
            require_source_contains(
                surface,
                "Dataflow consumer dataflow owner module",
                "pub mod graph_layout;",
            )?;
            require_source_contains(
                surface,
                "Dataflow consumer private output scratch contract",
                "mod output_scratch;",
            )?;
        }

        for line in surface.source.lines() {
            let trimmed = line.trim_start();
            let is_public_line = trimmed.starts_with("pub mod ")
                || trimmed.starts_with("pub use ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("pub enum ")
                || trimmed.starts_with("pub trait ")
                || trimmed.starts_with("pub const ")
                || trimmed.starts_with("pub fn ");
            if !is_public_line {
                continue;
            }
            public_line_count += 1;
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("pub mod pipeline") && !surface.allow_public_pipeline_module {
                return Err(PublicApiBoundaryError::InternalDetailExported {
                    crate_name: surface.crate_name.to_owned(),
                    symbol: trimmed.to_owned(),
                    forbidden_fragment: "pipeline",
                });
            }
            if lower.starts_with("pub mod stream") {
                return Err(PublicApiBoundaryError::InternalDetailExported {
                    crate_name: surface.crate_name.to_owned(),
                    symbol: trimmed.to_owned(),
                    forbidden_fragment: "stream",
                });
            }
            for forbidden_fragment in FORBIDDEN_PUBLIC_FRAGMENTS {
                if *forbidden_fragment == "pipeline::" && surface.allow_public_pipeline_module {
                    continue;
                }
                if lower.contains(forbidden_fragment) {
                    return Err(PublicApiBoundaryError::InternalDetailExported {
                        crate_name: surface.crate_name.to_owned(),
                        symbol: trimmed.to_owned(),
                        forbidden_fragment,
                    });
                }
            }
        }
    }

    artifact_at_least("source surfaces", surfaces.len(), 4)?;
    artifact_at_least("public source lines", public_line_count, 80)?;

    Ok(PublicApiSourceBoundaryProof {
        surface_count: surfaces.len(),
        public_line_count,
    })
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), PublicApiBoundaryError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(PublicApiBoundaryError::ArtifactMissingEvidence { evidence })
    }
}

fn artifact_at_least(
    field: &'static str,
    observed: usize,
    required: usize,
) -> Result<(), PublicApiBoundaryError> {
    if observed >= required {
        Ok(())
    } else {
        Err(PublicApiBoundaryError::ArtifactThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn require_source_contains(
    surface: &PublicApiSourceSurface<'_>,
    evidence: &'static str,
    needle: &str,
) -> Result<(), PublicApiBoundaryError> {
    if surface.source.contains(needle) {
        Ok(())
    } else {
        Err(PublicApiBoundaryError::ArtifactMissingEvidence { evidence })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn public_api_boundary_accepts_contract_level_exports() {
        let proof = validate_public_api_boundary(&[
            export("vyre-driver-cuda", "CudaBackend"),
            export("vyre-driver-cuda", "CudaMegakernelExecutionPlan"),
            export("vyre-self-substrate", "OptimizationRegistry"),
        ])
        .expect("Fix: contract-level exports should pass");

        assert_eq!(proof.export_count, 3);
    }

    #[test]
    fn public_api_boundary_rejects_staging_and_pipeline_exports() {
        assert_eq!(
            validate_public_api_boundary(&[export(
                "vyre-driver-cuda",
                "pipeline::CompiledDispatch"
            )])
            .expect_err("pipeline internals should fail"),
            PublicApiBoundaryError::InternalDetailExported {
                crate_name: "vyre-driver-cuda".to_owned(),
                symbol: "pipeline::CompiledDispatch".to_owned(),
                forbidden_fragment: "pipeline::",
            }
        );
        assert_eq!(
            validate_public_api_boundary(&[export("vyre", "TemporaryGraphEncoding")])
                .expect_err("temporary graph encodings should fail"),
            PublicApiBoundaryError::InternalDetailExported {
                crate_name: "vyre".to_owned(),
                symbol: "TemporaryGraphEncoding".to_owned(),
                forbidden_fragment: "temporary",
            }
        );
    }

    #[test]
    fn public_api_boundary_accepts_committed_source_surfaces() {
        let dataflow_source = dataflow_consumer_source();
        let surfaces = source_surfaces(&dataflow_source);
        let proof = validate_public_api_source_boundaries(
            &surfaces,
            include_str!("../../../../release/evidence/docs/vyre-readme-contracts.json"),
            include_str!("../../../../release/evidence/dataflow/readme-contracts.json"),
            include_str!("../../../../release/evidence/docs/docs-matrix.json"),
        )
        .expect("Fix: committed public source surfaces should keep internals behind boundaries");

        assert_eq!(proof.surface_count, 4);
        assert!(proof.public_line_count >= 80);
    }

    #[test]
    fn public_api_boundary_rejects_cuda_pipeline_module_export() {
        let cuda_source = include_str!("../../../../vyre-driver-cuda/src/lib.rs")
            .replace("mod pipeline;", "pub mod pipeline;");
        let dataflow_source = dataflow_consumer_source();
        let surfaces = [
            source_surface("vyre-driver-cuda", &cuda_source, false),
            source_surface(
                "vyre-driver",
                include_str!("../../../../vyre-driver/src/lib.rs"),
                true,
            ),
            source_surface(
                "vyre-libs",
                include_str!("../../../../vyre-libs/src/lib.rs"),
                false,
            ),
            source_surface("dataflow", &dataflow_source, false),
        ];

        assert_eq!(
            validate_public_api_source_boundaries(
                &surfaces,
                include_str!("../../../../release/evidence/docs/vyre-readme-contracts.json"),
                include_str!("../../../../release/evidence/dataflow/readme-contracts.json"),
                include_str!("../../../../release/evidence/docs/docs-matrix.json"),
            )
            .expect_err("CUDA pipeline internals must stay private"),
            PublicApiBoundaryError::InternalDetailExported {
                crate_name: "vyre-driver-cuda".to_owned(),
                symbol: "pub mod pipeline;".to_owned(),
                forbidden_fragment: "pipeline",
            }
        );
    }

    #[test]
    fn public_api_boundary_rejects_incomplete_docs_evidence() {
        let dataflow_source = dataflow_consumer_source();
        let surfaces = source_surfaces(&dataflow_source);
        let docs_matrix = include_str!("../../../../release/evidence/docs/docs-matrix.json")
            .replace("\"schema_version\": 5", "\"schema_version\": 1");

        assert_eq!(
            validate_public_api_source_boundaries(
                &surfaces,
                include_str!("../../../../release/evidence/docs/vyre-readme-contracts.json"),
                include_str!("../../../../release/evidence/dataflow/readme-contracts.json"),
                &docs_matrix,
            )
            .expect_err("boundary proof must not ignore stale docs matrix schema"),
            PublicApiBoundaryError::ArtifactMissingEvidence {
                evidence: "docs matrix schema",
            }
        );
    }

    fn export<'a>(crate_name: &'a str, symbol: &'a str) -> PublicApiExport<'a> {
        PublicApiExport { crate_name, symbol }
    }

    fn source_surfaces<'a>(dataflow_source: &'a str) -> [PublicApiSourceSurface<'a>; 4] {
        [
            source_surface(
                "vyre-driver-cuda",
                include_str!("../../../../vyre-driver-cuda/src/lib.rs"),
                false,
            ),
            source_surface(
                "vyre-driver",
                include_str!("../../../../vyre-driver/src/lib.rs"),
                true,
            ),
            source_surface(
                "vyre-libs",
                include_str!("../../../../vyre-libs/src/lib.rs"),
                false,
            ),
            source_surface("dataflow", dataflow_source, false),
        ]
    }

    fn dataflow_consumer_source() -> String {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../dataflow");
        let entries = fs::read_dir(&root)
            .expect("Fix: release public API boundary tests need adjacent dataflow consumers");
        for entry in entries {
            let path = entry
                .expect("Fix: dataflow consumer discovery must read directory entries")
                .path()
                .join("src/lib.rs");
            let Ok(source) = fs::read_to_string(&path) else {
                continue;
            };
            if source.contains("pub mod graph_layout;")
                && source.contains("mod output_scratch;")
                && source.contains("#[cfg(any(test, feature = \"cpu-parity\"))]")
            {
                return source;
            }
        }
        panic!(
            "Fix: could not discover a dataflow consumer source root with graph layout and scratch contracts"
        );
    }

    fn source_surface<'a>(
        crate_name: &'a str,
        source: &'a str,
        allow_public_pipeline_module: bool,
    ) -> PublicApiSourceSurface<'a> {
        PublicApiSourceSurface {
            crate_name,
            source,
            allow_public_pipeline_module,
        }
    }
}
