//! Dataflow fixpoint  -  monotone forward dataflow analysis on GPU.
//!
//! Compiler analyses such as liveness, reaching definitions, and available
//! expressions are all iterative dataflow to a fixed point over a control-flow
//! graph.  Vyre provides that sequential coordination as a first-class
//! primitive.  The CPU reference performs a classic bitwise-OR forward
//! iteration with a bounded sweep count; the target-text kernel performs the exact
//! same lattice join in workgroup-local SRAM, stopping when the change bit
//! goes cold.  This lets a model emit `dataflow_fixpoint` as an IR node
//! instead of hand-writing warp-synchronized shader loops.

use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use thiserror::Error;
use vyre_spec::AlgebraicLaw;

/// Registered device source for the dataflow fixpoint primitive.
#[must_use]
pub fn source() -> Option<&'static str> {
    crate::transform::compiler::shader_provider::source("dataflow_fixpoint")
}

/// Build a vyre IR Program that runs ONE dataflow relaxation step
/// over a CSR successor graph. Callers run the Program in a loop
/// until `changed_flag[0]` stays zero for a full iteration (the
/// fixed-point convergence criterion is loop-driven outside the
/// single-dispatch IR).
///
/// Per step, for every node `n` in `0..node_count`, the kernel
/// computes `propagated = state[n] | transfer[n]` and folds
/// `propagated` into every successor `s` via
/// `state[s] = state[s] | propagated` (bitwise OR, monotone).
/// If any `state[s]` changes, the kernel atomic-stores `1` into
/// `changed_flag[0]`.
///
/// Buffers:
/// - `state`: `ReadWrite` u32 array  -  per-node lattice element.
/// - `transfer`: `ReadOnly` u32 array  -  per-node transfer mask.
/// - `successor_offsets`: `ReadOnly` u32 array of length
///   `node_count + 1`  -  CSR offsets.
/// - `successors`: `ReadOnly` u32 array  -  flat successor list.
/// - `changed_flag`: `ReadWrite` u32 array of length 1  -  set to 1
///   on any change during this step.
///
/// The IR dispatches one lane per node; callers set workgroup size
/// to `node_count` (or its round-up) so every node is visited.
#[must_use]
pub fn relax_step_program(
    state: &str,
    transfer: &str,
    successor_offsets: &str,
    successors: &str,
    changed_flag: &str,
) -> Program {
    let tid = Expr::InvocationId { axis: 0 };
    let body = vec![
        Node::let_bind("node", tid.clone()),
        Node::let_bind("state_n", Expr::load(state, Expr::var("node"))),
        Node::let_bind("transfer_n", Expr::load(transfer, Expr::var("node"))),
        Node::let_bind(
            "propagated",
            Expr::bitor(Expr::var("state_n"), Expr::var("transfer_n")),
        ),
        Node::let_bind("start", Expr::load(successor_offsets, Expr::var("node"))),
        Node::let_bind(
            "end",
            Expr::load(
                successor_offsets,
                Expr::add(Expr::var("node"), Expr::u32(1)),
            ),
        ),
        Node::loop_for(
            "i",
            Expr::var("start"),
            Expr::var("end"),
            vec![
                Node::let_bind("succ", Expr::load(successors, Expr::var("i"))),
                Node::let_bind("old", Expr::load(state, Expr::var("succ"))),
                Node::let_bind(
                    "new",
                    Expr::bitor(Expr::var("old"), Expr::var("propagated")),
                ),
                Node::if_then(
                    Expr::ne(Expr::var("new"), Expr::var("old")),
                    vec![
                        Node::store(state, Expr::var("succ"), Expr::var("new")),
                        Node::let_bind(
                            "chg",
                            Expr::atomic_exchange(changed_flag, Expr::u32(0), Expr::u32(1)),
                        ),
                    ],
                ),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(state, 0, BufferAccess::ReadWrite, DataType::U32),
            BufferDecl::storage(transfer, 1, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(successor_offsets, 2, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(successors, 3, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(changed_flag, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [64, 1, 1],
        body,
    )
}

/// Compute a monotone forward dataflow fixed point over a CSR successor graph.
///
/// Each node contributes `state[node] | transfer[node]` to every successor via
/// bitwise OR. This models compiler analyses whose states are bounded `u32`
/// lattices such as liveness or reaching definitions.
///
/// # Errors
///
/// Returns `Fix: ...` when graph buffers are malformed or convergence exceeds
/// `max_iterations`.
#[must_use]
pub fn compute_fixpoint(
    initial_state: &[u32],
    transfer: &[u32],
    successor_offsets: &[u32],
    successors: &[u32],
    max_iterations: u32,
) -> Result<FixpointResult, DataflowFixpointError> {
    validate_graph(initial_state.len(), transfer, successor_offsets, successors)?;
    let mut state = initial_state.to_vec();
    for iteration in 0..max_iterations {
        let mut changed = false;
        for node in 0..state.len() {
            let propagated = state[node] | transfer[node];
            let start = usize::try_from(successor_offsets[node])
                .map_err(|_| DataflowFixpointError::OffsetOverflow)?;
            let end = usize::try_from(successor_offsets[node + 1])
                .map_err(|_| DataflowFixpointError::OffsetOverflow)?;
            for &successor in &successors[start..end] {
                let successor_index = usize::try_from(successor)
                    .map_err(|_| DataflowFixpointError::NodeIndexOverflow)?;
                let joined = state[successor_index] | propagated;
                if joined != state[successor_index] {
                    state[successor_index] = joined;
                    changed = true;
                }
            }
        }
        if !changed {
            return Ok(FixpointResult {
                state,
                iterations: iteration + 1,
            });
        }
    }
    Err(DataflowFixpointError::DidNotConverge { max_iterations })
}

/// Dataflow fixpoint validation errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum DataflowFixpointError {
    /// Transfer table length does not match node count.
    #[error(
        "DataflowTransferLength: expected {expected} transfer entries, got {got}. Fix: emit one transfer mask per CFG node."
    )]
    TransferLength {
        /// Expected transfer entries.
        expected: usize,
        /// Actual transfer entries.
        got: usize,
    },
    /// CSR offset length is invalid.
    #[error(
        "DataflowOffsetLength: expected {expected} offsets, got {got}. Fix: emit node_count + 1 CSR offsets."
    )]
    OffsetLength {
        /// Expected offsets.
        expected: usize,
        /// Actual offsets.
        got: usize,
    },
    /// CSR offsets are not monotone or exceed successor length.
    #[error(
        "DataflowInvalidOffset: CSR offsets must be monotone and within successors. Fix: rebuild successor_offsets."
    )]
    InvalidOffset,
    /// CSR offset cannot fit in host index space.
    #[error(
        "DataflowOffsetOverflow: CSR offset cannot fit usize. Fix: split the graph before dispatch."
    )]
    OffsetOverflow,
    /// Node id cannot fit in host index space.
    #[error(
        "DataflowNodeIndexOverflow: node id cannot fit usize. Fix: split the graph before dispatch."
    )]
    NodeIndexOverflow,
    /// Successor node id is outside the graph.
    #[error(
        "DataflowInvalidSuccessor: successor {successor} outside node_count {node_count}. Fix: validate CFG edge endpoints."
    )]
    InvalidSuccessor {
        /// Invalid successor id.
        successor: u32,
        /// Node count.
        node_count: usize,
    },
    /// Iteration cap was reached before convergence.
    #[error(
        "DataflowDidNotConverge: no fixed point within {max_iterations} iterations. Fix: raise the bounded iteration cap or inspect non-monotone transfer data."
    )]
    DidNotConverge {
        /// Maximum number of iterations attempted.
        max_iterations: u32,
    },
}

