//! Adversarial contract tests for graph reachability, fixpoint, and
//! traversal invariants.
//!
//! Coverage: reachable, toposort, scc_decompose, path_reconstruct,
//! tensor_scc, csr_forward_or_changed, dominator_frontier, and
//! fixpoint convergence semantics. GPU acquisition: none  -  every
//! assertion uses CPU reference oracles.
//!
//! Implementation lives in two `include!`-d chunks under `__split/`.
#![cfg(feature = "graph")]
#![cfg(feature = "fixpoint")]
#![cfg(feature = "math")]

use std::collections::HashSet;

use vyre_primitives::fixpoint::bitset_fixpoint::*;
use vyre_primitives::graph::csr_forward_or_changed::cpu_ref as csr_cpu_ref;
use vyre_primitives::graph::dominator_frontier::cpu_ref as dom_cpu_ref;
use vyre_primitives::graph::path_reconstruct::cpu_ref as path_cpu_ref;
use vyre_primitives::graph::reachable::{reachable, reachable_program};
use vyre_primitives::graph::scc_decompose::cpu_ref as scc_cpu_ref;
use vyre_primitives::graph::toposort::{toposort, ToposortError};
use vyre_primitives::math::tensor_scc::{cpu_ref as tensor_scc_cpu_ref, tensor_scc_fixpoint};

// ---------------------------------------------------------------------------
// Reachable  -  directed reachability
// ---------------------------------------------------------------------------

fn hs(items: &[u32]) -> HashSet<u32> {
    items.iter().copied().collect()
}

#[path = "adversarial_graph_reachability_fixpoint/reachable_contracts.rs"]
mod reachable_contracts;
#[path = "adversarial_graph_reachability_fixpoint/toposort_contracts.rs"]
mod toposort_contracts;
#[path = "adversarial_graph_reachability_fixpoint/scc_path_tensor_contracts.rs"]
mod scc_path_tensor_contracts;
#[path = "adversarial_graph_reachability_fixpoint/csr_dominator_contracts.rs"]
mod csr_dominator_contracts;
#[path = "adversarial_graph_reachability_fixpoint/fixpoint_contracts.rs"]
mod fixpoint_contracts;
