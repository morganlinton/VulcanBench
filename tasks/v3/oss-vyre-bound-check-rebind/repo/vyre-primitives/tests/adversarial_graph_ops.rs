//! Failure-oriented adversarial integration tests for graph primitives.
//!
//! Coverage: csr_forward_traverse, csr_backward_traverse, toposort,
//! scc_decompose, path_reconstruct  -  hostile boundaries, empty graphs,
//! edge-kind diversity (M8), malformed CSR, cross-word bitsets.
#![cfg(all(feature = "graph", feature = "cpu-parity"))]

use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::csr_backward_traverse::cpu_ref as bwd_cpu_ref;
use vyre_primitives::graph::csr_forward_traverse::cpu_ref as fwd_cpu_ref;
use vyre_primitives::graph::csr_frontier_queue::{
    frontier_to_queue_cpu, frontier_word_block_prefix_to_queue_parallel,
    frontier_word_counts_scan_pass_a, frontier_words_to_queue_parallel,
};
use vyre_primitives::graph::path_reconstruct::cpu_ref as path_cpu_ref;
use vyre_primitives::graph::scc_decompose::cpu_ref as scc_cpu_ref;
use vyre_primitives::graph::toposort::{toposort, ToposortError};
use vyre_reference::value::Value;

#[path = "adversarial_graph_ops/backward_traverse_contracts.rs"]
mod backward_traverse_contracts;
#[path = "adversarial_graph_ops/forward_traverse_contracts.rs"]
mod forward_traverse_contracts;
#[path = "adversarial_graph_ops/frontier_queue_contracts.rs"]
mod frontier_queue_contracts;
#[path = "adversarial_graph_ops/path_reconstruct_contracts.rs"]
mod path_reconstruct_contracts;
#[path = "adversarial_graph_ops/scc_contracts.rs"]
mod scc_contracts;
#[path = "adversarial_graph_ops/toposort_contracts.rs"]
mod toposort_contracts;
