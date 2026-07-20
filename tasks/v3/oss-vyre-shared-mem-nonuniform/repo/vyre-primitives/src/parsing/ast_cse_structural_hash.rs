//! Structural-hash CSE probe/insert wave.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::hash::fnv1a::{fnv1a32_mul_xor_word_expr, fnv1a32_mul_xor_word_state};

use super::ast_ops::{AST_ADD, AST_PTR_DEREF, AST_VAR};

/// Stable op id for the structural CSE child region.
pub const OP_ID: &str = "vyre-primitives::parsing::ast_cse_structural_hash";

/// Emit the structural-hash deduplication phase.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn ast_cse_structural_hash(
    ast_opcodes: &str,
    ast_lefts: &str,
    ast_rights: &str,
    ast_vals: &str,
    hash_set: &str,
    hash_set_capacity: u32,
    out_modified_flag: &str,
    t: Expr,
) -> Vec<Node> {
    vec![Node::if_then(
        Expr::or(
            Expr::eq(Expr::var("op"), Expr::u32(AST_ADD)),
            Expr::eq(Expr::var("op"), Expr::u32(AST_PTR_DEREF)),
        ),
        vec![
            Node::let_bind("l_idx2", Expr::load(ast_lefts, t.clone())),
            Node::let_bind("r_idx2", Expr::load(ast_rights, t.clone())),
            Node::let_bind(
                "h",
                fnv1a32_mul_xor_word_expr(Expr::var("op"), Expr::var("l_idx2")),
            ),
            Node::assign(
                "h",
                fnv1a32_mul_xor_word_expr(Expr::var("h"), Expr::var("r_idx2")),
            ),
            Node::let_bind(
                "slot",
                Expr::rem(Expr::var("h"), Expr::u32(hash_set_capacity)),
            ),
            Node::let_bind("active", Expr::bool(true)),
            Node::loop_for(
                "probe",
                Expr::u32(0),
                Expr::u32(hash_set_capacity),
                vec![Node::if_then(
                    Expr::var("active"),
                    vec![
                        Node::let_bind("slot_hash", Expr::mul(Expr::var("slot"), Expr::u32(2))),
                        Node::let_bind("slot_idx", Expr::add(Expr::var("slot_hash"), Expr::u32(1))),
                        Node::let_bind(
                            "old_hash",
                            Expr::atomic_compare_exchange(
                                hash_set,
                                Expr::var("slot_hash"),
                                Expr::u32(0),
                                Expr::var("h"),
                            ),
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("old_hash"), Expr::u32(0)),
                            vec![
                                Node::let_bind(
                                    "_",
                                    Expr::atomic_exchange(
                                        hash_set,
                                        Expr::var("slot_idx"),
                                        t.clone(),
                                    ),
                                ),
                                Node::assign("active", Expr::bool(false)),
                            ],
                        ),
                        Node::let_bind(
                            "earliest",
                            Expr::Select {
                                cond: Box::new(Expr::eq(Expr::var("old_hash"), Expr::var("h"))),
                                true_val: Box::new(Expr::atomic_add(
                                    hash_set,
                                    Expr::var("slot_idx"),
                                    Expr::u32(0),
                                )),
                                false_val: Box::new(Expr::u32(u32::MAX)),
                            },
                        ),
                        Node::if_then(
                            Expr::and(
                                Expr::eq(Expr::var("old_hash"), Expr::var("h")),
                                Expr::lt(Expr::var("earliest"), t.clone()),
                            ),
                            vec![
                                Node::store(ast_opcodes, t.clone(), Expr::u32(AST_VAR)),
                                Node::store(ast_vals, t.clone(), Expr::var("earliest")),
                                Node::let_bind(
                                    "_",
                                    Expr::atomic_add(out_modified_flag, Expr::u32(0), Expr::u32(1)),
                                ),
                            ],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("old_hash"), Expr::var("h")),
                            vec![Node::assign("active", Expr::bool(false))],
                        ),
                        Node::assign(
                            "slot",
                            Expr::rem(
                                Expr::add(Expr::var("slot"), Expr::u32(1)),
                                Expr::u32(hash_set_capacity),
                            ),
                        ),
                    ],
                )],
            ),
        ],
    )]
}

