//! `dominator_tree`  -  exact immediate-dominator primitive.
//!
//! Computes the immediate dominator (`idom`) of every reachable node in a
//! control-flow graph with a single entry.  The primitive ships both a
//! Lengauer–Tarjan CPU reference oracle and a serial lane-0 GPU `Program`
//! builder that implements the Cooper–Harvey–Kennedy iterative fixpoint
//! using parent-pointer LCA on the idom tree.
//!
//! # Wire shape
//!
//! ```text
//! pg_edge_offsets : u32[node_count + 1]   // forward CSR
//! pg_edge_targets : u32[edge_count]       // forward CSR
//! pred_offsets    : u32[node_count + 1]   // predecessor CSR
//! pred_targets    : u32[pred_edge_count]  // predecessor CSR
//! idom_out        : u32[node_count]       // output idoms; NONE = unreachable
//! ```
//!
//! `idom_out[entry] == entry` for the entry block.  Unreachable nodes keep
//! the sentinel `NONE` (== `node_count`).
//!
//! # Soundness
//!
//! Exact for every reducible and irreducible single-entry CFG.  Multi-entry
//! graphs (no path from entry to some node that has predecessors) are not
//! rejected explicitly, but the resulting idom tree is undefined for the
//! disconnected component; callers should run `reachable` first if they need
//! strict guarantees.

#[path = "dominator_tree/program.rs"]
mod program;

#[cfg(any(test, feature = "cpu-parity"))]
#[path = "dominator_tree/alloc_helpers.rs"]
mod alloc_helpers;

#[cfg(any(test, feature = "cpu-parity"))]
#[path = "dominator_tree/lengauer_tarjan.rs"]
mod lengauer_tarjan;

#[cfg(any(test, feature = "cpu-parity"))]
#[path = "dominator_tree/cooper_harvey_kennedy.rs"]
mod cooper_harvey_kennedy;

#[cfg(any(test, feature = "cpu-parity"))]
#[path = "dominator_tree/cpu_ref.rs"]
mod cpu_ref;

#[cfg(feature = "inventory-registry")]
#[path = "dominator_tree/registry.rs"]
mod registry;

#[cfg(test)]
#[path = "dominator_tree/tests/mod.rs"]
mod tests;

pub use program::{
    dominator_tree_program, try_dominator_tree_program, validate_dominator_tree_inputs,
    DominatorTreeError, DominatorTreeLayout, IDOM_NONE, OP_ID,
};

#[cfg(any(test, feature = "cpu-parity"))]
pub use cooper_harvey_kennedy::cooper_harvey_kennedy_idoms;
#[cfg(any(test, feature = "cpu-parity"))]
pub use cpu_ref::{
    cpu_ref, idoms_to_dominator_sets, try_cpu_ref, try_cpu_ref_into, try_idoms_to_dominator_sets,
};
#[cfg(any(test, feature = "cpu-parity"))]
pub use lengauer_tarjan::{
    lengauer_tarjan_idoms, try_lengauer_tarjan_idoms, try_lengauer_tarjan_idoms_into,
    DominatorTreeCpuScratch,
};
