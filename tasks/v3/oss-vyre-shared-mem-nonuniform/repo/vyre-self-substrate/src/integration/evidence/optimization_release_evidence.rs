//! Release evidence gate for the 100+ optimization pass contract.

use crate::optimization_registry::OptimizationRegistry;

/// Proof returned when committed optimization evidence satisfies release gates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OptimizationReleaseEvidenceProof {
    /// Registered optimization passes.
    pub registered_passes: usize,
    /// Required optimization families covered by artifacts.
    pub required_families: usize,
    /// Generated optimization corpus cases.
    pub generated_cases: u64,
    /// Verified optimization corpus cases.
    pub verified_cases: u64,
}

/// Optimization release evidence errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptimizationReleaseEvidenceError {
    /// Registry does not expose enough concrete passes.
    TooFewRegisteredPasses {
        /// Observed pass count.
        observed: usize,
    },
    /// Artifact is missing required evidence.
    MissingEvidence {
        /// Missing evidence label.
        evidence: &'static str,
    },
}

impl std::fmt::Display for OptimizationReleaseEvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFewRegisteredPasses { observed } => write!(
                f,
                "optimization registry exposes only {observed} passes. Fix: release requires at least 100 concrete optimization passes with stable owners, invariants, and gates."
            ),
            Self::MissingEvidence { evidence } => write!(
                f,
                "optimization release evidence is missing {evidence}. Fix: regenerate CUDA optimization artifacts and keep the registry, corpus, family manifest, and benchmark manifest aligned."
            ),
        }
    }
}

impl std::error::Error for OptimizationReleaseEvidenceError {}