/// Build the standalone structural-hash CSE primitive.
#[must_use]
pub fn ast_cse_structural_hash_program(num_nodes: u32, hash_set_capacity: u32) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let body = vec![
        Node::if_then(
            Expr::lt(t.clone(), Expr::u32(hash_set_capacity)),
            vec![
                Node::store("hash_set", Expr::mul(t.clone(), Expr::u32(2)), Expr::u32(0)),
                Node::store(
                    "hash_set",
                    Expr::add(Expr::mul(t.clone(), Expr::u32(2)), Expr::u32(1)),
                    Expr::u32(u32::MAX),
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(t.clone(), Expr::u32(0)),
            vec![Node::loop_for(
                "node_idx",
                Expr::u32(0),
                Expr::u32(num_nodes),
                vec![
                    Node::let_bind("op", Expr::load("ast_opcodes", Expr::var("node_idx"))),
                    Node::if_then(
                        Expr::or(
                            Expr::eq(Expr::var("op"), Expr::u32(AST_ADD)),
                            Expr::eq(Expr::var("op"), Expr::u32(AST_PTR_DEREF)),
                        ),
                        vec![
                            Node::let_bind(
                                "l_idx2",
                                Expr::load("ast_lefts", Expr::var("node_idx")),
                            ),
                            Node::let_bind(
                                "r_idx2",
                                Expr::load("ast_rights", Expr::var("node_idx")),
                            ),
                            Node::let_bind(
                                "h",
                                fnv1a32_mul_xor_word_expr(Expr::var("op"), Expr::var("l_idx2")),
                            ),
                            Node::assign(
                                "h",
                                fnv1a32_mul_xor_word_expr(Expr::var("h"), Expr::var("r_idx2")),
                            ),
                            Node::let_bind(
                                "slot",
                                Expr::rem(Expr::var("h"), Expr::u32(hash_set_capacity)),
                            ),
                            Node::let_bind("active", Expr::bool(true)),
                            Node::loop_for(
                                "probe",
                                Expr::u32(0),
                                Expr::u32(hash_set_capacity),
                                vec![Node::if_then(
                                    Expr::var("active"),
                                    vec![
                                        Node::let_bind(
                                            "slot_hash",
                                            Expr::mul(Expr::var("slot"), Expr::u32(2)),
                                        ),
                                        Node::let_bind(
                                            "slot_idx",
                                            Expr::add(Expr::var("slot_hash"), Expr::u32(1)),
                                        ),
                                        Node::let_bind(
                                            "old_hash",
                                            Expr::load("hash_set", Expr::var("slot_hash")),
                                        ),
                                        Node::if_then(
                                            Expr::eq(Expr::var("old_hash"), Expr::u32(0)),
                                            vec![
                                                Node::store(
                                                    "hash_set",
                                                    Expr::var("slot_hash"),
                                                    Expr::var("h"),
                                                ),
                                                Node::store(
                                                    "hash_set",
                                                    Expr::var("slot_idx"),
                                                    Expr::var("node_idx"),
                                                ),
                                                Node::assign("active", Expr::bool(false)),
                                            ],
                                        ),
                                        Node::if_then(
                                            Expr::eq(Expr::var("old_hash"), Expr::var("h")),
                                            vec![
                                                Node::let_bind(
                                                    "earliest",
                                                    Expr::load("hash_set", Expr::var("slot_idx")),
                                                ),
                                                Node::if_then(
                                                    Expr::lt(
                                                        Expr::var("earliest"),
                                                        Expr::var("node_idx"),
                                                    ),
                                                    vec![
                                                        Node::store(
                                                            "ast_opcodes",
                                                            Expr::var("node_idx"),
                                                            Expr::u32(AST_VAR),
                                                        ),
                                                        Node::store(
                                                            "ast_vals",
                                                            Expr::var("node_idx"),
                                                            Expr::var("earliest"),
                                                        ),
                                                        Node::let_bind(
                                                            "_",
                                                            Expr::atomic_add(
                                                                "out_modified_flag",
                                                                Expr::u32(0),
                                                                Expr::u32(1),
                                                            ),
                                                        ),
                                                    ],
                                                ),
                                                Node::assign("active", Expr::bool(false)),
                                            ],
                                        ),
                                        Node::assign(
                                            "slot",
                                            Expr::rem(
                                                Expr::add(Expr::var("slot"), Expr::u32(1)),
                                                Expr::u32(hash_set_capacity),
                                            ),
                                        ),
                                    ],
                                )],
                            ),
                        ],
                    ),
                ],
            )],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage("ast_opcodes", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(num_nodes),
            BufferDecl::storage("ast_lefts", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(num_nodes),
            BufferDecl::storage("ast_rights", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(num_nodes),
            BufferDecl::storage("ast_vals", 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(num_nodes),
            BufferDecl::storage("hash_set", 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(hash_set_capacity.saturating_mul(2)),
            BufferDecl::storage(
                "out_modified_flag",
                5,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(1),
        ],
        [num_nodes.max(hash_set_capacity).max(1), 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

#[cfg(feature = "inventory-registry")]
fn fixture_u32(words: &[u32]) -> Vec<u8> {
    crate::wire::pack_u32_slice(words)
}

#[cfg(feature = "inventory-registry")]
fn structural_hash(op: u32, left: u32, right: u32) -> u32 {
    let h = fnv1a32_mul_xor_word_state(op, left);
    fnv1a32_mul_xor_word_state(h, right)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || ast_cse_structural_hash_program(2, 8),
        Some(|| vec![vec![
            fixture_u32(&[AST_ADD, AST_ADD]),
            fixture_u32(&[1, 1]),
            fixture_u32(&[2, 2]),
            fixture_u32(&[0, 0]),
            fixture_u32(&[0; 16]),
            fixture_u32(&[0]),
        ]]),
        Some(|| {
            let h = structural_hash(AST_ADD, 1, 2);
            let mut hash_set = [0_u32; 16];
            for slot in 0..8 {
                hash_set[slot * 2 + 1] = u32::MAX;
            }
            let slot = (h % 8) as usize;
            hash_set[slot * 2] = h;
            hash_set[slot * 2 + 1] = 0;
            vec![vec![
                fixture_u32(&[AST_ADD, AST_VAR]),
                fixture_u32(&[0, 0]),
                fixture_u32(&hash_set),
                fixture_u32(&[1]),
            ]]
        }),
    )
}
