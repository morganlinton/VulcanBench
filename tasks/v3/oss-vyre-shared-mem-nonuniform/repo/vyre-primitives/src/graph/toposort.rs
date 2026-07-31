//! Kahn-style topological sort with LIFO worklist  -  CPU reference +
//! single-invocation GPU `Program` builder.
//!
//! Consumed by optimizer reaching-defs, dominator-tree, and graph-IR analyses
//! that need a DAG walk.
//!
//! AUDIT_2026-04-24 F-TS-04: `toposort_program` emits a single-invocation
//! Program that runs Kahn's algorithm serially on lane 0. The serial
//! lane-0 builder is the current Tier-2.5 contract because topological
//! ordering has a loop-carried dependency; callers that need large-DAG
//! throughput compose this with graph partitioning or SCC batching.
//!
//! AUDIT_2026-04-24 F-TS-02: the classical Kahn presentation uses a
//! FIFO queue (BFS-ish). This module uses a stack (LIFO) via
//! `Vec::pop` because it is O(1), has better cache locality on the
//! worklist, and produces an equally valid topological order  -  both
//! orderings satisfy the Kahn invariant (a node is emitted only
//! after all its prerequisites). If a caller needs stable BFS order
//! for deterministic diffs, swap in a `VecDeque` worklist; the
//! correctness of the sort does not depend on the worklist policy.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::graph::toposort";
/// Canonical dispatch input label for CSR offsets.
pub const TOPOSORT_OFFSETS_BUFFER: &str = "toposort offsets";
/// Canonical dispatch input label for CSR targets.
pub const TOPOSORT_TARGETS_BUFFER: &str = "toposort targets";
/// Canonical dispatch scratch label for indegrees.
pub const TOPOSORT_INDEGREE_SCRATCH_BUFFER: &str = "toposort indeg_scratch";
/// Canonical dispatch scratch label for the work queue.
pub const TOPOSORT_QUEUE_SCRATCH_BUFFER: &str = "toposort queue_scratch";
/// Canonical dispatch output label for the emitted order.
pub const TOPOSORT_ORDER_OUT_BUFFER: &str = "toposort order_out";
/// Single-lane Kahn dispatch grid.
pub const TOPOSORT_DISPATCH_GRID: [u32; 3] = [1, 1, 1];

/// Errors from topological sorting.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToposortError {
    /// The input graph contains a cycle  -  returned with the first
    /// node id that participates in the cycle, for diagnostic use.
    Cycle {
        /// One node id on the cycle. Callers can walk the adjacency
        /// list from here to enumerate the full cycle.
        node: u32,
    },
    /// An edge references a node id not present in `node_count`.
    UnknownNode {
        /// Offending edge index.
        edge: usize,
        /// The out-of-range node id that tripped the check.
        node: u32,
    },
    /// A node's dependency count exceeded the `u32` counter used by the
    /// compact scheduler representation.
    IndegreeOverflow {
        /// Node whose dependency count overflowed.
        node: u32,
    },
    /// Kahn's invariant was violated after input validation, indicating
    /// inconsistent derived adjacency state.
    InconsistentState {
        /// Actionable diagnostic.
        message: String,
    },
}

/// Errors from CSR topological-sort shape or order validation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToposortCsrError {
    /// CSR row pointers or targets are malformed for the declared node count.
    BadCsr {
        /// Actionable diagnostic.
        message: String,
    },
    /// The supplied topological order is not a full valid permutation.
    BadOrder {
        /// Actionable diagnostic.
        message: String,
    },
}

/// Validated dispatch layout for primitive-native CSR topological sorting.
///
/// The primitive owns these derived counts so dispatch wrappers do not fork CSR
/// offset or node scratch sizing rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToposortCsrLayout {
    /// Number of nodes accepted by the primitive.
    pub node_count: u32,
    /// Number of words required by node-indexed scratch and output buffers.
    pub node_words: usize,
    /// Number of words required by the CSR offsets buffer.
    pub offset_words: usize,
    /// Number of words required by the CSR targets buffer.
    pub target_words: usize,
}

/// Primitive-owned dispatch plan for CSR topological sort.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToposortCsrDispatchPlan {
    /// Validated CSR layout.
    pub layout: ToposortCsrLayout,
    /// Dispatch grid override.
    pub grid: [u32; 3],
    /// Words in the offsets input buffer.
    pub offset_words: usize,
    /// Words in the targets input buffer.
    pub target_words: usize,
    /// Words in each node-indexed scratch/output buffer.
    pub node_words: usize,
}

/// Primitive-owned identity for reusable topological-sort static inputs.
///
/// Dispatch wrappers use this key to decide whether the CSR graph inputs can
/// remain resident across calls. Keeping it in the primitive prevents each
/// wrapper from inventing a private fingerprint contract for the same graph
/// representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToposortCsrStaticInputKey {
    /// Number of nodes in the CSR graph.
    pub node_count: u32,
    /// Words in node-indexed scratch/output buffers.
    pub node_words: usize,
    /// Words in the CSR offsets buffer.
    pub offset_words: usize,
    /// Words in the CSR targets buffer.
    pub target_words: usize,
    /// Stable content fingerprint for CSR offsets.
    pub offsets_hash: u64,
    /// Stable content fingerprint for CSR targets.
    pub targets_hash: u64,
}

impl ToposortCsrDispatchPlan {
    /// Build the single-lane topological-sort program for this plan.
    #[must_use]
    pub fn program(&self) -> Program {
        toposort_program(
            self.layout.node_count,
            TOPOSORT_OFFSETS_BUFFER,
            TOPOSORT_TARGETS_BUFFER,
            TOPOSORT_INDEGREE_SCRATCH_BUFFER,
            TOPOSORT_QUEUE_SCRATCH_BUFFER,
            TOPOSORT_ORDER_OUT_BUFFER,
        )
    }

