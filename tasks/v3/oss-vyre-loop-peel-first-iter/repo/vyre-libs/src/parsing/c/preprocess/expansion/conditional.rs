//! Conditional-compilation mask builders for macro expansion.

#![allow(missing_docs)] // Internal macro-expansion helpers are documented at the owning module boundary.
use crate::parsing::c::lex::tokens::*;
use crate::parsing::composition::child_phase;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::*;

pub fn opt_conditional_mask(tok_types: &str, out_mask: &str, num_tokens: Expr) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let tok_count = match &num_tokens {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    let tok_buffer_count = tok_count.max(1);
    let entry = if tok_count == 0 {
        vec![Node::trap(
            Expr::u32(0),
            "conditional-mask-empty-token-stream",
        )]
    } else {
        vec![Node::if_then(
            Expr::lt(t.clone(), num_tokens),
            vec![
                Node::store(out_mask, t.clone(), Expr::u32(1)), // Base mask
            ],
        )]
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_buffer_count),
            BufferDecl::storage(out_mask, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(tok_buffer_count),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::opt_conditional_mask",
            entry,
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::opt_conditional_mask")
    .with_non_composable_with_self(true)
}

fn lower_depth_mask(depth: Expr) -> Expr {
    Expr::sub(Expr::shl(Expr::u32(1), depth), Expr::u32(1))
}

fn all_enclosing_active(active_bits: Expr, depth: Expr) -> Expr {
    let mask = lower_depth_mask(depth);
    Expr::eq(Expr::bitand(active_bits, mask.clone()), mask)
}

fn directive_is_conditional_open(kind: Expr) -> Expr {
    Expr::or(
        Expr::eq(kind.clone(), Expr::u32(TOK_PP_IF)),
        Expr::or(
            Expr::eq(kind.clone(), Expr::u32(TOK_PP_IFDEF)),
            Expr::eq(kind, Expr::u32(TOK_PP_IFNDEF)),
        ),
    )
}

/// GPU conditional-compilation mask over classified preprocessor directives.
#[must_use]
pub fn opt_conditional_mask_with_directives(
    tok_types: &str,
    directive_kinds: &str,
    directive_values: &str,
    out_mask: &str,
    num_tokens: Expr,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let tok_count = match &num_tokens {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    let tok_buffer_count = tok_count.max(1);

    let mut per_token = vec![
        Node::let_bind("tok", Expr::load(tok_types, Expr::var("i"))),
        Node::let_bind("kind", Expr::load(directive_kinds, Expr::var("i"))),
        Node::let_bind("value", Expr::load(directive_values, Expr::var("i"))),
        Node::let_bind(
            "current_active",
            all_enclosing_active(Expr::var("active_bits"), Expr::var("depth")),
        ),
        Node::let_bind(
            "is_control_directive",
            Expr::and(
                Expr::eq(Expr::var("tok"), Expr::u32(TOK_PREPROC)),
                Expr::or(
                    directive_is_conditional_open(Expr::var("kind")),
                    Expr::or(
                        Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_ELIF)),
                        Expr::or(
                            Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_ELSE)),
                            Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_ENDIF)),
                        ),
                    ),
                ),
            ),
        ),
        Node::store(
            out_mask,
            Expr::var("i"),
            Expr::select(
                Expr::var("is_control_directive"),
                Expr::u32(1),
                Expr::select(Expr::var("current_active"), Expr::u32(1), Expr::u32(0)),
            ),
        ),
    ];

    per_token.extend([
        Node::if_then(
            directive_is_conditional_open(Expr::var("kind")),
            vec![
                Node::if_then(
                    Expr::ge(Expr::var("depth"), Expr::u32(31)),
                    vec![Node::trap(
                        Expr::var("i"),
                        "c-preprocess-conditional-nesting-overflow",
                    )],
                ),
                Node::let_bind("open_bit", Expr::shl(Expr::u32(1), Expr::var("depth"))),
                Node::let_bind(
                    "open_active",
                    Expr::and(
                        Expr::var("current_active"),
                        Expr::ne(Expr::var("value"), Expr::u32(0)),
                    ),
                ),
                Node::assign(
                    "active_bits",
                    Expr::bitor(
                        Expr::bitand(
                            Expr::var("active_bits"),
                            Expr::bitnot(Expr::var("open_bit")),
                        ),
                        Expr::select(
                            Expr::var("open_active"),
                            Expr::var("open_bit"),
                            Expr::u32(0),
                        ),
                    ),
                ),
                Node::assign(
                    "taken_bits",
                    Expr::bitor(
                        Expr::bitand(Expr::var("taken_bits"), Expr::bitnot(Expr::var("open_bit"))),
                        Expr::select(
                            Expr::ne(Expr::var("value"), Expr::u32(0)),
                            Expr::var("open_bit"),
                            Expr::u32(0),
                        ),
                    ),
                ),
                Node::assign("depth", Expr::add(Expr::var("depth"), Expr::u32(1))),
            ],
        ),
        Node::if_then(
            Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_ELIF)),
            vec![
                Node::if_then(
                    Expr::eq(Expr::var("depth"), Expr::u32(0)),
                    vec![Node::trap(
                        Expr::var("i"),
                        "c-preprocess-elif-without-open-conditional",
                    )],
                ),
                Node::let_bind("slot_depth", Expr::sub(Expr::var("depth"), Expr::u32(1))),
                Node::let_bind("slot_bit", Expr::shl(Expr::u32(1), Expr::var("slot_depth"))),
                Node::let_bind(
                    "parent_active",
                    all_enclosing_active(Expr::var("active_bits"), Expr::var("slot_depth")),
                ),
                Node::let_bind(
                    "slot_taken",
                    Expr::ne(
                        Expr::bitand(Expr::var("taken_bits"), Expr::var("slot_bit")),
                        Expr::u32(0),
                    ),
                ),
                Node::let_bind(
                    "elif_active",
                    Expr::and(
                        Expr::and(
                            Expr::var("parent_active"),
                            Expr::not(Expr::var("slot_taken")),
                        ),
                        Expr::ne(Expr::var("value"), Expr::u32(0)),
                    ),
                ),
                Node::assign(
                    "active_bits",
                    Expr::bitor(
                        Expr::bitand(
                            Expr::var("active_bits"),
                            Expr::bitnot(Expr::var("slot_bit")),
                        ),
                        Expr::select(
                            Expr::var("elif_active"),
                            Expr::var("slot_bit"),
                            Expr::u32(0),
                        ),
                    ),
                ),
                Node::assign(
                    "taken_bits",
                    Expr::bitor(
                        Expr::var("taken_bits"),
                        Expr::select(
                            Expr::ne(Expr::var("value"), Expr::u32(0)),
                            Expr::var("slot_bit"),
                            Expr::u32(0),
                        ),
                    ),
                ),
            ],
        ),
        Node::if_then(
            Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_ELSE)),
            vec![
                Node::if_then(
                    Expr::eq(Expr::var("depth"), Expr::u32(0)),
                    vec![Node::trap(
                        Expr::var("i"),
                        "c-preprocess-else-without-open-conditional",
                    )],
                ),
                Node::let_bind(
                    "else_slot_depth",
                    Expr::sub(Expr::var("depth"), Expr::u32(1)),
                ),
                Node::let_bind(
                    "else_slot_bit",
                    Expr::shl(Expr::u32(1), Expr::var("else_slot_depth")),
                ),
                Node::let_bind(
                    "else_parent_active",
                    all_enclosing_active(Expr::var("active_bits"), Expr::var("else_slot_depth")),
                ),
                Node::let_bind(
                    "else_taken",
                    Expr::ne(
                        Expr::bitand(Expr::var("taken_bits"), Expr::var("else_slot_bit")),
                        Expr::u32(0),
                    ),
                ),
                Node::let_bind(
                    "else_active",
                    Expr::and(
                        Expr::var("else_parent_active"),
                        Expr::not(Expr::var("else_taken")),
                    ),
                ),
                Node::assign(
                    "active_bits",
                    Expr::bitor(
                        Expr::bitand(
                            Expr::var("active_bits"),
                            Expr::bitnot(Expr::var("else_slot_bit")),
                        ),
                        Expr::select(
                            Expr::var("else_active"),
                            Expr::var("else_slot_bit"),
                            Expr::u32(0),
                        ),
                    ),
                ),
                Node::assign(
                    "taken_bits",
                    Expr::bitor(Expr::var("taken_bits"), Expr::var("else_slot_bit")),
                ),
            ],
        ),
        Node::if_then(
            Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_ENDIF)),
            vec![
                Node::if_then(
                    Expr::eq(Expr::var("depth"), Expr::u32(0)),
                    vec![Node::trap(
                        Expr::var("i"),
                        "c-preprocess-endif-without-open-conditional",
                    )],
                ),
                Node::assign("depth", Expr::sub(Expr::var("depth"), Expr::u32(1))),
                Node::let_bind("close_bit", Expr::shl(Expr::u32(1), Expr::var("depth"))),
                Node::assign(
                    "active_bits",
                    Expr::bitand(
                        Expr::var("active_bits"),
                        Expr::bitnot(Expr::var("close_bit")),
                    ),
                ),
                Node::assign(
                    "taken_bits",
                    Expr::bitand(
                        Expr::var("taken_bits"),
                        Expr::bitnot(Expr::var("close_bit")),
                    ),
                ),
            ],
        ),
    ]);

    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_buffer_count),
            BufferDecl::storage(directive_kinds, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_buffer_count),
            BufferDecl::storage(directive_values, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_buffer_count),
            BufferDecl::storage(out_mask, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(tok_buffer_count),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::opt_conditional_mask_with_directives",
            vec![child_phase(
                "vyre-libs::parsing::opt_conditional_mask_with_directives",
                vyre_primitives::text::line_index::OP_ID,
                vec![Node::if_then(
                    Expr::eq(t, Expr::u32(0)),
                    vec![
                        Node::let_bind("depth", Expr::u32(0)),
                        Node::let_bind("active_bits", Expr::u32(0)),
                        Node::let_bind("taken_bits", Expr::u32(0)),
                        Node::loop_for("i", Expr::u32(0), num_tokens, per_token),
                        Node::if_then(
                            Expr::ne(Expr::var("depth"), Expr::u32(0)),
                            vec![Node::trap(
                                Expr::var("depth"),
                                "c-preprocess-unclosed-conditional-directive",
                            )],
                        ),
                    ],
                )],
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::opt_conditional_mask_with_directives")
    .with_non_composable_with_self(true)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::opt_conditional_mask_with_directives",
        build: || opt_conditional_mask_with_directives(
            "tok_types", "directive_kinds", "directive_values", "out_mask", Expr::u32(3)
        ),
        test_inputs: Some(|| {
            vec![vec![
                vyre_primitives::wire::pack_u32_slice(&[
                    TOK_PREPROC,
                    TOK_IDENTIFIER,
                    TOK_PREPROC,
                ]),
                vyre_primitives::wire::pack_u32_slice(&[TOK_PP_IF, 0, TOK_PP_ENDIF]),
                vyre_primitives::wire::pack_u32_slice(&[0u32, 0, 0]),
                vec![0u8; 4 * 3],
            ]]
        }),
        expected_output: Some(|| {
            vec![vec![vyre_primitives::wire::pack_u32_slice(&[1u32, 0, 1])]]
        }),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::opt_dynamic_macro_expansion",
        build: || opt_dynamic_macro_expansion(
            "in_tok_types", "macro_keys", "macro_vals", "macro_sizes",
            "out_tok_types", "out_tok_counts", Expr::u32(4), 16
        ),
        test_inputs: Some(dynamic_macro_fixture_inputs),
        expected_output: Some(dynamic_macro_fixture_expected),
        category: Some("parsing"),
    }
}

fn dynamic_macro_slot(token: u32) -> usize {
    (token.wrapping_mul(2_654_435_769) & MACRO_TABLE_MASK) as usize
}

fn write_u32_at(dst: &mut [u8], idx: usize, value: u32) {
    let base = idx * 4;
    dst[base..base + 4].copy_from_slice(&value.to_le_bytes());
}

use crate::scan::dispatch_io::pack_u32_slice as pack_u32_words;

fn dynamic_macro_fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    let macro_token = 777u32;
    let replacement_start = 8usize;
    let slot = dynamic_macro_slot(macro_token);

    let mut keys = vec![0u8; 4 * MACRO_TABLE_SLOTS as usize];
    for idx in 0..MACRO_TABLE_SLOTS as usize {
        write_u32_at(&mut keys, idx, EMPTY_MACRO_SLOT);
    }
    write_u32_at(&mut keys, slot, macro_token);

    let mut vals = vec![0u8; 4 * MACRO_TABLE_SLOTS as usize];
    write_u32_at(&mut vals, slot, replacement_start as u32);
    write_u32_at(&mut vals, replacement_start, 10);
    write_u32_at(&mut vals, replacement_start + 1, 20);

    let mut sizes = vec![0u8; 4 * MACRO_TABLE_SLOTS as usize];
    write_u32_at(&mut sizes, replacement_start, 2);

    vec![vec![
        pack_u32_words(&[macro_token, 5, TOK_PREPROC, macro_token]),
        keys,
        vals,
        sizes,
        vec![0u8; 4 * 16],
        vec![0u8; 4],
    ]]
}

fn dynamic_macro_fixture_expected() -> Vec<Vec<Vec<u8>>> {
    let mut out = vec![0u8; 4 * 16];
    for (idx, word) in [10u32, 20, 5, TOK_PREPROC, 10, 20].into_iter().enumerate() {
        write_u32_at(&mut out, idx, word);
    }
    vec![vec![out, 6u32.to_le_bytes().to_vec()]]
}
