//! Dominator tree  -  the control-flow primitive for borrow checking and
//! optimization passes.
//!
//! Every compiler needs dominator information: SSA construction, borrow-check
//! region inference, loop detection, and code motion all depend on it.  Vyre
//! provides `dominator_tree` as a first-class workgroup-local primitive.  The
//! CPU reference implements the classic Cooper-Harvey-Kennedy iterative
//! algorithm over a CSR CFG; the target-text kernel performs the exact same
//! reverse-postorder walk and intersection in workgroup SRAM.  This is the
//! sequential-coordination abstraction that lets a model emit borrow-check
//! logic without ever writing a shader.

use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use thiserror::Error;
use vyre_spec::AlgebraicLaw;

/// Registered device source for the dominator-tree primitive.
#[must_use]
pub fn source() -> Option<&'static str> {
    crate::transform::compiler::shader_provider::source("dominator_tree")
}

/// Sentinel used by `idom[...] == IDOM_UNDEFINED` to mark "no
/// dominator yet assigned" during the iterative Cooper-Harvey-
/// Kennedy walk.
pub const IDOM_UNDEFINED: u32 = u32::MAX;

/// Build a vyre IR Program that runs ONE Cooper-Harvey-Kennedy
/// step of iterative dominator-tree computation over a CSR CFG.
///
/// Callers drive the outer loop until `changed_flag[0]` stays 0.
/// Per step, each node (one lane per node) observes its first
/// already-defined predecessor, intersects all other defined
/// predecessors' idoms into it, and writes the resulting idom
/// if it changed.
///
/// Buffers:
/// - `idom`: `ReadWrite` u32 array  -  `node_count` entries; entry points
///   start as themselves, all others as [`IDOM_UNDEFINED`].
/// - `pred_offsets`: `ReadOnly` u32 array of length `node_count` + 1  -
///   CSR offsets into the predecessor table.
/// - `preds`: `ReadOnly` u32 array  -  flat predecessor list.
/// - `rpo_index`: `ReadOnly` u32 array  -  reverse-postorder index per
///   node; used by the intersect helper to pick the "lower" node
///   when climbing.
/// - `changed_flag`: `ReadWrite` u32 array of length 1.
///
/// The intersect helper is embedded inline  -  it walks the two idom
/// chains up the tree until they meet, comparing reverse-postorder
/// indices to pick the deeper of the two.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "IR construction mirrors the dominator kernel control flow; splitting obscures the graph invariant"
)]
pub fn relax_step_program(
    idom: &str,
    pred_offsets: &str,
    preds: &str,
    rpo_index: &str,
    changed_flag: &str,
) -> Program {
    let tid = Expr::InvocationId { axis: 0 };
    let body = vec![
        Node::let_bind("node", tid.clone()),
        Node::let_bind("start", Expr::load(pred_offsets, Expr::var("node"))),
        Node::let_bind(
            "end",
            Expr::load(pred_offsets, Expr::add(Expr::var("node"), Expr::u32(1))),
        ),
        // Seed new_idom with the first defined predecessor.
        Node::let_bind("new_idom", Expr::u32(IDOM_UNDEFINED)),
        Node::loop_for(
            "i",
            Expr::var("start"),
            Expr::var("end"),
            vec![
                Node::let_bind("p", Expr::load(preds, Expr::var("i"))),
                Node::let_bind("pi", Expr::load(idom, Expr::var("p"))),
                Node::if_then(
                    Expr::ne(Expr::var("pi"), Expr::u32(IDOM_UNDEFINED)),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("new_idom"), Expr::u32(IDOM_UNDEFINED)),
                        vec![Node::assign("new_idom", Expr::var("p"))],
                    )],
                ),
            ],
        ),
        // Intersect new_idom against every other defined
        // predecessor. We use rpo_index to pick the deeper node:
        // whichever has a higher rpo_index is "lower" in the tree.
        Node::loop_for(
            "j",
            Expr::var("start"),
            Expr::var("end"),
            vec![
                Node::let_bind("p2", Expr::load(preds, Expr::var("j"))),
                Node::let_bind("pi2", Expr::load(idom, Expr::var("p2"))),
                Node::if_then(
                    Expr::ne(Expr::var("pi2"), Expr::u32(IDOM_UNDEFINED)),
                    vec![Node::if_then(
                        Expr::ne(Expr::var("p2"), Expr::var("new_idom")),
                        vec![
                            Node::let_bind("finger1", Expr::var("p2")),
                            Node::let_bind("finger2", Expr::var("new_idom")),
                            Node::let_bind("converged", Expr::u32(0)),
                            Node::loop_for(
                                "k",
                                Expr::u32(0),
                                Expr::u32(1024),
                                vec![Node::if_then(
                                    Expr::eq(Expr::var("converged"), Expr::u32(0)),
                                    vec![
                                        Node::if_then(
                                            Expr::eq(Expr::var("finger1"), Expr::var("finger2")),
                                            vec![Node::assign("converged", Expr::u32(1))],
                                        ),
                                        Node::let_bind(
                                            "rpo1",
                                            Expr::load(rpo_index, Expr::var("finger1")),
                                        ),
                                        Node::let_bind(
                                            "rpo2",
                                            Expr::load(rpo_index, Expr::var("finger2")),
                                        ),
                                        Node::if_then(
                                            Expr::lt(Expr::var("rpo1"), Expr::var("rpo2")),
                                            vec![Node::assign(
                                                "finger1",
                                                Expr::load(idom, Expr::var("finger1")),
                                            )],
                                        ),
                                        Node::if_then(
                                            Expr::lt(Expr::var("rpo2"), Expr::var("rpo1")),
                                            vec![Node::assign(
                                                "finger2",
                                                Expr::load(idom, Expr::var("finger2")),
                                            )],
                                        ),
                                    ],
                                )],
                            ),
                            Node::assign("new_idom", Expr::var("finger1")),
                        ],
                    )],
                ),
            ],
        ),
        // Write the result if it changed.
        Node::let_bind("old_idom", Expr::load(idom, Expr::var("node"))),
        Node::if_then(
            Expr::ne(Expr::var("new_idom"), Expr::var("old_idom")),
            vec![
                Node::store(idom, Expr::var("node"), Expr::var("new_idom")),
                Node::let_bind(
                    "chg",
                    Expr::atomic_exchange(changed_flag, Expr::u32(0), Expr::u32(1)),
                ),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(idom, 0, BufferAccess::ReadWrite, DataType::U32),
            BufferDecl::storage(pred_offsets, 1, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(preds, 2, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(rpo_index, 3, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(changed_flag, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [64, 1, 1],
        body,
    )
}

/// Dominator-tree validation errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum DominatorTreeError {
    /// Offset table has no terminal offset.
    #[error(
        "DominatorEmptyOffsets: successor_offsets must include node_count + 1 entries. Fix: emit a valid CSR offset table."
    )]
    EmptyOffsets,
    /// Entry node is outside the CFG.
    #[error(
        "DominatorInvalidEntry: entry {entry} outside node_count {node_count}. Fix: pass a valid CFG entry node."
    )]
    InvalidEntry {
        /// Invalid entry.
        entry: u32,
        /// CFG node count.
        node_count: usize,
    },
    /// CSR offset is invalid.
    #[error(
        "DominatorInvalidOffset: CSR offsets must be monotone and within successors. Fix: rebuild successor_offsets."
    )]
    InvalidOffset,
    /// Node id cannot fit in host index space.
    #[error(
        "DominatorNodeIndexOverflow: node id cannot fit usize. Fix: split the CFG before dispatch."
    )]
    NodeIndexOverflow,
    /// Successor node is outside the CFG.
    #[error(
        "DominatorInvalidSuccessor: successor {successor} outside node_count {node_count}. Fix: validate CFG edge endpoints."
    )]
    InvalidSuccessor {
        /// Invalid successor id.
        successor: u32,
        /// CFG node count.
        node_count: usize,
    },
}

