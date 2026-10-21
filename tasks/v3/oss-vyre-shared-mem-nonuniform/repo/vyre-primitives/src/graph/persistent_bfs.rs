//! `persistent_bfs` - on-device multi-step BFS frontier expansion.
//!
//! The kernel copies `frontier_in` into `frontier_out`, then performs up to
//! `max_iters` forward traversal steps, accumulating reachable nodes into
//! `frontier_out` via atomic OR.  The first `min(max_iters, 4)` iterations
//! are unrolled and use a workgroup-local `wg_scratch` buffer to coalesce
//! per-workgroup change detection between steps.

#[path = "persistent_bfs/cpu_ref.rs"]
mod cpu_ref;
#[path = "persistent_bfs/dispatch_plan.rs"]
mod dispatch_plan;
#[path = "persistent_bfs/hash.rs"]
mod hash;
#[path = "persistent_bfs/layout.rs"]
mod layout;
#[path = "persistent_bfs/plan.rs"]
mod plan;
#[path = "persistent_bfs/program.rs"]
mod program;
#[path = "persistent_bfs/resident_plan.rs"]
mod resident_plan;
#[path = "persistent_bfs/validate.rs"]
mod validate;

#[cfg(feature = "inventory-registry")]
#[path = "persistent_bfs/registry.rs"]
mod registry;

#[cfg(test)]
#[path = "persistent_bfs/tests/mod.rs"]
mod tests;

#[cfg(any(test, feature = "cpu-parity"))]
pub use cpu_ref::{cpu_ref, try_cpu_ref, try_cpu_ref_into};
pub use hash::{persistent_bfs_layout_hash, persistent_bfs_program_layout_hash};
pub use layout::{
    persistent_bfs_batch_dispatch_grid, persistent_bfs_single_dispatch_grid,
    PersistentBfsPlanCacheKey, PersistentBfsStaticInputKey, BATCH_OP_ID, BINDING_CHANGED,
    BINDING_FRONTIER_IN, BINDING_FRONTIER_OUT, OP_ID, PERSISTENT_BFS_WORKGROUP_SIZE,
};
pub use plan::{
    copy_persistent_bfs_batch_seed_and_clear_changed_into, copy_persistent_bfs_seed_frontier_into,
    plan_persistent_bfs_dispatch, plan_persistent_bfs_resident_batch_dispatch,
    plan_persistent_bfs_resident_dispatch, validate_persistent_bfs_changed_flag,
};
pub use program::{bitset_words, persistent_bfs, persistent_bfs_batch, try_persistent_bfs_batch};
pub use validate::{
    validate_persistent_bfs_batch_frontiers, validate_persistent_bfs_frontier,
    validate_persistent_bfs_graph_layout, validate_persistent_bfs_inputs,
};

#[cfg(test)]
pub(crate) use {
    cpu_ref::{cpu_ref_into, try_cpu_ref_into_with_scratch, PersistentBfsCpuScratch},
    layout::{
        PersistentBfsBatchLayout, PersistentBfsFrontierLayout, PersistentBfsLayout,
        PersistentBfsPlanCacheKind,
    },
};
