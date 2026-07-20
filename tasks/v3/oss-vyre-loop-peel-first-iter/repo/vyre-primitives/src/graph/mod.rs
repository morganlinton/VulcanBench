//! Tier 2.5 graph primitives.
//!
//! The path IS the interface. Callers write
//! `vyre_primitives::graph::toposort::toposort(...)`; no wildcard
//! re-exports.

/// Kahn's-algorithm topological sort.
pub mod toposort;

/// GPU-resident depth-wave dispatcher for bottom-up callee-before-
/// caller computations (e.g. downstream_dataflow::summary's per-procedure summary
/// fixpoint with topological ordering). Composes Node::Loop +
/// Node::Barrier { ordering: vyre::memory_model::MemoryOrdering::SeqCst } + a per-lane depth predicate; no new sub-op.
pub mod level_wave;

/// Reachability scan  -  given a source set + edge list, which nodes
/// are transitively reachable?
pub mod reachable;

/// Canonical 5-buffer ProgramGraph ABI (CSR wire format, shared by
/// every graph primitive).
pub mod program_graph;
pub(crate) mod scratch;

pub(crate) fn checked_csr_offset_count(node_count: u32, op_name: &str) -> Result<u32, String> {
    node_count.checked_add(1).ok_or_else(|| {
        format!(
            "Fix: {op_name} node_count + 1 overflows u32 for node_count={node_count}. Shard the CSR graph before GPU dispatch."
        )
    })
}

/// One BFS step that accumulates into frontier_out and reports changes.
pub mod csr_forward_or_changed;
/// One BFS frontier step over ProgramGraph CSR.
pub mod csr_forward_traverse;
/// One persistent-BFS workgroup step with coalesced change detection.
pub mod persistent_bfs_step;

/// Reverse-direction in-place frontier step that reports changes.
pub mod csr_backward_or_changed;
/// Reverse-direction BFS frontier step.
pub mod csr_backward_traverse;

/// Total outgoing-edge count over the active frontier. Building block
/// for load-balanced one-thread-per-edge expansion that beats naive
/// one-thread-per-node on power-law graphs.
pub mod csr_frontier_degree_sum;
/// Device-side active-frontier queue materialization and queue-driven CSR
/// expansion for sparse dataflow waves.
pub mod csr_frontier_queue;
mod csr_frontier_step;
/// Queue-to-queue sparse CSR delta expansion for GPU-resident fixpoint waves.
pub mod csr_queue_delta;
/// Mixed queue traversal that keeps low-degree rows scalar and sends only hubs
/// to row-strided teams.
pub mod csr_queue_split;
/// Row-strided queue-driven CSR expansion for high-degree active rows.
pub mod csr_queue_strided;

/// One BFS step over BOTH forward + backward edges.
pub mod csr_bidirectional;

/// Dominance-frontier query for SSA phi placement.
pub mod dominator_frontier;

/// Exact immediate-dominator tree (Lengauer–Tarjan CPU reference +
/// Cooper–Harvey–Kennedy serial GPU kernel).
pub mod dominator_tree;

/// Walk parent-pointer array from a target back to the root; emit
/// the materialized path into a u32 buffer.
pub mod path_reconstruct;

/// Motif witness helpers over ProgramGraph edge constraints.
pub mod motif;

/// Forward-Backward strongly-connected components decomposition over
/// ProgramGraph CSR.
pub mod scc_decompose;

/// Vector nearest-neighbor graph construction plus graph-ranking parity.
pub mod vector_neighbor_graph;

/// Exploded-supergraph builder  -  (CFG × fact) pairs as graph vertices
/// so IFDS/IDE reduces to `csr_forward_traverse`.
pub mod exploded;

/// Adaptive CSR / dense bitmatrix traversal  -  picks representation
/// per tile based on frontier density.
pub mod adaptive_traverse;
/// Shared state/index frontier headers for graph and automata worklists.
pub mod state_index_frontier;

/// Persistent BFS  -  multi-step frontier expansion in a single dispatch.
pub mod persistent_bfs;

/// IR Extension interface registering Alias-solving opcodes to the compiler front-end.
pub mod alias_registry;

/// Lock-free Union-Find for subset alias resolving constraint grids.
pub mod union_find;
pub mod vast_tree_walk;

/// 3D sub-warp dataflow tensors
pub mod tensor_flow_forward;
#[cfg(test)]
mod tensor_flow_forward_tests;

/// K-step Chebyshev polynomial filter on a graph Laplacian. Composes
/// from `vyre-primitives::math::semiring_gemm` (each step is one
/// `n × n · n × 1` Real-semiring matvec). Same Program serves user
/// dialects (spectral GNN, security spectral anomaly) AND vyre-self
/// (#23 spectral analysis of dispatch graph for fusion clustering).
pub mod chebyshev_filter;

/// Sum-product circuit (probabilistic circuit) per-node evaluator.
/// Composes with `level_wave_program` for bottom-up evaluation. Same
/// Program serves user probabilistic-ML dialects AND vyre-self
/// dispatch cost-model (#28).
pub mod sum_product_circuit;

/// Pearl do-calculus graph surgery  -  incoming-edge deletion for
/// `do(X = x)` interventions. Same Program serves user causal-
/// inference dialects AND vyre-self change-impact analysis (do(rule_X)
/// on rule dependency graph predicts cache invalidation downstream).
pub mod do_calculus;

/// Back-door / front-door adjustment set predicates for causal
/// inference (#37). Composes with #36 do-calculus for full ID-
/// algorithm pipelines.
pub mod adjustment_set;

/// Matroid intersection  -  exchange-graph BFS step for combinatorial
/// scheduling and bipartite matching (#46). Self-consumer: vyre's
/// megakernel scheduler fusion-grouping (#22).
pub mod matroid;

/// Sheaf neural network diagonal-form diffusion step (#31). User:
/// heterophilic GNN, typed call-graph anomalies. Self: vyre's
/// dispatch graph as heterophilic sheaf.
pub mod sheaf;

/// Probabilistic knowledge compilation d-DNNF evaluator (#38).
/// Composes with #10 sum_product_circuit for probability-weighted
/// variants.
pub mod knowledge_compile;

/// Functorial data migration (#52). Schema-functor
/// application as graph rewrite.
pub mod functorial;

/// Monoidal-category sequential composition (#53).
/// String-diagram compilation primitive.
pub mod string_diagram;
