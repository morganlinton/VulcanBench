//! Contributor-facing optimizer pass catalog.
//!
//! The scheduler owns execution. This module owns discovery/reporting: every
//! registered pass gets a stable entry with owner, phase, invariant, and
//! benchmark contract so release tooling and contributors can answer "where
//! does this optimization live and how is it proved?" without reading the
//! scheduler.

use super::{
    registered_pass_registrations, CostModelFamily, OptimizerError, OptimizerProfile,
    PassBoundaryClass, PassMetadata, PassPhase,
};

/// Stable catalog entry for one registered optimizer pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationCatalogEntry {
    /// Whether this entry is an executable pass or a discoverable rule entry.
    pub kind: OptimizationCatalogEntryKind,
    /// Stable pass name.
    pub name: &'static str,
    /// Human owner for the optimization family.
    pub owner: &'static str,
    /// Scheduler phase.
    pub phase: PassPhase,
    /// Boundary class enforced by the scheduler/profile selector.
    pub boundary_class: PassBoundaryClass,
    /// Invariant this pass is required to preserve.
    pub invariant: &'static str,
    /// Benchmark family that must cover this optimization.
    pub benchmark: &'static str,
    /// Cost model family declared by the pass.
    pub cost_model_family: CostModelFamily,
    /// Whether this pass preserves public buffer ABI.
    pub preserves_abi: bool,
    /// Backend/runtime capabilities required before the pass may run.
    pub requires_caps: &'static [&'static str],
}

/// Source/kind of one optimizer catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationCatalogEntryKind {
    /// A runnable [`super::ProgramPassRegistration`] discovered by inventory.
    ExecutablePass,
    /// A named rule surfaced for contributor discovery and benchmark coverage.
    SupplementalRule,
}

impl OptimizationCatalogEntry {
    /// Build a catalog entry from pass metadata.
    #[must_use]
    pub fn from_metadata(metadata: PassMetadata) -> Self {
        Self {
            kind: OptimizationCatalogEntryKind::ExecutablePass,
            name: metadata.name,
            owner: owner_for(metadata),
            phase: metadata.phase,
            boundary_class: metadata.boundary_class,
            invariant: invariant_for(metadata),
            benchmark: benchmark_for(metadata),
            cost_model_family: metadata.cost_model_family,
            preserves_abi: metadata.preserves_abi,
            requires_caps: metadata.requires_caps,
        }
    }
}

/// Return every registered optimization in scheduled order.
///
/// # Errors
/// Returns [`OptimizerError::Scheduling`] when the pass dependency graph is
/// invalid.
pub fn optimization_catalog() -> Result<Vec<OptimizationCatalogEntry>, OptimizerError> {
    let mut catalog = registered_pass_registrations()?
        .iter()
        .map(|registration| OptimizationCatalogEntry::from_metadata(registration.metadata))
        .collect::<Vec<_>>();
    catalog.extend_from_slice(SUPPLEMENTAL_OPTIMIZATION_RULES);
    Ok(catalog)
}

/// Return registered optimizations accepted by a profile in scheduled order.
///
/// # Errors
/// Returns [`OptimizerError::Scheduling`] when the pass dependency graph is
/// invalid.
pub fn optimization_catalog_for_profile(
    profile: OptimizerProfile,
) -> Result<Vec<OptimizationCatalogEntry>, OptimizerError> {
    Ok(optimization_catalog()?
        .into_iter()
        .filter(|entry| {
            profile.accepts(PassMetadata {
                name: entry.name,
                requires: &[],
                invalidates: &[],
                phase: entry.phase,
                boundary_class: entry.boundary_class,
                requires_caps: entry.requires_caps,
                preserves_abi: entry.preserves_abi,
                cost_model_family: entry.cost_model_family,
            })
        })
        .collect())
}

fn owner_for(metadata: PassMetadata) -> &'static str {
    match metadata.phase {
        PassPhase::Dataflow => "dataflow",
        PassPhase::Megakernel => "vyre-runtime",
        PassPhase::Specialization => "vyre-backend-specialization",
        PassPhase::Loop | PassPhase::ScalarAlgebra | PassPhase::Canonicalization => {
            "vyre-foundation-optimizer"
        }
        PassPhase::Memory | PassPhase::FusionCse | PassPhase::Sync | PassPhase::Cleanup => {
            "vyre-foundation-optimizer"
        }
        PassPhase::Unclassified => match metadata.boundary_class {
            PassBoundaryClass::RuntimeAware => "vyre-runtime",
            PassBoundaryClass::DomainSpecific => "domain-crate",
            _ => "vyre-foundation-optimizer",
        },
    }
}