/// Category C dominator-tree intrinsic.
#[derive(Debug, Default, Clone, Copy)]
pub struct DominatorTreeOp;

/// Compute immediate dominators using the Cooper-Harvey-Kennedy iteration.
///
/// Unreachable nodes receive `u32::MAX`; the entry immediately dominates
/// itself.
///
/// # Errors
///
/// Returns `Fix: ...` when CSR buffers are malformed or the entry is invalid.
#[must_use]
pub fn immediate_dominators(
    entry: u32,
    successor_offsets: &[u32],
    successors: &[u32],
) -> Result<Vec<u32>, DominatorTreeError> {
    let node_count = successor_offsets
        .len()
        .checked_sub(1)
        .ok_or(DominatorTreeError::EmptyOffsets)?;
    let entry_index = usize::try_from(entry).map_err(|_| DominatorTreeError::NodeIndexOverflow)?;
    if entry_index >= node_count {
        return Err(DominatorTreeError::InvalidEntry { entry, node_count });
    }
    validate_graph(node_count, successor_offsets, successors)?;
    let rpo = reverse_postorder(entry_index, successor_offsets, successors)?;
    let mut order = vec![UNREACHABLE; node_count];
    for (rank, &node) in rpo.iter().enumerate() {
        order[node] = u32::try_from(rank).map_err(|_| DominatorTreeError::NodeIndexOverflow)?;
    }
    let preds = predecessors(node_count, successor_offsets, successors)?;
    let mut idom = vec![UNREACHABLE; node_count];
    idom[entry_index] = entry;
    let mut changed = true;
    while changed {
        changed = false;
        for &node in rpo.iter().skip(1) {
            let mut new_idom = UNREACHABLE;
            for &pred in &preds[node] {
                let pred_index =
                    usize::try_from(pred).map_err(|_| DominatorTreeError::NodeIndexOverflow)?;
                if idom[pred_index] != UNREACHABLE {
                    new_idom = if new_idom == UNREACHABLE {
                        pred
                    } else {
                        intersect(pred, new_idom, &idom, &order)?
                    };
                }
            }
            if idom[node] != new_idom {
                idom[node] = new_idom;
                changed = true;
            }
        }
    }
    Ok(idom)
}

