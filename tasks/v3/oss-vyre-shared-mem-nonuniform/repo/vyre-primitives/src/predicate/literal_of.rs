//! `literal_of`  -  `NodeSet = { v : nodes[v] == Literal AND
//!                                  literal_values[v] == probe }`.
//!
//! The IR-level primitive filters by NodeKind only; a external analyzer's
//! type-inference ensures `literal_of(probe)` is only lowered against
//! literal-typed frontiers. A runtime match on the literal value can
//! be composed by re-filtering with a dedicated literal-payload
//! comparison primitive in Tier 3.

use vyre_foundation::ir::Program;

use crate::predicate::node_kind;
use crate::predicate::node_kind_eq::node_kind_eq_with_op_id;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::predicate::literal_of";

/// Build a Program that emits every node whose kind is Literal.
#[must_use]
pub fn literal_of(nodes: &str, nodeset_out: &str, node_count: u32) -> Program {
    node_kind_eq_with_op_id(OP_ID, nodes, nodeset_out, node_count, node_kind::LITERAL)
}

/// CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(nodes: &[u32]) -> Vec<u32> {
    crate::predicate::node_kind_eq::cpu_ref(nodes, node_kind::LITERAL)
}

/// CPU reference using a caller-owned nodeset bitset.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(nodes: &[u32], out: &mut Vec<u32>) {
    crate::predicate::node_kind_eq::cpu_ref_into(nodes, node_kind::LITERAL, out);
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || literal_of("nodes", "nodeset", 4),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[1, 2, 1, 4]), // nodes: VARIABLE, CALL, VARIABLE, LITERAL
                to_bytes(&[0]),          // nodeset_out
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0b1000])]] // node 3 (LITERAL)
        }),
    )
}