fn invariant_for(metadata: PassMetadata) -> &'static str {
    match metadata.boundary_class {
        PassBoundaryClass::AbiPreserving => {
            "preserves public buffer ABI and externally visible program semantics"
        }
        PassBoundaryClass::AbiChanging => {
            "requires explicit caller opt-in before changing public buffer ABI"
        }
        PassBoundaryClass::BackendAware => {
            "fires only when declared backend capabilities are present"
        }
        PassBoundaryClass::RuntimeAware => {
            "preserves runtime residency, launch-order, and synchronization contracts"
        }
        PassBoundaryClass::DomainSpecific => {
            "preserves owning domain graph/fact encoding contracts"
        }
        PassBoundaryClass::Unknown => "must not enter release/profile pipelines until classified",
    }
}

fn benchmark_for(metadata: PassMetadata) -> &'static str {
    match metadata.cost_model_family {
        CostModelFamily::Scalar => "foundation.optimizer.scalar",
        CostModelFamily::Loop => "foundation.optimizer.loop",
        CostModelFamily::Memory => "foundation.optimizer.memory",
        CostModelFamily::Fusion => "foundation.optimizer.fusion",
        CostModelFamily::Sync => "foundation.optimizer.sync",
        CostModelFamily::Dataflow => "dataflow.optimizer",
        CostModelFamily::Megakernel => "vyre.megakernel.optimizer",
        CostModelFamily::Unknown => "foundation.optimizer.classification-gap",
    }
}

const SCALAR_RULE_INVARIANT: &str =
    "preserves scalar expression value while reducing redundant GPU work";
const LOOP_RULE_INVARIANT: &str =
    "preserves loop iteration semantics while reducing loop or index overhead";
const MEMORY_RULE_INVARIANT: &str =
    "preserves memory-visible behavior while reducing traffic or redundant storage";
const FUSION_RULE_INVARIANT: &str =
    "preserves visible effects while eliminating duplicate computation or launches";
const DATAFLOW_RULE_INVARIANT: &str =
    "preserves dataflow fixed-point semantics while reducing frontier or lattice work";
const MEGAKERNEL_RULE_INVARIANT: &str =
    "preserves resident megakernel execution semantics while reducing launches, readback, or queue work";

macro_rules! scalar_rule {
    ($name:literal) => {
        OptimizationCatalogEntry {
            kind: OptimizationCatalogEntryKind::SupplementalRule,
            name: $name,
            owner: "vyre-foundation-rules",
            phase: PassPhase::ScalarAlgebra,
            boundary_class: PassBoundaryClass::AbiPreserving,
            invariant: SCALAR_RULE_INVARIANT,
            benchmark: "foundation.optimizer.scalar.rules",
            cost_model_family: CostModelFamily::Scalar,
            preserves_abi: true,
            requires_caps: &[],
        }
    };
}

macro_rules! loop_rule {
    ($name:literal) => {
        OptimizationCatalogEntry {
            kind: OptimizationCatalogEntryKind::SupplementalRule,
            name: $name,
            owner: "vyre-foundation-rules",
            phase: PassPhase::Loop,
            boundary_class: PassBoundaryClass::AbiPreserving,
            invariant: LOOP_RULE_INVARIANT,
            benchmark: "foundation.optimizer.loop.rules",
            cost_model_family: CostModelFamily::Loop,
            preserves_abi: true,
            requires_caps: &[],
        }
    };
}

macro_rules! memory_rule {
    ($name:literal) => {
        OptimizationCatalogEntry {
            kind: OptimizationCatalogEntryKind::SupplementalRule,
            name: $name,
            owner: "vyre-foundation-rules",
            phase: PassPhase::Memory,
            boundary_class: PassBoundaryClass::AbiPreserving,
            invariant: MEMORY_RULE_INVARIANT,
            benchmark: "foundation.optimizer.memory.rules",
            cost_model_family: CostModelFamily::Memory,
            preserves_abi: true,
            requires_caps: &[],
        }
    };
}

macro_rules! fusion_rule {
    ($name:literal) => {
        OptimizationCatalogEntry {
            kind: OptimizationCatalogEntryKind::SupplementalRule,
            name: $name,
            owner: "vyre-foundation-rules",
            phase: PassPhase::FusionCse,
            boundary_class: PassBoundaryClass::AbiPreserving,
            invariant: FUSION_RULE_INVARIANT,
            benchmark: "foundation.optimizer.fusion.rules",
            cost_model_family: CostModelFamily::Fusion,
            preserves_abi: true,
            requires_caps: &[],
        }
    };
}

macro_rules! dataflow_rule {
    ($name:literal) => {
        OptimizationCatalogEntry {
            kind: OptimizationCatalogEntryKind::SupplementalRule,
            name: $name,
            owner: "dataflow-rules",
            phase: PassPhase::Dataflow,
            boundary_class: PassBoundaryClass::DomainSpecific,
            invariant: DATAFLOW_RULE_INVARIANT,
            benchmark: "dataflow.optimizer.rules",
            cost_model_family: CostModelFamily::Dataflow,
            preserves_abi: true,
            requires_caps: &[],
        }
    };
}

