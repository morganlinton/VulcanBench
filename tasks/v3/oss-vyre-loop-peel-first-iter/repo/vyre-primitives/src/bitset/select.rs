//! Select navigation for packed bitvectors.
//!
//! `select1_query` maps one-based ranks to zero-based bit positions. It is a
//! Tier-2.5 primitive because succinct AST, graph, parser, and security
//! structures all need the same packed-bit navigation substrate.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::bitset::select1_query";

/// Build a Program that answers `select1` queries over a packed u32 bitvector.
///
/// Each `k_indices[q]` is a one-based rank. The output is the zero-based bit
/// position of the `k`-th set bit. `k == 0` and `k > total_popcount` trap
/// loudly so callers cannot silently navigate to a bogus AST or graph node.
#[must_use]
pub fn select1_query(
    bits: &str,
    k_indices: &str,
    out: &str,
    word_count: u32,
    query_count: u32,
) -> Program {
    let q = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::lt(q.clone(), Expr::u32(query_count)),
        vec![
            Node::let_bind("select_k", Expr::load(k_indices, q.clone())),
            Node::if_then(
                Expr::eq(Expr::var("select_k"), Expr::u32(0)),
                vec![Node::trap(Expr::var("select_k"), "select-query-zero-rank")],
            ),
            Node::let_bind("select_remaining", Expr::var("select_k")),
            Node::let_bind("select_found", Expr::u32(0)),
            Node::let_bind("select_result", Expr::u32(0)),
            Node::loop_for(
                "select_word_idx",
                Expr::u32(0),
                Expr::u32(word_count),
                vec![Node::if_then(
                    Expr::eq(Expr::var("select_found"), Expr::u32(0)),
                    vec![
                        Node::let_bind(
                            "select_word",
                            Expr::load(bits, Expr::var("select_word_idx")),
                        ),
                        Node::let_bind("select_word_pop", Expr::popcount(Expr::var("select_word"))),
                        Node::if_then_else(
                            Expr::gt(Expr::var("select_remaining"), Expr::var("select_word_pop")),
                            vec![Node::assign(
                                "select_remaining",
                                Expr::sub(
                                    Expr::var("select_remaining"),
                                    Expr::var("select_word_pop"),
                                ),
                            )],
                            vec![
                                Node::let_bind("select_word_scan", Expr::var("select_word")),
                                Node::loop_for(
                                    "select_skip",
                                    Expr::u32(1),
                                    Expr::var("select_remaining"),
                                    vec![Node::assign(
                                        "select_word_scan",
                                        Expr::bitand(
                                            Expr::var("select_word_scan"),
                                            Expr::sub(Expr::var("select_word_scan"), Expr::u32(1)),
                                        ),
                                    )],
                                ),
                                Node::assign(
                                    "select_result",
                                    Expr::add(
                                        Expr::mul(Expr::var("select_word_idx"), Expr::u32(32)),
                                        Expr::ctz(Expr::var("select_word_scan")),
                                    ),
                                ),
                                Node::assign("select_found", Expr::u32(1)),
                                Node::assign("select_remaining", Expr::u32(1)),
                            ],
                        ),
                    ],
                )],
            ),
            Node::if_then(
                Expr::eq(Expr::var("select_found"), Expr::u32(0)),
                vec![Node::trap(
                    Expr::var("select_k"),
                    "select-query-rank-out-of-bounds",
                )],
            ),
            Node::store(out, q, Expr::var("select_result")),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(bits, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(word_count.max(1)),
            BufferDecl::storage(k_indices, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(query_count.max(1)),
            BufferDecl::output(out, 2, DataType::U32).with_count(query_count.max(1)),
        ],
        [64, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || select1_query("bits", "queries", "out", 4, 5),
        Some(|| {
            let bits = [0b1011u32, 0x8000_0000, 0xFFFF_0000, 0u32];
            let queries = [1u32, 2, 3, 4, 5];
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&bits), to_bytes(&queries), vec![0u8; 5 * 4]]]
        }),
        Some(|| {
            let expected = [0u32, 1, 3, 63, 80];
            let bytes = crate::wire::pack_u32_slice(&expected);
            vec![vec![bytes]]
        }),
    )
}
