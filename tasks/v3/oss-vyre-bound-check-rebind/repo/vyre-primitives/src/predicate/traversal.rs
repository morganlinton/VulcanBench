use vyre_foundation::ir::Program;

use crate::graph::program_graph::ProgramGraphShape;

pub(crate) fn forward_edge_program(
    op_id: &'static str,
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    edge_mask: u32,
) -> Program {
    crate::graph::csr_forward_traverse::csr_forward_traverse_with_op_id(
        op_id,
        shape,
        frontier_in,
        frontier_out,
        edge_mask,
    )
}

pub(crate) fn backward_edge_program(
    op_id: &'static str,
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    edge_mask: u32,
) -> Program {
    crate::graph::csr_backward_traverse::csr_backward_traverse_with_op_id(
        op_id,
        shape,
        frontier_in,
        frontier_out,
        edge_mask,
    )
}

#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn cpu_ref_forward(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    edge_mask: u32,
) -> Vec<u32> {
    crate::graph::csr_forward_traverse::cpu_ref(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        edge_mask,
    )
}

#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn cpu_ref_forward_into(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    edge_mask: u32,
    out: &mut Vec<u32>,
) {
    crate::graph::csr_forward_traverse::cpu_ref_into(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        edge_mask,
        out,
    );
}

#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn cpu_ref_backward(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    edge_mask: u32,
) -> Vec<u32> {
    crate::graph::csr_backward_traverse::cpu_ref(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        edge_mask,
    )
}

#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn cpu_ref_backward_into(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    edge_mask: u32,
    out: &mut Vec<u32>,
) {
    crate::graph::csr_backward_traverse::cpu_ref_into(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        edge_mask,
        out,
    );
}

#[cfg(test)]
pub(crate) fn assert_region_op_id(program: &Program, expected: &'static str, label: &str) {
    let generator = match &program.entry[0] {
        vyre_foundation::ir::Node::Region { generator, .. } => generator.to_string(),
        other => panic!("Fix: {label} must build a Region entry, got {other:?}."),
    };
    assert_eq!(generator, expected);
}
