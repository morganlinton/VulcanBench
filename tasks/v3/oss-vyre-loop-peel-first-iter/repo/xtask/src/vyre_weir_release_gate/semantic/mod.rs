//! Per-requirement semantic evidence checks.

mod c_parser_linux_subsystem;
mod conformance_hard_gate;
mod cpu_only_100x_proof;
mod crate_metadata;
mod cuda_first_path;
mod distributed_parser_coherence;
mod docs_evidence_linked;
mod final_completion_audit;
mod megakernel_default;
mod optimization_corpus_4096;
mod optimization_integration;
mod proof_workloads_12;
mod release_hygiene;
mod test_architecture;
mod version_story;
mod weir_analysis_integration;
mod wgpu_fallback;

use std::path::Path;

use super::types::Requirement;

pub(super) fn run_semantic_requirement_checks(
    requirement: &Requirement,
    base_dir: &Path,
    failures: &mut Vec<String>,
) {
    match requirement.id.as_str() {
        "alias-aware-upgrades" => optimization_integration::check(requirement, base_dir, failures),
        "c-parser-linux-subsystem" => {
            c_parser_linux_subsystem::check(requirement, base_dir, failures)
        }
        "conformance-hard-gate" => conformance_hard_gate::check(requirement, base_dir, failures),
        "cpu-only-100x-proof" => cpu_only_100x_proof::check(requirement, base_dir, failures),
        "crate-metadata" => crate_metadata::check(requirement, base_dir, failures),
        "cuda-first-path" => cuda_first_path::check(requirement, base_dir, failures),
        "distributed-parser-coherence" => {
            distributed_parser_coherence::check(requirement, base_dir, failures)
        }
        "docs-evidence-linked" => docs_evidence_linked::check(requirement, base_dir, failures),
        "egraph-saturation" => optimization_integration::check(requirement, base_dir, failures),
        "exhaustive-verification" => test_architecture::check(requirement, base_dir, failures),
        "final-completion-audit" => final_completion_audit::check(requirement, base_dir, failures),
        "megakernel-default" => megakernel_default::check(requirement, base_dir, failures),
        "modular-test-architecture" => test_architecture::check(requirement, base_dir, failures),
        "optimization-benchmark-proof" => {
            optimization_integration::check(requirement, base_dir, failures)
        }
        "optimization-corpus-4096" => {
            optimization_corpus_4096::check(requirement, base_dir, failures)
        }
        "proof-workloads-12" => proof_workloads_12::check(requirement, base_dir, failures),
        "release-hygiene" => release_hygiene::check(requirement, base_dir, failures),
        "version-story" => version_story::check(requirement, base_dir, failures),
        "weir-analysis-integration" => {
            weir_analysis_integration::check(requirement, base_dir, failures)
        }
        "wgpu-fallback" => wgpu_fallback::check(requirement, base_dir, failures),
        _ => {}
    }
}