impl DominatorTreeOp {}

/// Safely cast a `u32` node id to `usize` for host indexing.
///
/// # Errors
///
/// Returns `DominatorTreeError::NodeIndexOverflow` if the value does not fit.
#[must_use]
pub fn index(value: u32) -> Result<usize, DominatorTreeError> {
    usize::try_from(value).map_err(|_| DominatorTreeError::NodeIndexOverflow)
}

/// Intersect two node ids up the immediate-dominator tree.
///
/// Walks the higher-ranked node up the `idom` chain until both pointers meet.
/// This is the standard CHK intersect routine used during fixed-point
/// iteration.
///
/// On a valid dominator tree of N nodes the algorithm converges in at most N
/// steps because each inner-loop step strictly decreases the RPO rank of the
/// advancing pointer. `intersect` enforces this invariant explicitly: when
/// either pointer fails to advance (e.g. due to a self-referential `idom`
/// entry from a malformed caller) it returns
/// [`DominatorTreeError::NodeIndexOverflow`] rather than looping forever.
///
/// # Errors
///
/// Returns [`DominatorTreeError::NodeIndexOverflow`] when a node id in the
/// dominator chain cannot be used as a host index, or when the chain fails
/// to converge within `idom.len() * 2 + 2` steps, which indicates a
/// malformed (cyclic) idom table.
#[must_use]
pub fn intersect(
    mut left: u32,
    mut right: u32,
    idom: &[u32],
    order: &[u32],
) -> Result<u32, DominatorTreeError> {
    // Budget: a valid CHK walk visits every node at most once per side.
    let mut budget = idom.len().saturating_mul(2).saturating_add(2);
    while left != right {
        while order[index(left)?] > order[index(right)?] {
            let next = idom[index(left)?];
            if budget == 0 || next == left {
                return Err(DominatorTreeError::NodeIndexOverflow);
            }
            budget -= 1;
            left = next;
        }
        while order[index(right)?] > order[index(left)?] {
            let next = idom[index(right)?];
            if budget == 0 || next == right {
                return Err(DominatorTreeError::NodeIndexOverflow);
            }
            budget -= 1;
            right = next;
        }
    }
    Ok(left)
}

/// Algebraic laws declared by the dominator-tree primitive.
pub const LAWS: &[AlgebraicLaw] = &[AlgebraicLaw::Bounded {
    lo: 0,
    hi: u32::MAX,
}];

/// Build a predecessor list for every CFG node from CSR successor data.
///
/// # Errors
///
/// Returns [`DominatorTreeError::NodeIndexOverflow`] when a CSR offset,
/// successor id, or emitted predecessor id cannot fit the required host/index
/// representation.
#[must_use]
pub fn predecessors(
    node_count: usize,
    offsets: &[u32],
    successors: &[u32],
) -> Result<Vec<Vec<u32>>, DominatorTreeError> {
    let mut preds = vec![Vec::new(); node_count];
    for node in 0..node_count {
        for &succ in &successors[index(offsets[node])?..index(offsets[node + 1])?] {
            preds[index(succ)?]
                .push(u32::try_from(node).map_err(|_| DominatorTreeError::NodeIndexOverflow)?);
        }
    }
    Ok(preds)
}

