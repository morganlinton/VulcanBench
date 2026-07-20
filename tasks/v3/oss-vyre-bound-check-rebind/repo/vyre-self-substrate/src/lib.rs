#![allow(
    clippy::doc_lazy_continuation,
    clippy::double_must_use,
    clippy::manual_div_ceil,
    clippy::needless_range_loop,
    clippy::collapsible_if,
    clippy::match_like_matches_macro,
    clippy::redundant_closure
)]
//! Self-substrate  -  vyre using its own primitives to compile/dispatch vyre.
//!
//! These modules realize the **recursion thesis** (#30): every Tier-2.5
//! primitive shipped in `vyre-primitives` also has a vyre-self consumer
//! here that uses the same Program at compile / dispatch time.
//!
//! # Layering (audit cleanup A10, 2026-04-30)
//!
//! Extracted from `vyre-driver/src/self_substrate/` into a dedicated
//! crate so the substrate-self-uses live at a layer that depends only
//! on `vyre-foundation` + `vyre-primitives`  -  eliminating the layering
//! muddle where backend-specific dispatch code and substrate self-uses
//! shared one home in `vyre-driver`.
//!
//! ```text
//!   vyre-foundation
//!         ↑
//!   vyre-primitives
//!         ↑
//!   vyre-self-substrate          ← THIS CRATE (no driver deps)
//!         ↑
//!   vyre-driver / vyre-runtime / vyre-libs / vyre-driver-{cuda,wgpu}
//! ```
//!
//! No cycles. Every consumer above this crate reaches the substrate
//! via `vyre_self_substrate::*` directly.
//!
//! `vyre-foundation` cannot consume `self_substrate` from here because
//! `self_substrate` depends on `vyre-primitives` which depends on
//! `vyre-foundation`  -  that's the cycle that justifies the dedicated
//! crate. Foundation has its own smaller substrate at
//! `vyre_foundation::pass_substrate` (with the math kernels it needs
//! inlined locally  -  same pattern Linux uses for arch-local libs vs
//! `lib/`).
//!
//! # Module list
//!
//! - `dataflow_fixpoint` (#26)  -  Region-graph dataflow fixpoint via
//!   `vyre-primitives::math::semiring_gemm` over the Region adjacency.
//! - `cost_model` (#28)  -  probabilistic dispatch cost model via
//!   `vyre-primitives::graph::sum_product_circuit` + conformal
//!   intervals from `vyre-primitives::math::conformal`.
//! - `vsa_fingerprint` (#29)  -  VSA op-cache key via
//!   `vyre-primitives::hash::hypervector`.
//! - `spectral_schedule` (#23)  -  spectral clustering of dispatch
//!   graph via `vyre-primitives::graph::chebyshev_filter` +
//!   `vyre-primitives::math::spectral_shape`.
//! - `differentiable_autotune` (#27)  -  differentiable autotuner via
//!   `vyre-primitives::math::differentiable`.
//! - `polyhedral_fusion` (#19)  -  polyhedral / affine fusion via
//!   `vyre-primitives::math::semiring_gemm` on the affine-dependency
//!   adjacency.
//! - `megakernel_schedule` (#22)  -  megakernel ILP relaxation via
//!   `vyre-primitives::opt::homotopy` continuation.
//! - `tensor_train_chain_fusion` (#6)  -  chain-shaped Region fusion via
//!   `vyre-primitives::math::tensor_train::tt_contract_step` contraction.
//! - `do_calculus_change_impact` (#36)  -  rule-graph change-impact analysis
//!   via `vyre-primitives::graph::do_calculus` graph surgery.
//! - `scallop_provenance` (#39)  -  GPU-resident rule provenance closure via
//!   `vyre-primitives::math::scallop_join` Datalog fixpoint.
//! - `matroid_megakernel_scheduler` (#46)  -  discrete fusion-grouping via
//!   matroid intersection augmenting paths. Complements
//!   `megakernel_schedule` (#22 homotopy continuous solver) with the
//!   exact combinatorial selection.
//! - `mori_zwanzig_region_coarsen` (#58)  -  Region-tree coarse-graining
//!   via Mori-Zwanzig projection. Reduces O(N²) all-pairs analyses to
//!   O(K²) at workspace scale with quantified projection error.
//! - `fmm_polyhedral_compress` (#51)  -  FMM hierarchical compression of
//!   #19 polyhedral fusion's all-pairs affinity. Drops cost from O(N²)
//!   to O(N log N) at workspace scale.
//! - `submodular_cache_eviction` (#45)  -  pipeline-cache eviction via
//!   submodular maximization. Replaces LRU's heuristic with the
//!   provably-(1-1/e) greedy approximation.
//! - `qsvt_matrix_function_fusion` (#34)  -  transport-based fusion
//!   analysis via QSVT-applied matrix functions. Computes Wasserstein
//!   distances on dispatch graphs in O(K·N²) instead of O(N³).
//! - `persistent_homology_loop_signature` (#15)  -  Region-tree loop
//!   topology via Vietoris-Rips filtration. Fusion-vs-fission decision
//!   informed by H₁ persistent features.
//! - `adjustment_set_pass_dependency` (#37)  -  optimizer pass-ordering
//!   validity via causal back-door analysis on the rewrite-precondition
//!   graph.
//! - `functorial_pass_composition` (#52)  -  IR transform passes as
//!   categorical functors. Compositionality, equational reasoning, free
//!   adjoint pairs  -  pass framework moves from hand-managed DAG to a
//!   typed functor-category.
//! - `string_diagram_ir_rewrite` (#53)  -  Vyre IR Region tree IS a
//!   string diagram in Cat(GPU buffers, Programs). Optimizer rewrites
//!   become string-diagram rewrites; coherence theorems give free
//!   correctness proofs.
//! - `planar_rewrite_pass_scheduler` (#11)  -  schedule batch IR rewrites
//!   onto disjoint sub-trees via planar non-overlapping selection.
//!   Drops dispatch count from O(N) sequential to O(log N) batched.