    /// Return the primitive-owned cache identity for this plan's static graph inputs.
    ///
    /// # Errors
    ///
    /// Returns [`ToposortCsrError::BadCsr`] if the supplied CSR slices no
    /// longer match the validated dispatch plan shape.
    pub fn static_input_key(
        &self,
        offsets: &[u32],
        targets: &[u32],
    ) -> Result<ToposortCsrStaticInputKey, ToposortCsrError> {
        if offsets.len() != self.offset_words {
            return Err(ToposortCsrError::BadCsr {
                message: format!(
                    "Fix: toposort_csr static key expected {} offset words, got {}.",
                    self.offset_words,
                    offsets.len()
                ),
            });
        }
        if targets.len() != self.target_words {
            return Err(ToposortCsrError::BadCsr {
                message: format!(
                    "Fix: toposort_csr static key expected {} target words, got {}.",
                    self.target_words,
                    targets.len()
                ),
            });
        }
        Ok(ToposortCsrStaticInputKey {
            node_count: self.layout.node_count,
            node_words: self.node_words,
            offset_words: self.offset_words,
            target_words: self.target_words,
            offsets_hash: toposort_csr_slice_fingerprint(offsets),
            targets_hash: toposort_csr_slice_fingerprint(targets),
        })
    }
}

/// Stable primitive-owned fingerprint for CSR topological-sort u32 slices.
#[must_use]
pub fn toposort_csr_slice_fingerprint(values: &[u32]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in (values.len() as u64).to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    for value in values {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

/// CPU reference over the primitive-native CSR adjacency shape.
///
/// `offsets` has `node_count + 1` entries and `targets` stores outgoing
/// edges from each prerequisite node to its dependent nodes. The returned
/// order is valid iff every prerequisite appears before every dependent.
///
/// # Errors
///
/// Returns [`ToposortCsrError::BadCsr`] when the CSR shape is malformed and
/// [`ToposortCsrError::BadOrder`] only if derived state violates the
/// topological-order contract after input validation.
pub fn toposort_csr(
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
) -> Result<Vec<u32>, ToposortCsrError> {
    let mut order = Vec::new();
    toposort_csr_into(node_count, offsets, targets, &mut order)?;
    Ok(order)
}

/// Caller-owned workspace for repeated CSR topological-sort CPU oracle runs.
///
/// The CPU oracle is used heavily by conformance and CUDA parity paths. Keeping
/// indegree and queue storage outside the call lets proof runners amortize heap
/// growth across thousands of generated graphs without changing the public
/// allocating convenience API.
#[derive(Debug, Default, Clone)]
pub struct ToposortCsrScratch {
    /// Per-node incoming-edge counts rebuilt for each run.
    pub indeg: Vec<u32>,
    /// Zero-indegree work queue consumed by Kahn traversal.
    pub queue: Vec<u32>,
}

impl ToposortCsrScratch {
    /// Create an empty reusable topological-sort workspace.
    pub fn new() -> Self {
        Self::default()
    }
}

/// CPU reference over primitive-native CSR adjacency, reusing caller storage.
///
/// # Errors
///
/// Returns [`ToposortCsrError::BadCsr`] when CSR validation fails and
/// [`ToposortCsrError::BadOrder`] when the derived order violates the
/// primitive contract.
pub fn toposort_csr_into(
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
    order: &mut Vec<u32>,
) -> Result<(), ToposortCsrError> {
    let mut scratch = ToposortCsrScratch::default();
    toposort_csr_into_with_scratch(node_count, offsets, targets, order, &mut scratch)
}

/// CPU reference over primitive-native CSR adjacency with caller-owned output
/// and scratch storage.
///
/// # Errors
///
/// Returns [`ToposortCsrError::BadCsr`] when CSR validation fails and
/// [`ToposortCsrError::BadOrder`] when the derived order violates the
/// primitive contract. Validation happens before any caller-owned storage is
/// cleared, so rejected inputs do not clobber reusable buffers.
pub fn toposort_csr_into_with_scratch(
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
    order: &mut Vec<u32>,
    scratch: &mut ToposortCsrScratch,
) -> Result<(), ToposortCsrError> {
    let layout = validate_toposort_csr_inputs(node_count, offsets, targets)?;
    order.clear();
    scratch.indeg.clear();
    scratch.queue.clear();
    if node_count == 0 {
        return Ok(());
    }

    let node_words = layout.node_words;
    crate::graph::scratch::reserve_graph_items_with(
        &mut scratch.indeg,
        node_words,
        "toposort CSR CPU oracle",
        "toposort_csr indegree scratch",
        toposort_csr_allocation,
    )?;
    scratch.indeg.resize(node_words, 0);
    for (idx, &target) in targets.iter().enumerate() {
        scratch.indeg[target as usize] =
            scratch.indeg[target as usize]
                .checked_add(1)
                .ok_or_else(|| ToposortCsrError::BadCsr {
                    message: format!(
                    "Fix: toposort_csr target node {target} indegree overflowed at targets[{idx}]."
                ),
                })?;
    }

    crate::graph::scratch::reserve_graph_items_with(
        &mut scratch.queue,
        node_words,
        "toposort CSR CPU oracle",
        "toposort_csr zero-indegree queue",
        toposort_csr_allocation,
    )?;
    for node in 0..node_count {
        if scratch.indeg[node as usize] == 0 {
            scratch.queue.push(node);
        }
    }
    crate::graph::scratch::reserve_graph_items_with(
        order,
        node_words,
        "toposort CSR CPU oracle",
        "toposort_csr output order",
        toposort_csr_allocation,
    )?;
    while let Some(node) = scratch.queue.pop() {
        order.push(node);
        let start = offsets[node as usize] as usize;
        let end = offsets[node as usize + 1] as usize;
        for (edge_offset, &dependent) in targets[start..end].iter().enumerate() {
            let slot = &mut scratch.indeg[dependent as usize];
            *slot = slot
                .checked_sub(1)
                .ok_or_else(|| ToposortCsrError::BadOrder {
                    message: format!(
                    "Fix: toposort_csr indegree underflow for edge {} from {node} to {dependent}.",
                    start + edge_offset
                ),
                })?;
            if *slot == 0 {
                scratch.queue.push(dependent);
            }
        }
    }

    validate_toposort_csr_order_with_layout(&layout, offsets, targets, order)
}

/// Validate primitive-native CSR input shape.
///
/// # Errors
///
/// Returns [`ToposortCsrError::BadCsr`] when offsets are the wrong length, not
/// monotonic, inconsistent with `targets`, or when a target is out of range.
pub fn validate_toposort_csr_inputs(
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
) -> Result<ToposortCsrLayout, ToposortCsrError> {
    if node_count == 0 {
        if offsets != [0] || !targets.is_empty() {
            return Err(ToposortCsrError::BadCsr {
                message:
                    "Fix: toposort_csr zero-node graph requires offsets == [0] and empty targets."
                        .to_string(),
            });
        }
        return Ok(ToposortCsrLayout {
            node_count,
            node_words: 0,
            offset_words: 1,
            target_words: 0,
        });
    }
    let expected_offsets =
        (node_count as usize)
            .checked_add(1)
            .ok_or_else(|| ToposortCsrError::BadCsr {
                message: format!(
                    "Fix: toposort_csr node_count + 1 overflows usize for node_count={node_count}."
                ),
            })?;
    if offsets.len() != expected_offsets {
        return Err(ToposortCsrError::BadCsr {
            message: format!(
                "Fix: toposort_csr requires offsets.len() == node_count + 1, got len={}, node_count={node_count}.",
                offsets.len()
            ),
        });
    }
    if offsets[0] != 0 {
        return Err(ToposortCsrError::BadCsr {
            message: format!(
                "Fix: toposort_csr requires offsets[0] == 0, got {}.",
                offsets[0]
            ),
        });
    }
    for (idx, pair) in offsets.windows(2).enumerate() {
        if pair[0] > pair[1] {
            return Err(ToposortCsrError::BadCsr {
                message: format!(
                    "Fix: toposort_csr offsets must be monotonic, but offsets[{idx}]={} > offsets[{}]={}.",
                    pair[0],
                    idx + 1,
                    pair[1]
                ),
            });
        }
    }
    if offsets[node_count as usize] as usize != targets.len() {
        return Err(ToposortCsrError::BadCsr {
            message: format!(
                "Fix: toposort_csr offsets[node_count] must equal targets.len(), got {} vs {}.",
                offsets[node_count as usize],
                targets.len()
            ),
        });
    }
    for (idx, &target) in targets.iter().enumerate() {
        if target >= node_count {
            return Err(ToposortCsrError::BadCsr {
                message: format!(
                    "Fix: toposort_csr targets[{idx}]={target} is outside node_count={node_count}."
                ),
            });
        }
    }
    Ok(ToposortCsrLayout {
        node_count,
        node_words: node_count as usize,
        offset_words: expected_offsets,
        target_words: targets.len(),
    })
}

/// Validate primitive-native CSR inputs and return the full dispatch plan.

pub fn plan_toposort_csr_dispatch(
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
) -> Result<ToposortCsrDispatchPlan, ToposortCsrError> {
    let layout = validate_toposort_csr_inputs(node_count, offsets, targets)?;
    Ok(ToposortCsrDispatchPlan {
        offset_words: layout.offset_words,
        target_words: layout.target_words,
        node_words: layout.node_words,
        layout,
        grid: TOPOSORT_DISPATCH_GRID,
    })
}

#[cfg(test)]
mod dispatch_plan_tests {
    use super::*;

    #[test]
    fn dispatch_plan_owns_scratch_sizes_and_grid() {
        let plan = plan_toposort_csr_dispatch(3, &[0, 2, 3, 3], &[1, 2, 2])
            .expect("Fix: valid DAG CSR should plan topological-sort dispatch");

        assert_eq!(plan.grid, TOPOSORT_DISPATCH_GRID);
        assert_eq!(plan.offset_words, 4);
        assert_eq!(plan.target_words, 3);
        assert_eq!(plan.node_words, 3);
        assert_eq!(plan.layout.node_count, 3);
    }

    #[test]
    fn empty_dispatch_plan_is_non_dispatchable_but_well_shaped() {
        let plan = plan_toposort_csr_dispatch(0, &[0], &[])
            .expect("Fix: canonical empty CSR should plan without dispatch");

        assert_eq!(plan.grid, TOPOSORT_DISPATCH_GRID);
        assert_eq!(plan.offset_words, 1);
        assert_eq!(plan.target_words, 0);
        assert_eq!(plan.node_words, 0);
        assert_eq!(plan.layout.node_count, 0);
    }

    #[test]
    fn csr_into_emits_order_accepted_by_public_validator() {
        let offsets = [0, 2, 3, 3];
        let targets = [1, 2, 2];
        let mut order = Vec::with_capacity(3);

        toposort_csr_into(3, &offsets, &targets, &mut order)
            .expect("Fix: valid DAG CSR should topologically sort.");

        validate_toposort_csr_order(3, &offsets, &targets, &order)
            .expect("Fix: toposort_csr_into output must satisfy the public order validator.");
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn csr_order_validator_rejects_dependency_inversion() {
        let err = validate_toposort_csr_order(3, &[0, 2, 3, 3], &[1, 2, 2], &[2, 1, 0])
            .expect_err("Fix: dependency-inverted CSR order must be rejected.");

        assert!(matches!(err, ToposortCsrError::BadOrder { .. }));
    }

    #[test]
    fn static_input_key_tracks_content_not_only_shape() {
        let plan = plan_toposort_csr_dispatch(4, &[0, 2, 3, 3, 3], &[1, 2, 3])
            .expect("Fix: valid CSR should plan topological-sort dispatch");
        let first = plan
            .static_input_key(&[0, 2, 3, 3, 3], &[1, 2, 3])
            .expect("Fix: static key should accept matching slices");
        let same = plan
            .static_input_key(&[0, 2, 3, 3, 3], &[1, 2, 3])
            .expect("Fix: identical CSR should produce identical key");
        let changed_targets = plan
            .static_input_key(&[0, 2, 3, 3, 3], &[2, 3, 3])
            .expect("Fix: same-shape CSR content change should still key");

        assert_eq!(first, same);
        assert_eq!(first.node_count, 4);
        assert_eq!(first.node_words, 4);
        assert_eq!(first.offset_words, 5);
        assert_eq!(first.target_words, 3);
        assert_ne!(first, changed_targets);
        assert_eq!(first.offsets_hash, changed_targets.offsets_hash);
        assert_ne!(first.targets_hash, changed_targets.targets_hash);
    }

    #[test]
    fn static_input_key_rejects_plan_slice_drift() {
        let plan = plan_toposort_csr_dispatch(3, &[0, 1, 2, 2], &[1, 2])
            .expect("Fix: valid CSR should plan topological-sort dispatch");

        let err = plan
            .static_input_key(&[0, 1, 2, 2], &[1])
            .expect_err("Fix: stale plan must not accept mismatched target slices");

        assert!(matches!(err, ToposortCsrError::BadCsr { .. }));
    }
}

/// Validate that `order` is a full topological permutation for the
/// primitive-native CSR adjacency shape.
///
/// # Errors
///
/// Returns [`ToposortCsrError::BadCsr`] for malformed CSR input and
/// [`ToposortCsrError::BadOrder`] for malformed, partial, duplicate, or
/// dependency-violating orders.
pub fn validate_toposort_csr_order(
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
    order: &[u32],
) -> Result<(), ToposortCsrError> {
    let layout = validate_toposort_csr_inputs(node_count, offsets, targets)?;
    validate_toposort_csr_order_with_layout(&layout, offsets, targets, order)
}

fn validate_toposort_csr_order_with_layout(
    layout: &ToposortCsrLayout,
    offsets: &[u32],
    targets: &[u32],
    order: &[u32],
) -> Result<(), ToposortCsrError> {
    let node_count = layout.node_count;
    if order.len() != node_count as usize {
        return Err(ToposortCsrError::BadOrder {
            message: format!(
                "Fix: toposort_csr expected {} order entries, got {}.",
                node_count,
                order.len()
            ),
        });
    }
    let mut pos: Vec<usize> = Vec::new();
    crate::graph::scratch::reserve_graph_items_with(
        &mut pos,
        layout.node_words,
        "toposort CSR CPU oracle",
        "toposort_csr order positions",
        toposort_csr_allocation,
    )?;
    pos.resize(layout.node_words, usize::MAX);
    for (idx, &node) in order.iter().enumerate() {
        if node >= node_count {
            return Err(ToposortCsrError::BadOrder {
                message: format!(
                    "Fix: toposort_csr order[{idx}]={node} is outside node_count={node_count}."
                ),
            });
        }
        let slot = &mut pos[node as usize];
        if *slot != usize::MAX {
            return Err(ToposortCsrError::BadOrder {
                message: format!(
                    "Fix: toposort_csr order contains duplicate node {node}; graph may be cyclic or backend output is malformed."
                ),
            });
        }
        *slot = idx;
    }
    if let Some((missing, _)) = pos
        .iter()
        .enumerate()
        .find(|(_, position)| **position == usize::MAX)
    {
        return Err(ToposortCsrError::BadOrder {
            message: format!(
                "Fix: toposort_csr order omitted node {missing}; graph may be cyclic."
            ),
        });
    }

    for prereq in 0..node_count {
        let start = offsets[prereq as usize] as usize;
        let end = offsets[prereq as usize + 1] as usize;
        for &dependent in &targets[start..end] {
            if pos[prereq as usize] >= pos[dependent as usize] {
                return Err(ToposortCsrError::BadOrder {
                    message: format!(
                        "Fix: toposort_csr order violates dependency edge {prereq}->{dependent}; prerequisite position {} must be before dependent position {}.",
                        pos[prereq as usize],
                        pos[dependent as usize]
                    ),
                });
            }
        }
    }
    Ok(())
}

/// CPU reference: Kahn's algorithm over `(node_count, edges)`.
///
/// `edges` is a slice of `(from, to)` u32 pairs  -  `from` depends on
/// `to`, so `to` comes first in the sort. Returns a `Vec<u32>` in
/// topological order on success, or `ToposortError::Cycle` if the
/// graph has a cycle.
///
/// # Errors
///
/// Returns `ToposortError::Cycle` when the input has a cycle, or
/// `ToposortError::UnknownNode` when an edge names a node id
/// outside `0..node_count`.
pub fn toposort(node_count: u32, edges: &[(u32, u32)]) -> Result<Vec<u32>, ToposortError> {
    const NONE: usize = usize::MAX;

    validate_toposort_edge_ids(node_count, edges)?;

    let n = node_count as usize;
    let mut indeg: Vec<u32> = Vec::new();
    crate::graph::scratch::reserve_graph_items_with(
        &mut indeg,
        n,
        "toposort CPU oracle",
        "toposort indegree scratch",
        toposort_allocation,
    )?;
    indeg.resize(n, 0);
    let mut outgoing_head: Vec<usize> = Vec::new();
    crate::graph::scratch::reserve_graph_items_with(
        &mut outgoing_head,
        n,
        "toposort CPU oracle",
        "toposort outgoing heads",
        toposort_allocation,
    )?;
    outgoing_head.resize(n, NONE);
    let mut outgoing_to: Vec<u32> = Vec::new();
    crate::graph::scratch::reserve_graph_items_with(
        &mut outgoing_to,
        edges.len(),
        "toposort CPU oracle",
        "toposort outgoing targets",
        toposort_allocation,
    )?;
    let mut outgoing_next: Vec<usize> = Vec::new();
    crate::graph::scratch::reserve_graph_items_with(
        &mut outgoing_next,
        edges.len(),
        "toposort CPU oracle",
        "toposort outgoing links",
        toposort_allocation,
    )?;
    let mut depends_head: Vec<usize> = Vec::new();
    crate::graph::scratch::reserve_graph_items_with(
        &mut depends_head,
        n,
        "toposort CPU oracle",
        "toposort dependency heads",
        toposort_allocation,
    )?;
    depends_head.resize(n, NONE);
    let mut depends_to: Vec<u32> = Vec::new();
    crate::graph::scratch::reserve_graph_items_with(
        &mut depends_to,
        edges.len(),
        "toposort CPU oracle",
        "toposort dependency targets",
        toposort_allocation,
    )?;
    let mut depends_next: Vec<usize> = Vec::new();
    crate::graph::scratch::reserve_graph_items_with(
        &mut depends_next,
        edges.len(),
        "toposort CPU oracle",
        "toposort dependency links",
        toposort_allocation,
    )?;

    for &(from, to) in edges {
        let outgoing_idx = outgoing_to.len();
        outgoing_to.push(from);
        outgoing_next.push(outgoing_head[to as usize]);
        outgoing_head[to as usize] = outgoing_idx;

        let depends_idx = depends_to.len();
        depends_to.push(to);
        depends_next.push(depends_head[from as usize]);
        depends_head[from as usize] = depends_idx;

        indeg[from as usize] = indeg[from as usize]
            .checked_add(1)
            .ok_or(ToposortError::IndegreeOverflow { node: from })?;
    }

    let mut queue: Vec<u32> = Vec::new();
    crate::graph::scratch::reserve_graph_items_with(
        &mut queue,
        n,
        "toposort CPU oracle",
        "toposort zero-indegree queue",
        toposort_allocation,
    )?;
    for v in 0..node_count {
        if indeg[v as usize] == 0 {
            queue.push(v);
        }
    }
    let mut out: Vec<u32> = Vec::new();
    crate::graph::scratch::reserve_graph_items_with(
        &mut out,
        n,
        "toposort CPU oracle",
        "toposort output order",
        toposort_allocation,
    )?;

    while let Some(&v) = queue.last() {
        queue.pop();
        out.push(v);
        let mut edge = outgoing_head[v as usize];
        while edge != NONE {
            let u = outgoing_to[edge];
            let slot = &mut indeg[u as usize];
            *slot = slot.checked_sub(1).ok_or_else(|| {
                ToposortError::InconsistentState {
                    message: format!(
                        "toposort indegree underflow for node {u}. Fix: rebuild dependency edges before scheduling."
                    ),
                }
            })?;
            if *slot == 0 {
                queue.push(u);
            }
            edge = outgoing_next[edge];
        }
    }

    if out.len() != n {
        // AUDIT_2026-04-24 F-TS-03: returning the first node with
        // indeg > 0 is misleading  -  that node may be *downstream* of
        // a cycle (its predecessor is stuck, not itself). Instead,
        // walk outgoing "depends on" edges from any unemitted node
        // until we revisit a node already on the walk  -  that revisit
        // point is guaranteed to lie on the cycle.
        let seed = indeg
            .iter()
            .enumerate()
            .find(|(_, deg)| **deg > 0)
            .map(|(i, _)| i as u32)
            .ok_or_else(|| {
                ToposortError::InconsistentState {
                    message: format!(
                        "toposort could not find a positive-indegree seed while output_len={} node_count={n}. Fix: rebuild dependency indegrees before scheduling.",
                        out.len()
                    ),
                }
            });
        let seed = seed?;
        let mut on_stack: Vec<bool> = Vec::new();
        crate::graph::scratch::reserve_graph_items_with(
            &mut on_stack,
            n,
            "toposort CPU oracle",
            "toposort cycle diagnosis stack",
            toposort_allocation,
        )?;
        on_stack.resize(n, false);
        let mut cursor = seed;
        let cycle_node = loop {
            if on_stack[cursor as usize] {
                break cursor;
            }
            on_stack[cursor as usize] = true;
            let mut edge = depends_head[cursor as usize];
            let mut next = None;
            while edge != NONE {
                let candidate = depends_to[edge];
                if indeg[candidate as usize] > 0 {
                    next = Some(candidate);
                    break;
                }
                edge = depends_next[edge];
            }
            match next {
                Some(n) => cursor = n,
                None => {
                    return Err(ToposortError::InconsistentState {
                        message: format!(
                            "toposort cycle diagnosis found stuck node {cursor} without an unemitted dependency. Fix: rebuild the dependency adjacency; this state is inconsistent with Kahn's invariant."
                        ),
                    });
                }
            }
        };
        return Err(ToposortError::Cycle { node: cycle_node });
    }
    Ok(out)
}

fn validate_toposort_edge_ids(node_count: u32, edges: &[(u32, u32)]) -> Result<(), ToposortError> {
    for (edge_idx, &(from, to)) in edges.iter().enumerate() {
        if from >= node_count {
            return Err(ToposortError::UnknownNode {
                edge: edge_idx,
                node: from,
            });
        }
        if to >= node_count {
            return Err(ToposortError::UnknownNode {
                edge: edge_idx,
                node: to,
            });
        }
    }
    Ok(())
}

fn toposort_csr_allocation(message: String) -> ToposortCsrError {
    ToposortCsrError::BadCsr { message }
}

fn toposort_allocation(message: String) -> ToposortError {
    ToposortError::InconsistentState { message }
}

/// Build a single-invocation Program that runs Kahn's algorithm
/// serially on lane 0.
///
/// `offsets_buf` is a CSR row-pointer array with `node_count + 1`
/// entries; `targets_buf` is the CSR column array. `indeg_scratch`
/// and `queue_scratch` are caller-provided temporary buffers of
/// length `node_count`. `order_out` receives the topological order
/// (length `node_count` on an acyclic graph; fewer on a cyclic
/// graph because the kernel does not diagnose cycles).
///
/// Workgroup size is `[1, 1, 1]`. The kernel only executes on
/// invocation 0; other lanes return immediately.
#[must_use]
pub fn toposort_program(
    node_count: u32,
    offsets_buf: &str,
    targets_buf: &str,
    indeg_scratch: &str,
    queue_scratch: &str,
    order_out: &str,
) -> Program {
    let lane0 = Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0));

    let body = vec![
        // Zero out indeg_scratch.
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(node_count),
            vec![Node::store(indeg_scratch, Expr::var("i"), Expr::u32(0))],
        ),
        // Fill indegrees from edges. Edge count = offsets_buf[node_count].
        Node::let_bind("edge_count", Expr::load(offsets_buf, Expr::u32(node_count))),
        Node::loop_for(
            "e",
            Expr::u32(0),
            Expr::var("edge_count"),
            vec![
                Node::let_bind("t", Expr::load(targets_buf, Expr::var("e"))),
                Node::let_bind("old", Expr::load(indeg_scratch, Expr::var("t"))),
                Node::store(
                    indeg_scratch,
                    Expr::var("t"),
                    Expr::add(Expr::var("old"), Expr::u32(1)),
                ),
            ],
        ),
        // Seed queue with every node whose indegree is zero.
        Node::let_bind("write_head", Expr::u32(0)),
        Node::loop_for(
            "v",
            Expr::u32(0),
            Expr::u32(node_count),
            vec![Node::if_then(
                Expr::eq(Expr::load(indeg_scratch, Expr::var("v")), Expr::u32(0)),
                vec![
                    Node::store(queue_scratch, Expr::var("write_head"), Expr::var("v")),
                    Node::assign(
                        "write_head",
                        Expr::add(Expr::var("write_head"), Expr::u32(1)),
                    ),
                ],
            )],
        ),
        // Pop / decrement / push until the queue is empty.
        Node::let_bind("read_head", Expr::u32(0)),
        Node::let_bind("out_idx", Expr::u32(0)),
        Node::loop_for(
            "step",
            Expr::u32(0),
            Expr::u32(node_count),
            vec![Node::if_then(
                Expr::lt(Expr::var("read_head"), Expr::var("write_head")),
                vec![
                    Node::let_bind("v", Expr::load(queue_scratch, Expr::var("read_head"))),
                    Node::assign("read_head", Expr::add(Expr::var("read_head"), Expr::u32(1))),
                    Node::store(order_out, Expr::var("out_idx"), Expr::var("v")),
                    Node::assign("out_idx", Expr::add(Expr::var("out_idx"), Expr::u32(1))),
                    Node::let_bind("edge_start", Expr::load(offsets_buf, Expr::var("v"))),
                    Node::let_bind(
                        "edge_end",
                        Expr::load(offsets_buf, Expr::add(Expr::var("v"), Expr::u32(1))),
                    ),
                    Node::loop_for(
                        "e",
                        Expr::var("edge_start"),
                        Expr::var("edge_end"),
                        vec![
                            Node::let_bind("u", Expr::load(targets_buf, Expr::var("e"))),
                            Node::let_bind(
                                "new_deg",
                                Expr::sub(Expr::load(indeg_scratch, Expr::var("u")), Expr::u32(1)),
                            ),
                            Node::store(indeg_scratch, Expr::var("u"), Expr::var("new_deg")),
                            Node::if_then(
                                Expr::eq(Expr::var("new_deg"), Expr::u32(0)),
                                vec![
                                    Node::store(
                                        queue_scratch,
                                        Expr::var("write_head"),
                                        Expr::var("u"),
                                    ),
                                    Node::assign(
                                        "write_head",
                                        Expr::add(Expr::var("write_head"), Expr::u32(1)),
                                    ),
                                ],
                            ),
                        ],
                    ),
                ],
            )],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(offsets_buf, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(node_count.saturating_add(1)),
            BufferDecl::storage(targets_buf, 1, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(indeg_scratch, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(node_count.max(1)),
            BufferDecl::storage(queue_scratch, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(node_count.max(1)),
            BufferDecl::storage(order_out, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(node_count.max(1)),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(lane0, body)]),
        }],
    )
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn empty_graph_sorts_to_empty() {
        assert_eq!(toposort(0, &[]), Ok(Vec::new()));
    }

    #[test]
    fn no_edges_returns_all_nodes() {
        let got = toposort(3, &[])
            .expect("Fix: no-cycle case; restore this invariant before continuing.");
        assert_eq!(got.len(), 3);
        let mut sorted = got.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, vec![0, 1, 2]);
    }

    #[test]
    fn linear_chain_respects_order() {
        // 0 depends on 1 depends on 2 → sort places 2 before 1 before 0.
        let got = toposort(3, &[(0, 1), (1, 2)])
            .expect("Fix: linear chain is acyclic; restore this invariant before continuing.");
        let pos = |v: u32| got.iter().position(|&x| x == v).unwrap();
        assert!(pos(2) < pos(1));
        assert!(pos(1) < pos(0));
    }

    #[test]
    fn cycle_is_rejected() {
        let err = toposort(2, &[(0, 1), (1, 0)]).expect_err("2-cycle must be detected");
        assert!(matches!(err, ToposortError::Cycle { .. }));
    }

    #[test]
    fn cycle_diagnostic_names_node_on_cycle_not_downstream() {
        // AUDIT_2026-04-24 F-TS-03: graph where node 0 depends on
        // the cycle {1 → 2 → 3 → 1} but is not on it. Prior code
        // reported the first `indeg > 0` node (node 0  -  downstream of
        // the cycle), which was misleading because 0 itself is not on
        // any cycle. Diagnostic must name a node actually on a cycle.
        let err = toposort(4, &[(0, 1), (1, 2), (2, 3), (3, 1)])
            .expect_err("3-cycle with downstream consumer must be detected");
        match err {
            ToposortError::Cycle { node } => {
                assert!(
                    matches!(node, 1..=3),
                    "cycle node {node} must be on the cycle {{1,2,3}}, not the downstream node 0"
                );
            }
            other => panic!("expected Cycle error, got {other:?}"),
        }
    }

    #[test]
    fn unknown_node_surfaces_edge_index() {
        let err = toposort(2, &[(0, 5)]).expect_err("node 5 is out of range");
        match err {
            ToposortError::UnknownNode { edge, node } => {
                assert_eq!(edge, 0);
                assert_eq!(node, 5);
            }
            _ => panic!("expected UnknownNode"),
        }
    }

    #[test]
    fn unknown_node_validation_runs_before_node_sized_allocations() {
        let source = include_str!("toposort.rs");
        let function_source = source
            .split("pub fn toposort(")
            .nth(1)
            .expect("Fix: primitive topological sort source should contain toposort.");
        let validation_pos = function_source
            .find("validate_toposort_edge_ids(node_count, edges)?")
            .expect("Fix: toposort should prevalidate edge ids.");
        let first_node_scratch_pos = function_source
            .find("vec![")
            .expect("Fix: toposort source should contain node-sized scratch allocation.");
        assert!(
            validation_pos < first_node_scratch_pos,
            "Fix: reject malformed edges before allocating node-sized topological-sort scratch."
        );

        let err = validate_toposort_edge_ids(3, &[(0, 1), (2, 3)])
            .expect_err("edge target equal to node_count must be rejected");
        assert_eq!(err, ToposortError::UnknownNode { edge: 1, node: 3 });
    }

    #[test]
    fn diamond_graph_sorts() {
        // 0 depends on 1 and 2; both depend on 3.
        let got = toposort(4, &[(0, 1), (0, 2), (1, 3), (2, 3)])
            .expect("Fix: diamond is acyclic; restore this invariant before continuing.");
        let pos = |v: u32| got.iter().position(|&x| x == v).unwrap();
        assert!(pos(3) < pos(1));
        assert!(pos(3) < pos(2));
        assert!(pos(1) < pos(0));
        assert!(pos(2) < pos(0));
    }

    #[test]
    fn emitted_program_has_expected_buffers_and_workgroup_size() {
        let p = toposort_program(4, "offsets", "targets", "indeg", "queue", "order");
        assert_eq!(p.workgroup_size, [1, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["offsets", "targets", "indeg", "queue", "order"]);
        assert_eq!(p.buffers[0].count(), 5); // node_count + 1
        assert_eq!(p.buffers[2].count(), 4); // node_count
        assert_eq!(p.buffers[3].count(), 4); // node_count
        assert_eq!(p.buffers[4].count(), 4); // node_count
    }

    #[test]
    fn csr_reference_sorts_prerequisites_before_dependents() {
        let order = toposort_csr(3, &[0, 2, 3, 3], &[1, 2, 2]).unwrap();
        let pos = |v: u32| order.iter().position(|&x| x == v).unwrap();
        assert!(pos(0) < pos(1));
        assert!(pos(0) < pos(2));
        assert!(pos(1) < pos(2));
    }

    #[test]
    fn csr_reference_reuses_output_storage() {
        let mut order = Vec::with_capacity(8);
        toposort_csr_into(3, &[0, 1, 2, 2], &[1, 2], &mut order).unwrap();
        let capacity = order.capacity();
        assert_eq!(order.len(), 3);

        toposort_csr_into(2, &[0, 1, 1], &[1], &mut order).unwrap();
        assert_eq!(order.capacity(), capacity);
        assert_eq!(order.len(), 2);
    }

    #[test]
    fn csr_reference_with_scratch_reuses_storage_and_clears_stale_state() {
        let mut order = Vec::with_capacity(8);
        order.extend_from_slice(&[99, 98, 97]);
        let mut queue = Vec::with_capacity(8);
        queue.extend_from_slice(&[6, 5, 4]);
        let mut scratch = ToposortCsrScratch {
            indeg: vec![7; 8],
            queue,
        };
        let order_capacity = order.capacity();
        let indeg_capacity = scratch.indeg.capacity();
        let queue_capacity = scratch.queue.capacity();

        toposort_csr_into_with_scratch(4, &[0, 2, 3, 3, 3], &[1, 2, 3], &mut order, &mut scratch)
            .expect("Fix: valid DAG must sort while reusing caller-owned scratch.");

        validate_toposort_csr_order(4, &[0, 2, 3, 3, 3], &[1, 2, 3], &order)
            .expect("Fix: scratch-backed topological order must satisfy the CSR contract.");
        assert_eq!(order.capacity(), order_capacity);
        assert_eq!(scratch.indeg.capacity(), indeg_capacity);
        assert_eq!(scratch.queue.capacity(), queue_capacity);
        assert_eq!(
            scratch.indeg,
            vec![0, 0, 0, 0],
            "Fix: scratch-backed traversal must not retain stale indegree counts."
        );
        assert!(
            scratch.queue.is_empty(),
            "Fix: scratch-backed traversal must consume stale and live queue entries."
        );

        toposort_csr_into_with_scratch(2, &[0, 1, 1], &[1], &mut order, &mut scratch)
            .expect("Fix: second smaller DAG must reuse the same workspace.");
        validate_toposort_csr_order(2, &[0, 1, 1], &[1], &order)
            .expect("Fix: reused workspace must not leak prior graph state.");
        assert_eq!(order.capacity(), order_capacity);
        assert_eq!(scratch.indeg.capacity(), indeg_capacity);
        assert_eq!(scratch.queue.capacity(), queue_capacity);
        assert_eq!(scratch.indeg, vec![0, 0]);
        assert!(scratch.queue.is_empty());
    }

    #[test]
    fn csr_reference_with_scratch_validates_before_mutating_reused_storage() {
        let mut order = vec![9, 8, 7];
        let mut scratch = ToposortCsrScratch {
            indeg: vec![1, 2],
            queue: vec![3],
        };
        let err = toposort_csr_into_with_scratch(2, &[0, 2, 1], &[1], &mut order, &mut scratch)
            .expect_err("Fix: malformed CSR offsets must be rejected.");

        assert!(matches!(err, ToposortCsrError::BadCsr { .. }));
        assert_eq!(
            order,
            vec![9, 8, 7],
            "Fix: validation failures must not clobber reusable output storage."
        );
        assert_eq!(
            scratch.indeg,
            vec![1, 2],
            "Fix: validation failures must not clear reusable indegree scratch."
        );
        assert_eq!(
            scratch.queue,
            vec![3],
            "Fix: validation failures must not clear reusable queue scratch."
        );
    }

    #[test]
    fn generated_csr_reference_with_scratch_matches_allocating_reference() {
        let mut order = Vec::new();
        let mut scratch = ToposortCsrScratch::new();

        for case in 0..2048usize {
            let n = case % 17;
            let mut offsets = Vec::with_capacity(n + 1);
            let mut targets = Vec::new();
            offsets.push(0);
            for src in 0..n {
                for dst in src + 1..n {
                    let mixed = case
                        .wrapping_mul(31)
                        .wrapping_add(src.wrapping_mul(17))
                        .wrapping_add(dst.wrapping_mul(13));
                    if mixed % 5 == 0 || (case % 11 == 0 && dst == src + 1) {
                        targets.push(dst as u32);
                    }
                }
                offsets.push(targets.len() as u32);
            }

            let expected = toposort_csr(n as u32, &offsets, &targets)
                .expect("Fix: generated lower-triangular CSR graph must be a valid DAG.");
            toposort_csr_into_with_scratch(n as u32, &offsets, &targets, &mut order, &mut scratch)
                .expect("Fix: scratch-backed oracle must accept every generated valid DAG.");
            assert_eq!(
                order, expected,
                "Fix: scratch-backed oracle diverged from allocating oracle at generated case {case}."
            );
        }
    }

    #[test]
    fn csr_validation_rejects_bad_shape() {
        let err = validate_toposort_csr_inputs(2, &[0, 2, 1], &[1]).unwrap_err();
        assert!(matches!(err, ToposortCsrError::BadCsr { .. }));
    }

    #[test]
    fn csr_validation_returns_dispatch_layout() {
        assert_eq!(
            validate_toposort_csr_inputs(3, &[0, 2, 3, 3], &[1, 2, 2]).unwrap(),
            ToposortCsrLayout {
                node_count: 3,
                node_words: 3,
                offset_words: 4,
                target_words: 3,
            }
        );
        assert_eq!(
            validate_toposort_csr_inputs(0, &[0], &[]).unwrap(),
            ToposortCsrLayout {
                node_count: 0,
                node_words: 0,
                offset_words: 1,
                target_words: 0,
            }
        );
    }

    #[test]
    fn csr_order_validation_rejects_duplicate_backend_output() {
        let err = validate_toposort_csr_order(3, &[0, 1, 2, 2], &[1, 2], &[0, 1, 1]).unwrap_err();
        assert!(matches!(err, ToposortCsrError::BadOrder { .. }));
    }

    #[test]
    fn csr_order_validation_rejects_dependency_inversion() {
        let err = validate_toposort_csr_order(2, &[0, 1, 1], &[1], &[1, 0]).unwrap_err();
        assert!(matches!(err, ToposortCsrError::BadOrder { .. }));
    }

    // ------------------------------------------------------------------
    // Adversarial fixtures  -  empty/single/disconnected/self-loop/max-size.
    // ------------------------------------------------------------------

    #[test]
    fn single_node_no_edges() {
        assert_eq!(toposort(1, &[]), Ok(vec![0]));
    }

    #[test]
    fn self_loops_only_rejected() {
        // Every node has a self-loop  -  each is a 1-cycle.
        let err = toposort(3, &[(0, 0), (1, 1), (2, 2)]).expect_err("self-loops are cycles");
        assert!(matches!(err, ToposortError::Cycle { .. }));
    }

    #[test]
    fn disconnected_components_sorts_all() {
        // Component A: 0 depends on 1. Component B: 2 depends on 3.
        let got = toposort(4, &[(0, 1), (2, 3)]).unwrap();
        assert_eq!(got.len(), 4);
        let pos = |v: u32| got.iter().position(|&x| x == v).unwrap();
        assert!(pos(1) < pos(0), "1 must come before 0");
        assert!(pos(3) < pos(2), "3 must come before 2");
    }

    #[test]
    fn max_node_count_min_edges() {
        // 1000 nodes, one chain edge 0→1.
        let got = toposort(1000, &[(0, 1)]).unwrap();
        assert_eq!(got.len(), 1000);
        let pos = |v: u32| got.iter().position(|&x| x == v).unwrap();
        assert!(pos(1) < pos(0), "1 must come before 0");
    }

    #[test]
    fn cycle_on_large_graph_diagnostic_is_on_cycle() {
        // 100 nodes in a line, back-edge creating cycle 50→51→…→99→50.
        let mut edges: Vec<(u32, u32)> = (0..99).map(|i| (i, i + 1)).collect();
        edges.push((99, 50));
        let err = toposort(100, &edges).expect_err("cycle must be detected");
        match err {
            ToposortError::Cycle { node } => {
                assert!(
                    (50..=99).contains(&node),
                    "cycle node {node} must be on the back-edge cycle"
                );
            }
            other => panic!("expected Cycle, got {other:?}"),
        }
    }

    #[test]
    fn toposort_result_path_has_no_internal_panics() {
        let source = include_str!("toposort.rs");
        let result_path = source
            .split("pub fn toposort(")
            .nth(1)
            .expect("Fix: toposort implementation source must be present")
            .split("/// Build a single-invocation Program")
            .next()
            .expect("Fix: toposort implementation source must precede program builder");

        assert!(
            result_path.contains("ToposortError::IndegreeOverflow")
                && result_path.contains("ToposortError::InconsistentState")
                && !result_path.contains(concat!("panic", "!("))
                && !result_path.contains(".unwrap_or_else("),
            "Fix: toposort already returns Result, so internal failure states must be Err variants instead of panics."
        );
    }
}