/// Compute a reverse-postorder sequence starting from `entry`.
///
/// The CHK algorithm converges faster when nodes are processed in RPO.
/// This routine uses an explicit stack so the traversal bound is controlled
/// and mirrored by the target-text reference.
///
/// # Errors
///
/// Returns [`DominatorTreeError::NodeIndexOverflow`] when a CSR offset or
/// successor id cannot be used as a host index.
#[must_use]
pub fn reverse_postorder(
    entry: usize,
    offsets: &[u32],
    successors: &[u32],
) -> Result<Vec<usize>, DominatorTreeError> {
    let mut seen = vec![false; offsets.len() - 1];
    let mut stack = Vec::with_capacity(offsets.len());
    stack.push((entry, false));
    let mut postorder = Vec::with_capacity(offsets.len().saturating_sub(1));
    while let Some((node, expanded)) = stack.pop() {
        if expanded {
            postorder.push(node);
            continue;
        }
        if seen[node] {
            continue;
        }
        seen[node] = true;
        stack.push((node, true));
        let start = index(offsets[node])?;
        let end = index(offsets[node + 1])?;
        for &succ in successors[start..end].iter().rev() {
            let succ_index = index(succ)?;
            if !seen[succ_index] {
                stack.push((succ_index, false));
            }
        }
    }
    postorder.reverse();
    Ok(postorder)
}

/// Sentinel value meaning "no immediate dominator" or "unreachable node".
pub const UNREACHABLE: u32 = u32::MAX;

/// Validate that CSR offsets and successors describe a well-formed CFG.
///
/// # Errors
///
/// Returns `Fix: ...` when offsets are non-monotone, out of range, or any
/// successor node id exceeds the graph bounds.
#[must_use]
pub fn validate_graph(
    node_count: usize,
    offsets: &[u32],
    successors: &[u32],
) -> Result<(), DominatorTreeError> {
    let mut previous = 0usize;
    for &offset in offsets {
        let current = index(offset)?;
        if current < previous || current > successors.len() {
            return Err(DominatorTreeError::InvalidOffset);
        }
        previous = current;
    }
    for &successor in successors {
        if index(successor)? >= node_count {
            return Err(DominatorTreeError::InvalidSuccessor {
                successor,
                node_count,
            });
        }
    }
    Ok(())
}

/// Workgroup size used by the reference target-text lowering.
pub const WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];

#[cfg(test)]
mod ir_program_tests {
    use super::*;

    #[test]
    fn relax_step_program_validates() {
        let prog = relax_step_program("idom", "po", "preds", "rpo", "cf");
        let errors = crate::validate::validate::validate(&prog);
        assert!(errors.is_empty(), "dominator IR must validate: {errors:?}");
    }

    #[test]
    fn relax_step_program_wire_round_trips() {
        let prog = relax_step_program("idom", "po", "preds", "rpo", "cf");
        let bytes = prog
            .to_wire()
            .expect("Fix: serialize; restore this invariant before continuing.");
        let decoded = Program::from_wire(&bytes)
            .expect("Fix: decode; restore this invariant before continuing.");
        assert_eq!(decoded.buffers().len(), 5);
    }

    #[test]
    fn idom_undefined_is_u32_max() {
        assert_eq!(IDOM_UNDEFINED, u32::MAX);
    }

    /// A self-loop in `idom` (e.g. `idom[1] == 1`) must not cause `intersect`
    /// to spin forever. Before the fix the inner `while` would reassign
    /// `left = idom[left_index] = left` (unchanged), looping infinitely.
    /// After the fix `intersect` returns `Err(NodeIndexOverflow)`.
    #[test]
    fn intersect_self_loop_idom_returns_error_not_infinite_loop() {
        // idom[0] = 0 (entry; self-dominates), idom[1] = 1 (malformed self-loop)
        // order[0] = 0 (lower rank), order[1] = 1 (higher rank)
        // intersect(1, 0) sees order[1]=1 > order[0]=0, tries to advance left:
        //   idom[1] = 1 — left would not change, must error.
        let idom = vec![0u32, 1u32];
        let order = vec![0u32, 1u32];
        let result = intersect(1, 0, &idom, &order);
        assert!(
            result.is_err(),
            "intersect with a self-loop idom must return Err, not loop forever; got {:?}",
            result
        );
    }

    /// `intersect` must converge normally for a simple two-node valid tree:
    ///   node 0 is the entry (idom[0]=0), node 1 is dominated by 0 (idom[1]=0).
    #[test]
    fn intersect_converges_on_valid_two_node_tree() {
        // idom = [0, 0]: both nodes dominated by node 0 (the entry).
        // order = [0, 1]: node 0 has lower RPO rank.
        // intersect(1, 0): order[1]=1 > order[0]=0 → left advances to idom[1]=0.
        // Now left==right==0 → converges to 0.
        let idom = vec![0u32, 0u32];
        let order = vec![0u32, 1u32];
        let result = intersect(1, 0, &idom, &order);
        assert_eq!(
            result,
            Ok(0),
            "intersect must converge to the common dominator (node 0)"
        );
    }
}