pub mod analysis;
pub mod data;
pub mod graph;
pub mod hardware;
pub mod integration;
pub mod logic;
pub mod math;
pub mod scheduling;
pub mod telemetry;

#[cfg(test)]
mod test_support;

/// Self-hosted optimizer keystone  -  the encoder + GPU passes that run
/// the compiler against its own substrate. Exposed at the lib root so
/// external consumers (driver-cuda parity tests, conform runners) can
/// reach `OptimizerDispatcher`, the per-pass `*_via_encoded` entry
/// points, and optimizer contract metadata without descending into
/// private module paths.
pub mod optimizer;

/// Backward-compatible facade for the optimizer contract modules.
pub mod optimization;

pub use analysis::{
    cost_model, dataflow_fixpoint, decision_telemetry, diagnostic_aggregation,
    diagnostic_comparison, effect_signature_check, incremental_invalidation,
    knowledge_compile_pass_precondition, linear_type_check, persistent_fixpoint_program,
    shape_smt_check,
};

pub use logic::{
    adjustment_set_pass_dependency, categorical_check, dnnf_compile, do_calculus_change_impact,
    functorial_pass_composition, string_diagram_ir_rewrite, zx_rewrite,
};

pub use data::{
    bitset_compression, bitset_summary, matroid_exact_megakernel, matroid_megakernel_scheduler,
    parsing_dispatch_pipeline, scallop_provenance, scallop_provenance_wide, vsa_fingerprint,
};

pub use telemetry::observability;

pub use scheduling::{
    branch_compaction, frontier_partitioning, frontier_typed_ir, megakernel_schedule,
    multi_corpus_batching, planar_rewrite_pass_scheduler, polyhedral_fusion, spectral_schedule,
    submodular_cache_eviction,
};

pub use integration::evidence::{
    benchmark_baselines, c_parser_benchmark_evidence, cuda_ptx_pattern_evidence,
    optimization_release_evidence,
};
pub use integration::{coverage, evidence, quality, release};

pub use optimizer::contracts::{
    cross_crate_perf_contracts, optimization_composition_contracts, optimization_pass_selection,
    optimization_registry, optimization_release_passes,
};

pub use integration::quality::{
    allocation_regression, architecture_boundary_map, contributor_module_map,
    crate_metadata_readiness, deep_review_gate, paradigm_shift_plan_audit, public_api_boundary,
    public_api_doctest_gate,
};

pub use integration::coverage::{
    analysis_coverage, c_dialect_matrix, clang_parity_dashboard, graph_layout_coverage,
    hostile_input_coverage, linux_corpus_parity, parser_semantic_safety, semantic_parity_coverage,
    test_taxonomy_coverage,
};

pub use graph::{
    adaptive_traverse, alias_registry, csr_bidirectional, csr_forward_or_changed,
    csr_frontier_queue_batch_memory, csr_frontier_queue_batch_resident,
    csr_frontier_queue_resident, dominator_frontier, exploded, level_wave_pass, motif,
    path_reconstruct, persistent_bfs, structural_kernel_pipeline, toposort,
    traversal_dispatch_pipeline, union_find_emit, vast_tree_walk,
};

pub use math::{
    amg_pass_solver, bellman_tn_order, differentiable_autotune, fmm_polyhedral_compress,
    kfac_autotune_step, mori_zwanzig_region_coarsen, multigrid_matroid_solver,
    natural_gradient_autotuner, persistent_homology_loop_signature, qsvt_matrix_function_fusion,
    sheaf_heterophilic_dispatch, sheaf_spectral_clustering, sinkhorn_dispatch_clustering,
    sinkhorn_full_clustering, tensor_network_fusion_order, tensor_train_chain_fusion,
    tensor_train_compression,
};

pub(crate) use hardware::dispatch_buffers;
pub use hardware::{
    device_resident_token_fact_graph, gpu_preprocessing_coverage, gpu_probe_contract,
    memory_ownership_contract,
};

pub use integration::release::{
    release_checklist_gate, release_completion_audit, release_gap_findings, release_gpu_evidence,
    release_launch_sequence, release_scope_docs, release_validation_matrix,
};
