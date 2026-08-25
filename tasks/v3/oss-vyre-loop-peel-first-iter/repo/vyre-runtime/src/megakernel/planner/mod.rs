//! Megakernel planning, fusion selection, sizing, and work-item contracts.
//!
//! This module is the runtime-owned home for the resident execution policy
//! shared by runtime planning and concrete driver dispatchers.

mod barriers;
mod caps;
mod config;
mod cross_pipeline;
mod fusion;
mod geometry;
mod grid;
#[cfg(feature = "self-substrate-adapters")]
mod programs;
mod provenance;
mod sizing;
mod whole_megakernel_opt;

pub use barriers::{elide_value_flow_barriers, BarrierElisionReport};
pub use caps::{MegakernelCaps, MegakernelReport, MegakernelTelemetry, MegakernelWorkItem};
pub use config::{MegakernelConfig, MegakernelWorkloadHints};
pub use cross_pipeline::{
    plan_cross_pipeline_fusion, CrossPipelineFusionPlan, PipelineFusionBreak, PipelineFusionSegment,
};
pub use fusion::{
    plan_compact_fusion_into, prune_dead_arms_inplace, select_fused_subset,
    select_fused_subset_checked_into, select_fused_subset_compact,
    select_fused_subset_compact_checked_into, select_fused_subset_compact_into,
    select_fused_subset_into, select_fused_subset_pruned, select_fused_subset_pruned_into,
    select_fused_subset_with_rate, select_optimal_fused_subset, shared_prologue_length,
    CompactFusionPlanningScratch, FusionSelectionError, FusionSelectionScratch,
};
pub use geometry::{
    default_worker_groups_from_limits, dispatch_grid_for, padded_slot_count, worker_workgroup_size,
    MegakernelLaunchGeometry,
};
pub use grid::{MegakernelGridLimits, MegakernelGridPlan, MegakernelGridRequest};
#[cfg(feature = "self-substrate-adapters")]
pub use programs::{
    build_bellman_tn_order_program, build_kfac_autotune_step_program,
    build_persistent_fixpoint_program, build_scallop_provenance_wide_program,
    build_sinkhorn_clustering_program, build_sinkhorn_full_clustering_program,
};
pub use provenance::build_scallop_lineage_with_program_and_scratch;
#[cfg(feature = "self-substrate-adapters")]
pub use provenance::build_scallop_lineage_with_scratch;
pub use sizing::MegakernelSizingPolicy;
#[cfg(any(test, feature = "legacy-infallible"))]
pub use whole_megakernel_opt::{
    detect_cross_arm_redundancy, prune_redundant_work_items_into,
    prune_redundant_work_items_with_scratch_into,
};
pub use whole_megakernel_opt::{
    try_detect_cross_arm_redundancy, try_prune_redundant_work_items_into,
    try_prune_redundant_work_items_with_scratch_into, CrossArmRedundancy,
    RedundantWorkItemPruneScratch,
};

#[cfg(test)]
use super::task::TaskWorkItem;

#[cfg(test)]
mod tests {
    include!("../core_tests.rs");
}