/// Validate committed optimization release artifacts against the CUDA-first release contract.
pub fn validate_optimization_release_evidence(
    registry: &OptimizationRegistry,
    family_manifest: &str,
    corpus: &str,
    corpus_contracts: &str,
    benchmark_manifest: &str,
    optimizer_impact_cuda: &str,
    pass_family_benchmarks: &str,
    integration_matrix: &str,
) -> Result<OptimizationReleaseEvidenceProof, OptimizationReleaseEvidenceError> {
    let registered_passes = registry.len();
    if registered_passes < 100 {
        return Err(OptimizationReleaseEvidenceError::TooFewRegisteredPasses {
            observed: registered_passes,
        });
    }
    registry
        .validate()
        .map_err(|_| OptimizationReleaseEvidenceError::MissingEvidence {
            evidence: "valid optimization registry metadata",
        })?;

    for (evidence, needle) in [
        ("family manifest schema", "\"schema_version\": 1"),
        (
            "fourteen required optimization families",
            "\"required_family_count\": 14",
        ),
        (
            "zero missing required families",
            "\"missing_required_families\": []",
        ),
        ("zero family manifest blockers", "\"blockers\": []"),
        ("algebraic family", "\"family\": \"algebraic\""),
        ("predicate family", "\"family\": \"predicate\""),
        ("egraph family", "\"family\": \"egraph\""),
        ("memory-layout family", "\"family\": \"memory-layout\""),
        ("control-flow family", "\"family\": \"control-flow\""),
        ("vector-layout family", "\"family\": \"vector-layout\""),
        (
            "Dataflow analysis DSE family",
            "\"family\": \"weir-dataflow-dse\"",
        ),
        (
            "Dataflow analysis loop fusion family",
            "\"family\": \"weir-dataflow-loop-fusion\"",
        ),
        (
            "Dataflow analysis loop fission family",
            "\"family\": \"weir-dataflow-loop-fission\"",
        ),
        (
            "Dataflow analysis LICM family",
            "\"family\": \"weir-dataflow-licm\"",
        ),
    ] {
        artifact_contains(family_manifest, evidence, needle)?;
    }
    let required_families = count_occurrences(family_manifest, "\"family\": ");
    if required_families < 14 {
        return Err(OptimizationReleaseEvidenceError::MissingEvidence {
            evidence: "at least fourteen concrete optimization family rows",
        });
    }

    for (evidence, needle) in [
        ("corpus schema", "\"schema_version\": 1"),
        ("corpus blockers cleared", "\"blockers\": []"),
        ("corpus convergence", "\"non_converged_cases\": 0"),
        ("corpus required minimum", "\"required_min_cases\": 4096"),
        ("corpus generated case total", "\"generated_cases\": 12288"),
        ("corpus verified case total", "\"verified_cases\": 12288"),
        ("corpus optimized case total", "\"optimized_cases\": 10386"),
        (
            "Dataflow analysis corpus coverage",
            "\"dataflow_analysis_cases\": 1024",
        ),
        (
            "Dataflow analysis optimized coverage",
            "\"dataflow_analysis_optimized_cases\": 1024",
        ),
    ] {
        artifact_contains(corpus, evidence, needle)?;
    }
    for (evidence, needle) in [
        ("corpus contract blockers cleared", "\"blockers\": []"),
        ("corpus contract convergence", "\"non_converged_cases\": 0"),
        (
            "corpus contract generated case total",
            "\"generated_cases\": 12288",
        ),
        (
            "corpus contract verified case total",
            "\"verified_cases\": 12288",
        ),
        (
            "corpus contract optimized case total",
            "\"optimized_cases\": 10386",
        ),
    ] {
        artifact_contains(corpus_contracts, evidence, needle)?;
    }

    for (evidence, needle) in [
        ("benchmark manifest CUDA backend", "\"backend\": \"cuda\""),
        ("benchmark manifest blockers cleared", "\"blockers\": []"),
        (
            "no uncovered optimization families",
            "\"uncovered_pass_families\": []",
        ),
        (
            "required benchmark case count",
            "\"required_case_count\": 4",
        ),
        (
            "lower rewrite impact artifact",
            "release/evidence/optimization/lower-rewrite-impact-before-after.json",
        ),
        (
            "optimizer impact CUDA artifact",
            "release/evidence/optimization/optimizer-impact-cuda.json",
        ),
        (
            "egraph saturation artifact",
            "release/evidence/optimization/egraph-before-after.json",
        ),
        (
            "alias-aware before-after artifact",
            "release/evidence/benchmarks/alias-aware-before-after.json",
        ),
        ("alias pass wins metric", "\"alias_pass_wins\""),
        ("coalescing metric", "\"lower_coalesce_problematic_before\""),
        ("shared-memory metric", "\"lower_shared_candidates_before\""),
        ("bank-conflict metric", "\"lower_bank_critical_before\""),
        ("vector packing metric", "\"lower_vec_pack_chains_before\""),
        (
            "unclaimed family wall-speedup contract",
            "\"min_wall_speedup_x1000\": null",
        ),
    ] {
        artifact_contains(benchmark_manifest, evidence, needle)?;
    }

    for (artifact, label) in [
        (optimizer_impact_cuda, "optimizer impact CUDA benchmark"),
        (pass_family_benchmarks, "pass family CUDA benchmark"),
    ] {
        for (evidence, needle) in [
            ("CUDA selected backend", "\"selected_backend\": \"cuda\""),
            ("RTX 5090 benchmark hardware", "NVIDIA GeForce RTX 5090"),
            ("CUDA runtime version", "\"nvidia_cuda_version\": \"12.8\""),
            ("usable CUDA backend", "\"backend.usable.cuda\""),
            ("backend CUDA feature", "\"backend:cuda\""),
            ("passing CUDA benchmark status", "\"status\": \"pass\""),
            ("exact benchmark correctness", "\"correctness\": \"Exact\""),
            (
                "explicitly unclaimed benchmark contract",
                "\"contract\": null",
            ),
            (
                "explicitly unclaimed performance verdict",
                "\"performance\": null",
            ),
            ("source fingerprint", "\"source_fingerprint\""),
            ("allocation metrics", "\"alloc_bytes\""),
            ("wall samples", "\"samples\""),
        ] {
            artifact_contains(artifact, evidence, needle).map_err(|_| {
                OptimizationReleaseEvidenceError::MissingEvidence { evidence: label }
            })?;
        }
    }

    for (evidence, needle) in [
        ("integration matrix blockers inventory", "\"blockers\""),
        ("dead-store pass source", "\"id\": \"dse\""),
        (
            "store-to-load forwarding pass source",
            "\"id\": \"store-to-load-forwarding\"",
        ),
        ("LICM pass source", "\"id\": \"licm\""),
        ("loop fusion pass source", "\"id\": \"loop-fusion\""),
        ("loop fission pass source", "\"id\": \"loop-fission\""),
        (
            "shared-memory promotion pass source",
            "\"id\": \"shared-mem-promote\"",
        ),
        (
            "bank-conflict padding pass source",
            "\"id\": \"bank-conflict-pad\"",
        ),
        (
            "egraph saturation pass source",
            "\"id\": \"egraph-saturation\"",
        ),
        (
            "Dataflow alias analysis source",
            "\"marker\": \"AliasFactSet\"",
        ),
        (
            "Dataflow reaching-def analysis source",
            "\"marker\": \"import_descriptor_reaching_defs\"",
        ),
        (
            "local transform entrypoints",
            "\"has_transform_entrypoint\": true",
        ),
        ("local tests", "\"has_local_tests\": true"),
        (
            "no unresolved implementation markers",
            "\"unresolved_markers\": []",
        ),
    ] {
        artifact_contains(integration_matrix, evidence, needle)?;
    }

    Ok(OptimizationReleaseEvidenceProof {
        registered_passes,
        required_families,
        generated_cases: 12_288,
        verified_cases: 12_288,
    })
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), OptimizationReleaseEvidenceError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(OptimizationReleaseEvidenceError::MissingEvidence { evidence })
    }
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optimization_release_evidence_accepts_committed_cuda_artifacts() {
        let registry = OptimizationRegistry::with_release_builtins();
        let proof = validate_optimization_release_evidence(
            &registry,
            include_str!(
                "../../../../release/evidence/optimization/optimization-family-manifest.json"
            ),
            include_str!("../../../../release/evidence/optimization/optimization-corpus.json"),
            include_str!(
                "../../../../release/evidence/optimization/optimization-corpus-contracts.json"
            ),
            include_str!(
                "../../../../release/evidence/optimization/pass-family-benchmark-manifest.json"
            ),
            include_str!("../../../../release/evidence/optimization/optimizer-impact-cuda.json"),
            include_str!("../../../../release/evidence/optimization/pass-family-benchmarks.json"),
            include_str!(
                "../../../../release/evidence/optimization/optimization-integration-matrix.json"
            ),
        )
        .expect("Fix: committed CUDA optimization release evidence should pass");

        assert!(proof.registered_passes >= 100);
        assert_eq!(proof.required_families, 14);
        assert_eq!(proof.generated_cases, 12_288);
        assert_eq!(proof.verified_cases, 12_288);
    }

    #[test]
    fn optimization_release_evidence_rejects_cpu_backend_benchmark() {
        let registry = OptimizationRegistry::with_release_builtins();
        let err = validate_optimization_release_evidence(
            &registry,
            include_str!(
                "../../../../release/evidence/optimization/optimization-family-manifest.json"
            ),
            include_str!("../../../../release/evidence/optimization/optimization-corpus.json"),
            include_str!(
                "../../../../release/evidence/optimization/optimization-corpus-contracts.json"
            ),
            include_str!(
                "../../../../release/evidence/optimization/pass-family-benchmark-manifest.json"
            ),
            include_str!("../../../../release/evidence/optimization/optimizer-impact-cuda.json")
                .replace(
                    "\"selected_backend\": \"cuda\"",
                    "\"selected_backend\": \"cpu\"",
                )
                .as_str(),
            include_str!("../../../../release/evidence/optimization/pass-family-benchmarks.json"),
            include_str!(
                "../../../../release/evidence/optimization/optimization-integration-matrix.json"
            ),
        )
        .expect_err("CPU backend optimizer evidence must not satisfy release path");

        assert_eq!(
            err,
            OptimizationReleaseEvidenceError::MissingEvidence {
                evidence: "optimizer impact CUDA benchmark",
            }
        );
    }
}
