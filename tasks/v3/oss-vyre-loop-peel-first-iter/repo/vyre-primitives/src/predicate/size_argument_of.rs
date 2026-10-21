//! `size_argument_of`  -  reverse CallArg traversal for size argument
//! candidates.
//!
//! The primitive marks argument nodes whose callee is in the input
//! frontier. Rule-level predicates own any additional node-kind
//! filtering.

use vyre_foundation::ir::Program;

use crate::graph::csr_backward_traverse::csr_backward_traverse_with_op_id;
use crate::graph::program_graph::ProgramGraphShape;
use crate::predicate::edge_kind;
#[cfg(feature = "inventory-registry")]
use crate::predicate::node_kind;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::predicate::size_argument_of";

/// Build a Program that reverse-traverses CallArg edges and marks
/// argument nodes whose callees are in `frontier_in`.
///
/// Downstream analyzer rules own any additional node-kind predicates at the rule
/// layer. This primitive deliberately avoids a baked-in Literal filter:
/// allocator size arguments are often computed expressions, and
/// filtering here would erase realistic vulnerability witnesses before
/// rule-specific predicates can inspect them.
#[must_use]
pub fn size_argument_of(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
) -> Program {
    csr_backward_traverse_with_op_id(OP_ID, shape, frontier_in, frontier_out, edge_kind::CALL_ARG)
}

/// CPU reference: reverse-traverse CallArg edges and mark every caller
/// argument whose callee bit is present in `frontier_in`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(
    node_count: u32,
    _nodes: &[u32],
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
) -> Vec<u32> {
    crate::graph::csr_backward_traverse::cpu_ref(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        edge_kind::CALL_ARG,
    )
}

/// CPU reference using caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(
    node_count: u32,
    _nodes: &[u32],
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    out: &mut Vec<u32>,
) {
    crate::graph::csr_backward_traverse::cpu_ref_into(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        edge_kind::CALL_ARG,
        out,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::Node;

    #[test]
    fn preserves_wrapper_op_id() {
        let program = size_argument_of(ProgramGraphShape::new(4, 2), "fin", "fout");
        let generator = match &program.entry[0] {
            Node::Region { generator, .. } => generator.to_string(),
            other => panic!("Fix: size_argument_of must build a Region entry, got {other:?}."),
        };
        assert_eq!(generator, OP_ID);
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || size_argument_of(ProgramGraphShape::new(4, 4), "fin", "fout"),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[node_kind::LITERAL, node_kind::CALL, node_kind::LITERAL, node_kind::CALL]),
                to_bytes(&[0, 1, 2, 3, 4]),
                to_bytes(&[1, 2, 3, 0]),
                to_bytes(&[edge_kind::CALL_ARG, 0, edge_kind::CALL_ARG, 0]),
                to_bytes(&[0, 0, 0, 0]),
                to_bytes(&[0b1010]),
                to_bytes(&[0]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0b0101])]]
        }),
    )
}
