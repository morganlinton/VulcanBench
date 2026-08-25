//! Dead-code elimination as a dispatched vyre Program.
//!
//! The encoder turns the user's `Program` into the canonical 5-buffer
//! ProgramGraph CSR; we ask an `OptimizerDispatcher` to run the optimizer
//! DCE BFS program against those buffers; the
//! returned live-frontier bitset drives a structural rewrite of the
//! input Program. There is no host-reference escape in production. If the
//! encoder cannot yet handle a Program shape it returns `EncodeError`;
//! the caller must either extend the encoder or produce a Program the
//! encoder accepts.
//!
//! The dispatcher trait inverts the dependency on a concrete backend  -
//! production callers wire `vyre-driver-wgpu` or `-cuda`; tests in this
//! crate use the in-tree `CpuOracleDispatcher` (test-only).

use vyre_foundation::ir::Program;
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::program_graph::ProgramGraphShape;

use crate::dispatch_buffers::{
    decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes, write_zero_bytes,
};

use super::dce_program::build_dce_bfs_program;
use super::dispatcher::{DispatchError, OptimizerDispatcher};
use super::encode::{apply_live_mask, encode_program, EncodeError, EncodedProgram, ROOT_GRAPH_ID};

#[derive(Debug, Default)]
struct DceKernelScratch {
    inputs: Vec<Vec<u8>>,
    seed: Vec<u32>,
    frontier: Vec<u32>,
    changed: Vec<u32>,
}

/// DCE as a dispatched analysis Program. Errors are honest:
/// - `Encode` if the input shape is not yet supported by the encoder.
/// - `Dispatch` if the dispatcher rejects the analysis Program.
#[derive(Debug)]
pub enum DceError {
    /// Encoder did not accept the input shape.
    Encode(EncodeError),
    /// Dispatcher rejected or failed to run the analysis Program.
    Dispatch(DispatchError),
}

impl std::fmt::Display for DceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(err) => write!(f, "gpu_dce encode error: {err:?}"),
            Self::Dispatch(err) => write!(f, "gpu_dce dispatch error: {err}"),
        }
    }
}

impl std::error::Error for DceError {}

/// Run DCE on `program` by encoding it into a ProgramGraph, dispatching
/// `persistent_bfs` through `dispatcher`, and rewriting the input from
/// the live-mask the dispatcher returns.
pub fn gpu_dce(
    program: Program,
    dispatcher: &dyn OptimizerDispatcher,
) -> Result<Program, DceError> {
    let encoded = encode_program(&program).map_err(DceError::Encode)?;
    let mut scratch = DceKernelScratch::default();
    let mut live = Vec::with_capacity(encoded.node_count as usize);
    compute_live_mask_with_scratch_into(&encoded, dispatcher, &mut scratch, &mut live)
        .map_err(DceError::Dispatch)?;
    Ok(apply_live_mask(&program, &encoded, &live))
}

