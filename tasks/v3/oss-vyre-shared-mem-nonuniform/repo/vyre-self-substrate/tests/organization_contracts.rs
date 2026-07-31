#[path = "organization_contracts/root_structure_contracts.rs"]
mod root_structure_contracts;
#[path = "organization_contracts/graph_module_contracts.rs"]
mod graph_module_contracts;
#[path = "organization_contracts/csr_queue_contracts.rs"]
mod csr_queue_contracts;
#[path = "organization_contracts/persistent_bfs_contracts.rs"]
mod persistent_bfs_contracts;
#[path = "organization_contracts/graph_primitive_contracts.rs"]
mod graph_primitive_contracts;
#[path = "organization_contracts/graph_adaptive_contracts.rs"]
mod graph_adaptive_contracts;
#[path = "organization_contracts/domain_core_contracts.rs"]
mod domain_core_contracts;
#[path = "organization_contracts/domain_release_contracts.rs"]
mod domain_release_contracts;

// Organization contracts for the self-substrate crate.

use std::path::Path;

const GRAPH_WRAPPERS: &[&str] = &[
    "adaptive_traverse.rs",
    "alias_registry.rs",
    "csr_bidirectional.rs",
    "csr_forward_or_changed.rs",
    "csr_frontier_queue_batch_memory.rs",
    "csr_frontier_queue_batch_resident.rs",
    "csr_frontier_queue_resident.rs",
    "dominator_frontier.rs",
    "exploded.rs",
    "level_wave_pass.rs",
    "motif.rs",
    "path_reconstruct.rs",
    "persistent_bfs.rs",
    "toposort.rs",
    "union_find_emit.rs",
    "vast_tree_walk.rs",
];

const CONSOLIDATED_GRAPH_WRAPPERS: &[(&str, &str)] = &[
    ("adaptive_traverse.rs", "adaptive_traverse"),
    ("alias_registry.rs", "alias_registry"),
    ("csr_bidirectional.rs", "csr_bidirectional"),
    ("csr_forward_or_changed.rs", "csr_forward_or_changed"),
    ("dominator_frontier.rs", "dominator_frontier"),
    ("exploded.rs", "exploded"),
    ("motif.rs", "motif"),
    ("path_reconstruct.rs", "path_reconstruct"),
    ("persistent_bfs.rs", "persistent_bfs"),
    ("toposort.rs", "toposort"),
    ("vast_tree_walk.rs", "vast_tree_walk"),
];

const RELEASE_GATES: &[&str] = &[
    "release_checklist_gate.rs",
    "release_completion_audit.rs",
    "release_gap_findings.rs",
    "release_gpu_evidence.rs",
    "release_launch_sequence.rs",
    "release_scope_docs.rs",
    "release_validation_matrix.rs",
];

const HARDWARE_MODULES: &[&str] = &[
    "dispatch_buffers.rs",
    "device_resident_token_fact_graph.rs",
    "gpu_preprocessing_coverage.rs",
    "gpu_probe_contract.rs",
    "memory_ownership_contract.rs",
    "scratch.rs",
];

const EVIDENCE_MODULES: &[&str] = &[
    "benchmark_baselines.rs",
    "c_parser_benchmark_evidence.rs",
    "cuda_ptx_pattern_evidence.rs",
    "optimization_release_evidence.rs",
];

const COVERAGE_MODULES: &[&str] = &[
    "c_dialect_matrix.rs",
    "clang_parity_dashboard.rs",
    "hostile_input_coverage.rs",
    "linux_corpus_parity.rs",
    "parser_semantic_safety.rs",
    "semantic_parity_coverage.rs",
    "test_taxonomy_coverage.rs",
    "analysis_coverage.rs",
    "graph_layout_coverage.rs",
];

const MATH_MODULES: &[&str] = &[
    "amg_pass_solver.rs",
    "bellman_tn_order.rs",
    "differentiable_autotune.rs",
    "fmm_polyhedral_compress.rs",
    "kfac_autotune_step.rs",
    "mori_zwanzig_region_coarsen.rs",
    "multigrid_matroid_solver.rs",
    "natural_gradient_autotuner.rs",
    "persistent_homology_loop_signature.rs",
    "qsvt_matrix_function_fusion.rs",
    "sheaf_heterophilic_dispatch.rs",
    "sheaf_spectral_clustering.rs",
    "sinkhorn_dispatch_clustering.rs",
    "sinkhorn_full_clustering.rs",
    "tensor_network_fusion_order.rs",
    "tensor_train_chain_fusion.rs",
    "tensor_train_compression.rs",
];

const OPTIMIZER_MODULES: &[&str] = &[
    "canonicalize_via_encoded.rs",
    "const_fold_via_encoded.rs",
    "const_prop.rs",
    "cross_scope_cse.rs",
    "cse_via_encoded.rs",
    "dce_program.rs",
    "dce_via_encoded.rs",
    "dead_branch.rs",
    "dispatcher.rs",
    "encode.rs",
    "expr_arena.rs",
    "licm.rs",
    "pattern_match_via_encoded.rs",
    "pipeline.rs",
    "pipeline_resident.rs",
    "pipeline_resident_decode.rs",
    "validate_via_encoded.rs",
];

