//! `node_kind_eq`  -  `NodeSet = { v : nodes[v] == kind }`.

use vyre_foundation::ir::Program;

#[cfg(any(test, feature = "cpu-parity"))]
use crate::nodeset_filter::{nodeset_filter_cpu_ref, nodeset_filter_cpu_ref_into};
use crate::nodeset_filter::{nodeset_filter_program, NodeSetFilter};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::predicate::node_kind_eq";

/// Build a Program: `NodeSet = { v : nodes[v] == kind }`.
#[must_use]
pub fn node_kind_eq(nodes: &str, nodeset_out: &str, node_count: u32, kind: u32) -> Program {
    node_kind_eq_with_op_id(OP_ID, nodes, nodeset_out, node_count, kind)
}

/// Build a node-kind predicate Program under a caller-owned op id.
#[must_use]
pub(crate) fn node_kind_eq_with_op_id(
    op_id: &'static str,
    nodes: &str,
    nodeset_out: &str,
    node_count: u32,
    kind: u32,
) -> Program {
    nodeset_filter_program(
        op_id,
        nodes,
        nodeset_out,
        node_count,
        NodeSetFilter::Eq(kind),
    )
}

/// CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(nodes: &[u32], kind: u32) -> Vec<u32> {
    nodeset_filter_cpu_ref(nodes, NodeSetFilter::Eq(kind))
}

/// CPU reference using a caller-owned nodeset bitset.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(nodes: &[u32], kind: u32, out: &mut Vec<u32>) {
    nodeset_filter_cpu_ref_into(nodes, NodeSetFilter::Eq(kind), out);
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || node_kind_eq("nodes", "nodeset", 4, crate::predicate::node_kind::CALL),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[2, 1, 2, 4]), // nodes: CALL, VARIABLE, CALL, LITERAL
                to_bytes(&[0]),          // nodeset_out
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0b0101])]] // nodes 0 and 2 (CALL)
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::predicate::node_kind;

    #[test]
    fn filters_by_kind() {
        let got = cpu_ref(
            &[
                node_kind::CALL,
                node_kind::VARIABLE,
                node_kind::CALL,
                node_kind::LITERAL,
            ],
            node_kind::CALL,
        );
        assert_eq!(got, vec![0b0101]);
    }

    #[test]
    fn cpu_ref_into_reuses_nodeset_buffer() {
        let mut out = Vec::with_capacity(4);
        let ptr = out.as_ptr();
        cpu_ref_into(
            &[
                node_kind::CALL,
                node_kind::VARIABLE,
                node_kind::CALL,
                node_kind::LITERAL,
            ],
            node_kind::CALL,
            &mut out,
        );
        assert_eq!(out, vec![0b0101]);
        assert_eq!(out.as_ptr(), ptr);
    }
}
