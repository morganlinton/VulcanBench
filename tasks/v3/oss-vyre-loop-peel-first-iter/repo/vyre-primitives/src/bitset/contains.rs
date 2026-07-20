//! `bitset_contains`  -  query one bit at a given index.
//!
//! `out[0] = (input[index / 32] >> (index % 32)) & 1`. Single-lane
//! Program. Consumed by a external analyzer's point-lookup predicates (e.g.
//! `target ∈ frontier`).

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::bitset::contains";

/// Build a Program: `out[0]` = bit at `index_buffer[0]` of `input`.
#[must_use]
pub fn bitset_contains(input: &str, index_buffer: &str, out: &str, words: u32) -> Program {
    // AUDIT_2026-04-24 F-BSC-01: gate the `input[idx/32]` load on
    // an in-bounds check. Prior code loaded unconditionally, which
    // on a misconfigured predicate (index >= words * 32) produced
    // an OOB read on the GPU. Now out-of-range indices yield 0,
    // matching the cpu_ref semantics below.
    let body = vec![
        Node::let_bind("idx", Expr::load(index_buffer, Expr::u32(0))),
        Node::let_bind("word_idx", Expr::shr(Expr::var("idx"), Expr::u32(5))),
        Node::if_then_else(
            Expr::lt(Expr::var("word_idx"), Expr::u32(words)),
            vec![
                Node::let_bind("word", Expr::load(input, Expr::var("word_idx"))),
                Node::let_bind(
                    "bit",
                    Expr::bitand(
                        Expr::shr(
                            Expr::var("word"),
                            Expr::bitand(Expr::var("idx"), Expr::u32(31)),
                        ),
                        Expr::u32(1),
                    ),
                ),
                Node::store(out, Expr::u32(0), Expr::var("bit")),
            ],
            vec![Node::store(out, Expr::u32(0), Expr::u32(0))],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(words),
            BufferDecl::storage(index_buffer, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1),
            BufferDecl::storage(out, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
                body,
            )]),
        }],
    )
}

/// CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(input: &[u32], index: u32) -> u32 {
    let w = (index / 32) as usize;
    let b = index % 32;
    if w < input.len() {
        (input[w] >> b) & 1
    } else {
        0
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || bitset_contains("input", "index", "out", 1),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0b1010]), to_bytes(&[1]), to_bytes(&[0])]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[1])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_single_bit() {
        assert_eq!(cpu_ref(&[0b1010], 1), 1);
        assert_eq!(cpu_ref(&[0b1010], 0), 0);
        assert_eq!(cpu_ref(&[0b1010], 3), 1);
    }
}
