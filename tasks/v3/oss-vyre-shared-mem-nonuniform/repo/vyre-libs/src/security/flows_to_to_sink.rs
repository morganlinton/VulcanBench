//! `flows_to_to_sink`  -  composite source→sink reachability primitive.
//!
//! The "does taint reach a sink node" pattern is the single most
//! common composition in every taint-style generic query dialect rule:
//!
//! ```text
//!   reach    = csr_forward_traverse(source, FLOWS_TO_MASK)
//!   hits     = reach AND sink
//!   any_hit  = bitset_any(hits) → u32
//! ```
//!
//! Earlier lowering paths emitted this composition
//! inline at every call site (~25 lines of boilerplate per call,
//! plus a fresh accumulator buffer per invocation). Centralising it
//! here as one fused Region:
//!
//! * cuts per-call lowering surface from ~5 sub-programs
//!   merged via `merge_programs` to one helper invocation;
//! * gives the optimizer one Region with a stable op id to fuse,
//!   cache, and CSE across rules;
//! * eliminates the "did you remember to compose all three steps"
//!   foot-gun that the audit caught when `flows_to_via` and
//!   `flows_to_not_via` silently shared the same emitted Program.
//!
//! Soundness: identical to the one BFS step `flows_to` provides  -
//! [`MayOver`](super::super::dataflow::Soundness::MayOver) on a single
//! step, `Exact` when iterated to fixpoint with sanitizer gating.

use vyre::ir::Program;
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::predicate::edge_kind;

use crate::security::flow_composition::dataflow_hit_program;
#[cfg(test)]
use crate::security::flow_composition::{any_dataflow_hit_cpu_ref, dataflow_reach_step_cpu_ref};

pub(crate) const OP_ID: &str = "vyre-libs::security::flows_to_to_sink";

/// One BFS step from `source_buf` along dataflow edges, intersected
/// with `sink_buf`, reduced to a single u32 stored in `out_scalar_buf`.
///
/// Buffers:
/// * `source_buf`   -  read-only bitset of source-tagged nodes.
/// * `sink_buf`     -  read-only bitset of sink-tagged nodes.
/// * `reach_buf`    -  read-write scratch bitset for the BFS step result.
/// * `hits_buf`     -  read-write scratch bitset for the AND result.
/// * `out_scalar_buf`  -  read-write 1-word output: nonzero iff any
///   sink node was reached.
#[must_use]
pub fn flows_to_to_sink(
    shape: ProgramGraphShape,
    source_buf: &str,
    sink_buf: &str,
    reach_buf: &str,
    hits_buf: &str,
    out_scalar_buf: &str,
) -> Program {
    dataflow_hit_program(
        OP_ID,
        shape,
        source_buf,
        sink_buf,
        reach_buf,
        hits_buf,
        out_scalar_buf,
    )
}

/// CPU oracle: walks one BFS step from `source` along dataflow edges,
/// intersects with `sink`, returns 1 if any bit set, 0 otherwise.
#[must_use]
#[cfg(test)]
pub(crate) fn cpu_ref(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    source: &[u32],
    sink: &[u32],
) -> u32 {
    let reach = dataflow_reach_step_cpu_ref(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        source,
    );
    any_dataflow_hit_cpu_ref(&reach, sink)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || flows_to_to_sink(ProgramGraphShape::new(4, 3), "source", "sink", "reach", "hits", "out_scalar"),
        test_inputs: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![
                to_bytes(&[0, 0, 0, 0]),          // pg_nodes
                to_bytes(&[0, 1, 2, 3, 3]),       // pg_edge_offsets
                to_bytes(&[1, 2, 3]),             // pg_edge_targets
                to_bytes(&[
                    edge_kind::ASSIGNMENT,
                    edge_kind::ASSIGNMENT,
                    edge_kind::ASSIGNMENT,
                ]),                               // pg_edge_kind_mask
                to_bytes(&[0, 0, 0, 0]),          // pg_node_tags
                to_bytes(&[0b0001]),              // source = {0}
                to_bytes(&[0b0001]),              // reach = {0}
                to_bytes(&[0b0010]),              // sink = {1}
                to_bytes(&[0b0000]),              // hits
                to_bytes(&[0b0000]),              // out_scalar
            ]]
        }),
        expected_output: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![
                to_bytes(&[0b0011]),              // reach = {0,1}
                to_bytes(&[0b0010]),              // hits = {1}
                to_bytes(&[0b0001]),              // out_scalar = 1
            ]]
        }),
        category: Some("security"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::flow_composition::linear_dataflow;

    #[test]
    fn one_hop_source_reaches_sink_returns_one() {
        let (off, tgt, msk) = linear_dataflow(4);
        let source = [0b0001u32]; // node 0
        let sink = [0b0010u32]; // node 1 (one hop away)
        let result = cpu_ref(4, &off, &tgt, &msk, &source, &sink);
        assert_eq!(result, 1);
    }

    #[test]
    fn two_hops_unreachable_in_one_step_returns_zero() {
        let (off, tgt, msk) = linear_dataflow(4);
        let source = [0b0001u32]; // node 0
        let sink = [0b0100u32]; // node 2 (two hops away  -  not reached in one step)
        let result = cpu_ref(4, &off, &tgt, &msk, &source, &sink);
        assert_eq!(result, 0);
    }

    #[test]
    fn empty_source_returns_zero() {
        let (off, tgt, msk) = linear_dataflow(4);
        let source = [0u32];
        let sink = [0b0010u32];
        let result = cpu_ref(4, &off, &tgt, &msk, &source, &sink);
        assert_eq!(result, 0);
    }

    #[test]
    fn empty_sink_returns_zero() {
        let (off, tgt, msk) = linear_dataflow(4);
        let source = [0b0001u32];
        let sink = [0u32];
        let result = cpu_ref(4, &off, &tgt, &msk, &source, &sink);
        assert_eq!(result, 0);
    }
}