fn compute_live_mask_with_scratch_into(
    encoded: &EncodedProgram,
    dispatcher: &dyn OptimizerDispatcher,
    scratch: &mut DceKernelScratch,
    live: &mut Vec<bool>,
) -> Result<(), DispatchError> {
    let n = encoded.node_count;
    if n == 0 {
        live.clear();
        return Ok(());
    }

    // Build the DCE analysis Program for this exact graph shape. Buffer
    // names + binding indices match the persistent BFS layout, but the DCE
    // variant exposes only the final changed flag instead of large-graph
    // active scratch.
    let shape = ProgramGraphShape::new(encoded.node_count, encoded.edge_count);
    let analysis = build_dce_bfs_program(shape, n.max(1));

    let words = bitset_words(n) as usize;
    scratch.seed.clear();
    scratch.seed.resize(words.max(1), 0);
    let root = ROOT_GRAPH_ID as usize;
    scratch.seed[root / 32] |= 1u32 << (root % 32);

    ensure_input_slots(&mut scratch.inputs, 8);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], &encoded.nodes);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], &encoded.edge_offsets);
    write_padded_one_u32_bytes(&mut scratch.inputs[2], &encoded.edge_targets);
    write_padded_one_u32_bytes(&mut scratch.inputs[3], &encoded.edge_kind_mask);
    write_u32_slice_le_bytes(&mut scratch.inputs[4], &encoded.node_tags);
    write_u32_slice_le_bytes(&mut scratch.inputs[5], &scratch.seed);
    write_zero_bytes(
        &mut scratch.inputs[6],
        words.max(1) * std::mem::size_of::<u32>(),
    );
    write_zero_bytes(&mut scratch.inputs[7], std::mem::size_of::<u32>());

    let outputs = dispatcher.dispatch(&analysis, &scratch.inputs, None)?;
    if outputs.len() != 2 {
        return Err(DispatchError::BackendError(format!(
            "Fix: persistent_bfs dispatch expected exactly 2 outputs (frontier_out, changed), got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(
        &outputs[0],
        words,
        "gpu_dce persistent_bfs frontier_out",
        &mut scratch.frontier,
    )?;
    decode_u32_output_exact(
        &outputs[1],
        1,
        "gpu_dce persistent_bfs changed",
        &mut scratch.changed,
    )?;

    live.clear();
    live.resize(n as usize, false);
    for graph_id in 0..(n as usize) {
        let word = scratch.frontier.get(graph_id / 32).copied().unwrap_or(0);
        if word & (1u32 << (graph_id % 32)) != 0 {
            live[graph_id] = true;
        }
    }
    Ok(())
}

fn write_padded_one_u32_bytes(out: &mut Vec<u8>, buf: &[u32]) {
    if buf.is_empty() {
        write_zero_bytes(out, std::mem::size_of::<u32>());
    } else {
        write_u32_slice_le_bytes(out, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use crate::optimizer::dispatcher::oracle::CpuOracleDispatcher;
    use crate::optimizer::dispatcher::DispatchError;
    use vyre_foundation::ir::{Expr, Node, Program};
    use vyre_foundation::optimizer::fingerprint_program;
    use vyre_foundation::optimizer::passes::fusion_cse::dce::engine::dce as oracle_cpu_dce;

    fn wrapped_program(entry: Vec<Node>) -> Program {
        Program::wrapped(Vec::new(), [1, 1, 1], entry)
    }

    fn assert_parity(entry: Vec<Node>) {
        let dispatcher = CpuOracleDispatcher::new();
        let oracle_input = wrapped_program(entry.clone());
        let test_input = wrapped_program(entry);

        let oracle_out = oracle_cpu_dce(oracle_input);
        let gpu_out = gpu_dce(test_input, &dispatcher).expect("Fix: encoder accepts program");
        assert_eq!(
            fingerprint_program(&oracle_out),
            fingerprint_program(&gpu_out),
            "encoded DCE must produce a fingerprint-equal Program. oracle entry={:?} gpu entry={:?}",
            oracle_out.entry(),
            gpu_out.entry()
        );
    }

    struct MalformedDispatcher {
        outputs: Vec<Vec<u8>>,
    }

    impl OptimizerDispatcher for MalformedDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            Ok(self.outputs.clone())
        }
    }

    #[test]
    fn empty_entry_parity() {
        assert_parity(vec![]);
    }

    #[test]
    fn dce_rejects_extra_dispatch_outputs() {
        let program = wrapped_program(vec![Node::store("buf", Expr::u32(0), Expr::u32(1))]);
        let dispatcher = MalformedDispatcher {
            outputs: vec![
                u32_slice_to_le_bytes(&[1]),
                u32_slice_to_le_bytes(&[0]),
                u32_slice_to_le_bytes(&[0]),
            ],
        };
        let err = gpu_dce(program, &dispatcher).expect_err("extra outputs must be rejected");
        assert!(
            matches!(err, DceError::Dispatch(DispatchError::BackendError(_))),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn dce_rejects_trailing_changed_bytes() {
        let program = wrapped_program(vec![Node::store("buf", Expr::u32(0), Expr::u32(1))]);
        let encoded = encode_program(&program).expect("Fix: encoder accepts store");
        let words = bitset_words(encoded.node_count) as usize;
        let dispatcher = MalformedDispatcher {
            outputs: vec![u32_slice_to_le_bytes(&vec![1; words]), vec![0, 0, 0, 0, 1]],
        };
        let err = gpu_dce(program, &dispatcher).expect_err("trailing changed bytes rejected");
        assert!(
            matches!(err, DceError::Dispatch(DispatchError::BackendError(_))),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn live_mask_with_scratch_reuses_dispatch_decode_and_output_storage() {
        let program = wrapped_program(vec![Node::store("buf", Expr::u32(0), Expr::u32(1))]);
        let encoded = encode_program(&program).expect("Fix: encoder accepts store");
        let words = bitset_words(encoded.node_count) as usize;
        let dispatcher = MalformedDispatcher {
            outputs: vec![
                u32_slice_to_le_bytes(&vec![u32::MAX; words]),
                vec![0, 0, 0, 0],
            ],
        };
        let mut scratch = DceKernelScratch::default();
        let mut live = Vec::with_capacity(encoded.node_count as usize);

        compute_live_mask_with_scratch_into(&encoded, &dispatcher, &mut scratch, &mut live)
            .expect("Fix: dispatch succeeds");

        let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
        let seed_capacity = scratch.seed.capacity();
        let frontier_capacity = scratch.frontier.capacity();
        let changed_capacity = scratch.changed.capacity();
        let live_capacity = live.capacity();

        compute_live_mask_with_scratch_into(&encoded, &dispatcher, &mut scratch, &mut live)
            .expect("Fix: dispatch succeeds");

        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_capacities
        );
        assert_eq!(scratch.seed.capacity(), seed_capacity);
        assert_eq!(scratch.frontier.capacity(), frontier_capacity);
        assert_eq!(scratch.changed.capacity(), changed_capacity);
        assert_eq!(live.capacity(), live_capacity);
        assert!(live.iter().all(|&is_live| is_live));
    }

    #[test]
    fn pure_let_with_no_use_is_dropped() {
        assert_parity(vec![Node::let_bind("dead", Expr::u32(7))]);
    }

    #[test]
    fn live_let_used_by_store_is_kept() {
        assert_parity(vec![
            Node::let_bind("x", Expr::u32(7)),
            Node::store("buf", Expr::u32(0), Expr::var("x")),
        ]);
    }

    #[test]
    fn chained_lets_used_by_store_keep_chain() {
        assert_parity(vec![
            Node::let_bind("a", Expr::u32(1)),
            Node::let_bind("b", Expr::var("a")),
            Node::store("buf", Expr::u32(0), Expr::var("b")),
        ]);
    }

    #[test]
    fn unused_chain_is_dropped() {
        assert_parity(vec![
            Node::let_bind("a", Expr::u32(1)),
            Node::let_bind("b", Expr::var("a")),
            Node::let_bind("c", Expr::u32(2)),
            Node::store("buf", Expr::u32(0), Expr::var("c")),
        ]);
    }

    #[test]
    fn return_drops_unreachable_suffix() {
        assert_parity(vec![
            Node::let_bind("live", Expr::u32(1)),
            Node::store("buf", Expr::u32(0), Expr::var("live")),
            Node::Return,
            Node::let_bind("after_return", Expr::u32(99)),
            Node::store("buf", Expr::u32(0), Expr::u32(2)),
        ]);
    }

    #[test]
    fn shadowed_let_only_keeps_most_recent() {
        assert_parity(vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::let_bind("x", Expr::u32(2)),
            Node::store("buf", Expr::u32(0), Expr::var("x")),
        ]);
    }

    #[test]
    fn store_with_index_var_keeps_its_definer() {
        assert_parity(vec![
            Node::let_bind("idx", Expr::u32(3)),
            Node::store("buf", Expr::var("idx"), Expr::u32(99)),
        ]);
    }

    #[test]
    fn assign_is_always_kept() {
        assert_parity(vec![
            Node::Assign {
                name: "x".into(),
                value: Expr::u32(2),
            },
            Node::store("buf", Expr::u32(0), Expr::var("x")),
        ]);
    }

    #[test]
    fn if_with_dead_lets_in_both_branches_drops_them() {
        assert_parity(vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("dead_then", Expr::u32(0))],
            otherwise: vec![Node::let_bind("dead_else", Expr::u32(0))],
        }]);
    }

    #[test]
    fn if_branch_with_live_store_keeps_outer_definer() {
        assert_parity(vec![
            Node::let_bind("x", Expr::u32(7)),
            Node::If {
                cond: Expr::var("c"),
                then: vec![Node::store("buf", Expr::u32(0), Expr::var("x"))],
                otherwise: vec![Node::let_bind("dead_else", Expr::u32(0))],
            },
        ]);
    }

    #[test]
    fn loop_with_store_using_induction_var_is_kept() {
        assert_parity(vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(10),
            vec![Node::store("buf", Expr::var("i"), Expr::u32(0))],
        )]);
    }

    #[test]
    fn block_with_dead_let_is_kept_with_empty_body() {
        assert_parity(vec![Node::Block(vec![Node::let_bind(
            "dead_in_block",
            Expr::u32(99),
        )])]);
    }

    #[test]
    fn nested_region_with_live_store_keeps_outer_definer() {
        // Cribbed from foundation's `dce_region_live_ins_propagate_to_outer_scope`.
        assert_parity(vec![
            Node::let_bind("live", Expr::u32(7)),
            Node::Region {
                generator: "test".into(),
                source_region: None,
                body: std::sync::Arc::new(vec![Node::store(
                    "out",
                    Expr::u32(0),
                    Expr::var("live"),
                )]),
            },
        ]);
    }

    #[test]
    fn return_inside_region_truncates_region_body() {
        // Cribbed from foundation's `dce_descends_into_region_bodies`.
        assert_parity(vec![Node::Region {
            generator: "test".into(),
            source_region: None,
            body: std::sync::Arc::new(vec![
                Node::let_bind("dead", Expr::u32(1)),
                Node::Return,
                Node::let_bind("unreachable", Expr::u32(2)),
            ]),
        }]);
    }
}