/// Category C iterative dataflow intrinsic.
#[derive(Debug, Default, Clone, Copy)]
pub struct DataflowFixpointOp;

/// Fixed-point state and iteration count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixpointResult {
    /// Converged per-node bitset state.
    pub state: Vec<u32>,
    /// Number of full graph sweeps executed.
    pub iterations: u32,
}

impl DataflowFixpointOp {}

/// Algebraic laws declared by the dataflow-fixpoint primitive.
pub const LAWS: &[AlgebraicLaw] = &[AlgebraicLaw::Bounded {
    lo: 0,
    hi: u32::MAX,
}];

/// Validate that transfer, offsets, and successors describe a well-formed
/// dataflow graph.
///
/// # Errors
///
/// Returns `Fix: ...` when buffer lengths mismatch, offsets are invalid, or
/// any successor index overflows the node space.
#[must_use]
pub fn validate_graph(
    node_count: usize,
    transfer: &[u32],
    offsets: &[u32],
    successors: &[u32],
) -> Result<(), DataflowFixpointError> {
    if transfer.len() != node_count {
        return Err(DataflowFixpointError::TransferLength {
            expected: node_count,
            got: transfer.len(),
        });
    }
    if offsets.len() != node_count.saturating_add(1) {
        return Err(DataflowFixpointError::OffsetLength {
            expected: node_count.saturating_add(1),
            got: offsets.len(),
        });
    }
    let mut previous = 0usize;
    for &offset in offsets {
        let current = usize::try_from(offset).map_err(|_| DataflowFixpointError::OffsetOverflow)?;
        if current < previous || current > successors.len() {
            return Err(DataflowFixpointError::InvalidOffset);
        }
        previous = current;
    }
    for &successor in successors {
        let index =
            usize::try_from(successor).map_err(|_| DataflowFixpointError::NodeIndexOverflow)?;
        if index >= node_count {
            return Err(DataflowFixpointError::InvalidSuccessor {
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
        let prog = relax_step_program(
            "state",
            "transfer",
            "successor_offsets",
            "successors",
            "changed_flag",
        );
        let errors = crate::validate::validate::validate(&prog);
        assert!(errors.is_empty(), "dataflow IR must validate: {errors:?}");
    }

    #[test]
    fn relax_step_program_wire_round_trips() {
        let prog = relax_step_program("s", "t", "o", "sc", "cf");
        let bytes = prog
            .to_wire()
            .expect("Fix: serialize; restore this invariant before continuing.");
        let decoded = Program::from_wire(&bytes)
            .expect("Fix: decode; restore this invariant before continuing.");
        assert_eq!(decoded.buffers().len(), 5);
        assert_eq!(decoded.workgroup_size(), [64, 1, 1]);
    }

    #[test]
    fn relax_step_program_declares_five_buffers_in_csr_order() {
        let prog = relax_step_program(
            "state",
            "transfer",
            "successor_offsets",
            "successors",
            "changed_flag",
        );
        let names: Vec<&str> = prog.buffers().iter().map(|b| b.name()).collect();
        assert_eq!(
            names,
            vec![
                "state",
                "transfer",
                "successor_offsets",
                "successors",
                "changed_flag",
            ]
        );
    }
}
