//! Occupancy-aware grid scaling for megakernels.
//!
//! Runtime-owned megakernel planning and launch policy.

#[cfg(feature = "self-substrate-adapters")]
pub use super::planner::{
    build_bellman_tn_order_program, build_kfac_autotune_step_program,
    build_persistent_fixpoint_program, build_sinkhorn_clustering_program,
};
pub use super::planner::{
    default_worker_groups_from_limits, dispatch_grid_for, padded_slot_count, select_fused_subset,
    select_fused_subset_compact_into, select_fused_subset_into, select_fused_subset_pruned,
    select_fused_subset_pruned_into, select_fused_subset_with_rate, select_optimal_fused_subset,
    worker_workgroup_size, FusionSelectionScratch, MegakernelGridLimits, MegakernelGridPlan,
    MegakernelGridRequest, MegakernelLaunchGeometry, MegakernelSizingPolicy,
};
#[cfg(any(test, feature = "legacy-infallible"))]
pub use super::policy::{diffuse_priority_across_siblings, diffuse_priority_across_siblings_into};
pub use super::policy::{
    try_diffuse_priority_across_siblings, try_diffuse_priority_across_siblings_into,
    MegakernelExecutionMode, MegakernelLaunchPolicy, MegakernelLaunchRecommendation,
    MegakernelLaunchRequest, MegakernelQueuePressure, PriorityRequeueAccounting,
};