const OPTIMIZER_CONTRACT_MODULES: &[&str] = &[
    "cross_crate_perf_contracts.rs",
    "optimization_composition_contracts.rs",
    "optimization_pass_selection.rs",
    "optimization_registry.rs",
    "optimization_release_passes.rs",
];

const QUALITY_MODULES: &[&str] = &[
    "allocation_regression.rs",
    "architecture_boundary_map.rs",
    "contributor_module_map.rs",
    "cpu_fallback_reachability.rs",
    "crate_metadata_readiness.rs",
    "deep_review_gate.rs",
    "paradigm_shift_plan_audit.rs",
    "public_api_boundary.rs",
    "public_api_doctest_gate.rs",
];

const ANALYSIS_MODULES: &[&str] = &[
    "cost_model.rs",
    "dataflow_fixpoint.rs",
    "decision_telemetry.rs",
    "diagnostic_aggregation.rs",
    "diagnostic_comparison.rs",
    "effect_signature_check.rs",
    "incremental_invalidation.rs",
    "knowledge_compile_pass_precondition.rs",
    "linear_type_check.rs",
    "persistent_fixpoint_program.rs",
    "shape_smt_check.rs",
];

const SCHEDULING_MODULES: &[&str] = &[
    "branch_compaction.rs",
    "frontier_partitioning.rs",
    "frontier_typed_ir.rs",
    "megakernel_schedule.rs",
    "multi_corpus_batching.rs",
    "planar_rewrite_pass_scheduler.rs",
    "polyhedral_fusion.rs",
    "spectral_schedule.rs",
    "submodular_cache_eviction.rs",
];

const LOGIC_MODULES: &[&str] = &[
    "adjustment_set_pass_dependency.rs",
    "categorical_check.rs",
    "dnnf_compile.rs",
    "do_calculus_change_impact.rs",
    "functorial_pass_composition.rs",
    "string_diagram_ir_rewrite.rs",
    "zx_rewrite.rs",
];

const DATA_MODULES: &[&str] = &[
    "bitset_compression.rs",
    "bitset_summary.rs",
    "matroid_exact_megakernel.rs",
    "matroid_megakernel_scheduler.rs",
    "scallop_provenance.rs",
    "scallop_provenance_wide.rs",
    "vsa_fingerprint.rs",
];

const TELEMETRY_MODULES: &[&str] = &["observability.rs"];

const DOMAIN_MODULES: &[(&str, &[&str])] = &[
    ("analysis", ANALYSIS_MODULES),
    ("integration/coverage", COVERAGE_MODULES),
    ("data", DATA_MODULES),
    ("integration/evidence", EVIDENCE_MODULES),
    ("graph", GRAPH_WRAPPERS),
    ("hardware", HARDWARE_MODULES),
    ("logic", LOGIC_MODULES),
    ("math", MATH_MODULES),
    ("optimizer", OPTIMIZER_MODULES),
    ("integration/quality", QUALITY_MODULES),
    ("integration/release", RELEASE_GATES),
    ("scheduling", SCHEDULING_MODULES),
    ("telemetry", TELEMETRY_MODULES),
];


fn read_graph_wrapper_source(wrapper_path: &Path) -> String {
    let actual_wrapper_path = if wrapper_path.exists() {
        wrapper_path.to_path_buf()
    } else {
        let stem = wrapper_path
            .file_stem()
            .unwrap_or_else(|| panic!("{} must have a stem", wrapper_path.display()));
        wrapper_path
            .parent()
            .unwrap_or_else(|| panic!("{} must have a parent", wrapper_path.display()))
            .join(stem)
            .join("mod.rs")
    };
    let mut source = std::fs::read_to_string(&actual_wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", actual_wrapper_path.display()));
    let Some(parent) = actual_wrapper_path.parent() else {
        return source;
    };
    let Some(stem) = actual_wrapper_path.file_stem() else {
        return source;
    };
    let child_dir = if actual_wrapper_path
        .file_name()
        .is_some_and(|name| name == "mod.rs")
    {
        parent.to_path_buf()
    } else {
        parent.join(stem)
    };
    if !child_dir.is_dir() {
        return source;
    }
    let mut child_modules = std::fs::read_dir(&child_dir)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", child_dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|err| {
                    panic!("{} entry must be readable: {err}", child_dir.display())
                })
                .path()
        })
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect::<Vec<_>>();
    child_modules.sort();
    for child in child_modules {
        source.push('\n');
        source.push_str(
            &std::fs::read_to_string(&child)
                .unwrap_or_else(|err| panic!("{} must be readable: {err}", child.display())),
        );
    }
    source
}

