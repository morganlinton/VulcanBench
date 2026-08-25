//! Graph-dispatch substrate wrappers.
//!
//! These modules wire `vyre-primitives::graph` programs into
//! self-substrate dispatch, scratch, evidence, and observability contracts.
//! Primitive graph logic stays in `vyre-primitives`; this module owns only
//! self-hosting integration.

pub mod adaptive_traverse;
pub mod alias_registry;
pub mod csr_bidirectional;
pub mod csr_forward_or_changed;
pub mod csr_frontier_queue_batch_memory;
pub mod csr_frontier_queue_batch_resident;
pub mod csr_frontier_queue_resident;
pub(crate) mod csr_frontier_queue_scratch;
pub(crate) mod dispatch_bridge;
pub mod dominator_frontier;
pub mod exploded;
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) mod frontier;
pub mod level_wave_pass;
pub mod motif;
pub mod path_reconstruct;
pub mod persistent_bfs;
pub(crate) mod plan_cache;
pub(crate) mod resident_handles;
pub mod structural_kernel_pipeline;
pub mod toposort;
pub mod traversal_dispatch_pipeline;
pub mod union_find_emit;
pub mod vast_tree_walk;