macro_rules! megakernel_rule {
    ($name:literal) => {
        OptimizationCatalogEntry {
            kind: OptimizationCatalogEntryKind::SupplementalRule,
            name: $name,
            owner: "vyre-runtime-rules",
            phase: PassPhase::Megakernel,
            boundary_class: PassBoundaryClass::RuntimeAware,
            invariant: MEGAKERNEL_RULE_INVARIANT,
            benchmark: "vyre.megakernel.optimizer.rules",
            cost_model_family: CostModelFamily::Megakernel,
            preserves_abi: true,
            requires_caps: &[],
        }
    };
}

const SUPPLEMENTAL_OPTIMIZATION_RULES: &[OptimizationCatalogEntry] = &[
    scalar_rule!("const_fold.unary.logical_not_involution"),
    scalar_rule!("const_fold.unary.negate_involution"),
    scalar_rule!("const_fold.unary.reverse_bits_involution"),
    scalar_rule!("const_fold.unary.bitnot_involution"),
    scalar_rule!("const_fold.unary.negate_sub_flip"),
    scalar_rule!("const_fold.unary.abs_neg"),
    scalar_rule!("const_fold.unary.abs_idempotent"),
    scalar_rule!("const_fold.unary.floor_idempotent"),
    scalar_rule!("const_fold.unary.ceil_idempotent"),
    scalar_rule!("const_fold.unary.round_idempotent"),
    scalar_rule!("const_fold.unary.trunc_idempotent"),
    scalar_rule!("const_fold.unary.sign_idempotent"),
    scalar_rule!("const_fold.unary.floor_trunc_subsumption"),
    scalar_rule!("const_fold.unary.ceil_trunc_subsumption"),
    scalar_rule!("const_fold.unary.round_trunc_subsumption"),
    scalar_rule!("const_fold.unary.inverse_sqrt_one"),
    scalar_rule!("const_fold.unary.reciprocal_one"),
    scalar_rule!("const_fold.unary.sqrt_one"),
    scalar_rule!("const_fold.unary.sqrt_zero"),
    scalar_rule!("const_fold.unary.sin_zero"),
    scalar_rule!("const_fold.unary.cos_zero"),
    scalar_rule!("const_fold.unary.tan_zero"),
    scalar_rule!("const_fold.unary.exp_zero"),
    scalar_rule!("const_fold.unary.exp2_zero"),
    scalar_rule!("const_fold.unary.log_one"),
    scalar_rule!("const_fold.unary.log2_one"),
    scalar_rule!("const_fold.unary.asin_zero"),
    scalar_rule!("const_fold.unary.acos_one"),
    scalar_rule!("const_fold.unary.atan_zero"),
    scalar_rule!("const_fold.unary.tanh_zero"),
    scalar_rule!("const_fold.unary.sinh_zero"),
    scalar_rule!("const_fold.unary.cosh_zero"),
    scalar_rule!("const_fold.unary.popcount_zero"),
    scalar_rule!("const_fold.unary.clz_zero"),
    scalar_rule!("const_fold.unary.ctz_zero"),
    scalar_rule!("const_fold.unary.reverse_bits_zero"),
    scalar_rule!("const_fold.unary.popcount_literal"),
    scalar_rule!("const_fold.unary.clz_literal"),
    scalar_rule!("const_fold.unary.ctz_literal"),
    scalar_rule!("const_fold.unary.reverse_bits_literal"),
    scalar_rule!("const_fold.unary.bitnot_literal"),
    scalar_rule!("const_fold.unary.negate_i32_literal"),
    scalar_rule!("const_fold.unary.abs_u32_literal"),
    scalar_rule!("const_fold.unary.abs_i32_literal"),
    scalar_rule!("const_fold.select.bool_true"),
    scalar_rule!("const_fold.select.bool_false"),
    scalar_rule!("const_fold.select.u32_zero_false"),
    scalar_rule!("const_fold.select.u32_nonzero_true"),
    scalar_rule!("const_fold.select.identical_branches"),
    scalar_rule!("const_fold.select.logical_not_swap"),
    scalar_rule!("const_fold.select.bool_to_u32_cast"),
    scalar_rule!("const_fold.select.inverted_bool_to_u32_cast"),
    scalar_rule!("const_fold.select.true_branch_select_fusion"),
    scalar_rule!("const_fold.select.false_branch_select_fusion"),
    scalar_rule!("const_fold.fma.literal"),
    scalar_rule!("const_fold.fma.left_identity_multiplier"),
    scalar_rule!("const_fold.fma.right_identity_multiplier"),
    scalar_rule!("const_fold.fma.left_zero_multiplier"),
    scalar_rule!("const_fold.fma.right_zero_multiplier"),
    scalar_rule!("const_fold.binop.add_fma_left"),
    scalar_rule!("const_fold.binop.add_fma_right"),
    scalar_rule!("const_fold.binop.sub_fma_left"),
    scalar_rule!("const_fold.binop.sub_fma_right"),
    scalar_rule!("const_fold.binop.add_zero_left"),
    scalar_rule!("const_fold.binop.add_zero_right"),
    scalar_rule!("const_fold.binop.sub_self"),
    scalar_rule!("const_fold.binop.sub_zero_right"),
    scalar_rule!("const_fold.binop.add_reassociate_right_literal"),
    scalar_rule!("const_fold.binop.add_reassociate_left_literal"),
    scalar_rule!("const_fold.binop.distribute_add_u32"),
    scalar_rule!("const_fold.binop.distribute_add_i32"),
    scalar_rule!("const_fold.binop.distribute_sub_u32"),
    scalar_rule!("const_fold.binop.distribute_sub_i32"),
    scalar_rule!("strength_reduce.horner_polynomial_int"),
    scalar_rule!("strength_reduce.shift_add_decompose_plus"),
    scalar_rule!("strength_reduce.shift_add_decompose_minus"),
    scalar_rule!("strength_reduce.naf_shift_add_chain"),
    scalar_rule!("strength_reduce.mul_power_of_two_shift"),
    scalar_rule!("strength_reduce.mul_negative_const_negate"),
    scalar_rule!("strength_reduce.div_power_of_two_shift"),
    scalar_rule!("strength_reduce.float_div_reciprocal"),
    scalar_rule!("strength_reduce.self_inverse_select"),
    scalar_rule!("strength_reduce.shift_negation_fma"),
    scalar_rule!("strength_reduce.complement_bounds"),
    loop_rule!("loop.trip_zero_eliminate"),
    loop_rule!("loop.lower_bound_normalize"),
    loop_rule!("loop.bound_tighten"),
    loop_rule!("loop.var_range_fold"),
    loop_rule!("loop.peel"),
    loop_rule!("loop.unroll"),
    loop_rule!("loop.strip_mine"),
    loop_rule!("loop.fission"),
    loop_rule!("loop.fusion"),
    loop_rule!("loop.licm"),
    loop_rule!("loop.software_pipeline"),
    loop_rule!("loop.redundant_bound_check_elide"),
    memory_rule!("memory.const_buffer_fold"),
    memory_rule!("memory.dead_buffer_elim"),
    memory_rule!("memory.dead_store_elim"),
    memory_rule!("memory.decode_scan_fuse"),
    memory_rule!("memory.read_only_load_hoist"),
    memory_rule!("memory.store_to_load_forward"),
    memory_rule!("memory.vectorization"),
    fusion_rule!("fusion.cse.structural_expr_dedup"),
    fusion_rule!("fusion.dce.dead_let_eliminate"),
    fusion_rule!("fusion.dce.unreachable_eliminate"),
    fusion_rule!("fusion.dce.const_loop_empty"),
    fusion_rule!("fusion.dce.const_truth"),
    fusion_rule!("fusion.region_fusion_hint"),
    fusion_rule!("fusion.cross_rule_cse"),
    dataflow_rule!("dataflow.layout_normalization"),
    dataflow_rule!("dataflow.frontier_density_switch"),
    dataflow_rule!("dataflow.sparse_frontier_expand"),
    dataflow_rule!("dataflow.dense_bitset_propagate"),
    dataflow_rule!("dataflow.bitset_compression"),
    dataflow_rule!("dataflow.fixed_point_buffer_reuse"),
    dataflow_rule!("dataflow.resident_csr_reuse"),
    dataflow_rule!("dataflow.multi_query_batch"),
    dataflow_rule!("dataflow.lattice_join_idempotence_skip"),
    dataflow_rule!("dataflow.delta_frontier_prune"),
    megakernel_rule!("megakernel.allocation_reuse"),
    megakernel_rule!("megakernel.launch_fusion"),
    megakernel_rule!("megakernel.readback_minimization"),
    megakernel_rule!("megakernel.queue_no_conflict_fast_path"),
    megakernel_rule!("megakernel.frontier_density_worker_shed"),
    megakernel_rule!("megakernel.device_side_convergence"),
    megakernel_rule!("megakernel.plan_cache_reuse"),
    megakernel_rule!("megakernel.barrier_wave_grouping"),
    megakernel_rule!("megakernel.zero_copy_result_compaction"),
    megakernel_rule!("megakernel.resident_graph_reuse"),
];

#[cfg(test)]
mod tests;
