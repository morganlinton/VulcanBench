//! C11 digraph + line-splice resolution pass.

use crate::parsing::c::lex::tokens::*;
use crate::parsing::composition::child_phase;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::core::*;

/// Resolves C11 digraphs and line-splicing markers natively in the token stream.
/// Translates sequence pairs like `<` and `:` into `[` natively via parallel SIMT passes
/// without branching diverging divergence loops.
#[must_use]
pub fn c11_lex_digraphs(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    tok_count: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let logical_token_count = Expr::u32(tok_count);

    // Core transformation loop logic
    let transform_logic = vec![
        Node::let_bind("t1_type", Expr::load(tok_types, t.clone())),
        Node::let_bind("has_prev2", Expr::gt(t.clone(), Expr::u32(1))),
        Node::let_bind(
            "prev2_type",
            Expr::select(
                Expr::var("has_prev2"),
                Expr::load(tok_types, Expr::saturating_sub(t.clone(), Expr::u32(2))),
                Expr::u32(TOK_EOF),
            ),
        ),
        Node::let_bind(
            "prev1_type",
            Expr::select(
                Expr::gt(t.clone(), Expr::u32(0)),
                Expr::load(tok_types, Expr::saturating_sub(t.clone(), Expr::u32(1))),
                Expr::u32(TOK_EOF),
            ),
        ),
        // Boundary safety check for adjacent lookahead
        Node::if_then(
            Expr::lt(
                Expr::add(t.clone(), Expr::u32(1)),
                logical_token_count.clone(),
            ),
            vec![
                Node::let_bind(
                    "t2_type",
                    Expr::load(tok_types, Expr::add(t.clone(), Expr::u32(1))),
                ),
                Node::let_bind(
                    "t3_type",
                    Expr::select(
                        Expr::lt(
                            Expr::add(t.clone(), Expr::u32(2)),
                            logical_token_count.clone(),
                        ),
                        Expr::load(tok_types, Expr::add(t.clone(), Expr::u32(2))),
                        Expr::u32(TOK_EOF),
                    ),
                ),
                Node::let_bind(
                    "t4_type",
                    Expr::select(
                        Expr::lt(
                            Expr::add(t.clone(), Expr::u32(3)),
                            logical_token_count.clone(),
                        ),
                        Expr::load(tok_types, Expr::add(t.clone(), Expr::u32(3))),
                        Expr::u32(TOK_EOF),
                    ),
                ),
                Node::let_bind(
                    "is_percent_colon_percent_colon",
                    Expr::and(
                        Expr::and(
                            Expr::eq(Expr::var("t1_type"), Expr::u32(TOK_PERCENT)),
                            Expr::eq(Expr::var("t2_type"), Expr::u32(TOK_COLON)),
                        ),
                        Expr::and(
                            Expr::eq(Expr::var("t3_type"), Expr::u32(TOK_PERCENT)),
                            Expr::eq(Expr::var("t4_type"), Expr::u32(TOK_COLON)),
                        ),
                    ),
                ),
                Node::let_bind(
                    "inside_percent_colon_percent_colon_tail",
                    Expr::and(
                        Expr::var("has_prev2"),
                        Expr::and(
                            Expr::eq(Expr::var("prev2_type"), Expr::u32(TOK_PERCENT)),
                            Expr::eq(Expr::var("prev1_type"), Expr::u32(TOK_COLON)),
                        ),
                    ),
                ),
                Node::if_then(
                    Expr::var("is_percent_colon_percent_colon"),
                    vec![
                        Node::store(tok_types, t.clone(), Expr::u32(TOK_HASHHASH)),
                        Node::store(
                            tok_types,
                            Expr::add(t.clone(), Expr::u32(1)),
                            Expr::u32(TOK_COMMENT),
                        ),
                        Node::store(
                            tok_types,
                            Expr::add(t.clone(), Expr::u32(2)),
                            Expr::u32(TOK_COMMENT),
                        ),
                        Node::store(
                            tok_types,
                            Expr::add(t.clone(), Expr::u32(3)),
                            Expr::u32(TOK_COMMENT),
                        ),
                    ],
                ),
                // Match `<:` -> `[` (LBRACKET == 14)
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("t1_type"), Expr::u32(TOK_LT)),
                        Expr::eq(Expr::var("t2_type"), Expr::u32(TOK_COLON)),
                    ),
                    vec![
                        Node::store(tok_types, t.clone(), Expr::u32(TOK_LBRACKET)),
                        Node::store(
                            tok_types,
                            Expr::add(t.clone(), Expr::u32(1)),
                            Expr::u32(TOK_COMMENT),
                        ), // Erase the second component natively
                    ],
                ),
                // Match `:>` -> `]` (RBRACKET == 15)
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("t1_type"), Expr::u32(TOK_COLON)),
                        Expr::eq(Expr::var("t2_type"), Expr::u32(TOK_GT)),
                    ),
                    vec![
                        Node::store(tok_types, t.clone(), Expr::u32(TOK_RBRACKET)),
                        Node::store(
                            tok_types,
                            Expr::add(t.clone(), Expr::u32(1)),
                            Expr::u32(TOK_COMMENT),
                        ),
                    ],
                ),
                // Match `<%` -> `{` (LBRACE == 12)
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("t1_type"), Expr::u32(TOK_LT)),
                        Expr::eq(Expr::var("t2_type"), Expr::u32(TOK_PERCENT)),
                    ),
                    vec![
                        Node::store(tok_types, t.clone(), Expr::u32(TOK_LBRACE)),
                        Node::store(
                            tok_types,
                            Expr::add(t.clone(), Expr::u32(1)),
                            Expr::u32(TOK_COMMENT),
                        ),
                    ],
                ),
                // Match `%>` -> `}` (RBRACE == 13)
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("t1_type"), Expr::u32(TOK_PERCENT)),
                        Expr::eq(Expr::var("t2_type"), Expr::u32(TOK_GT)),
                    ),
                    vec![
                        Node::store(tok_types, t.clone(), Expr::u32(TOK_RBRACE)),
                        Node::store(
                            tok_types,
                            Expr::add(t.clone(), Expr::u32(1)),
                            Expr::u32(TOK_COMMENT),
                        ),
                    ],
                ),
                // Match `%:` -> `#` (HASH == 33)
                Node::if_then(
                    Expr::and(
                        Expr::and(
                            Expr::eq(Expr::var("t1_type"), Expr::u32(TOK_PERCENT)),
                            Expr::eq(Expr::var("t2_type"), Expr::u32(TOK_COLON)),
                        ),
                        Expr::and(
                            Expr::not(Expr::var("is_percent_colon_percent_colon")),
                            Expr::not(Expr::var("inside_percent_colon_percent_colon_tail")),
                        ),
                    ),
                    vec![
                        Node::store(tok_types, t.clone(), Expr::u32(TOK_HASH)),
                        Node::store(
                            tok_types,
                            Expr::add(t.clone(), Expr::u32(1)),
                            Expr::u32(TOK_COMMENT),
                        ),
                    ],
                ),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(tok_count.max(1)),
            BufferDecl::storage(tok_starts, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(tok_count.max(1)),
            BufferDecl::storage(tok_lens, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(tok_count.max(1)),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::c11_lex_digraphs",
            vec![child_phase(
                "vyre-libs::parsing::c11_lex_digraphs",
                vyre_primitives::text::utf8_validate::OP_ID,
                vec![Node::if_then(
                    Expr::lt(t.clone(), logical_token_count.clone()),
                    transform_logic,
                )],
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::c11_lex_digraphs")
    .with_non_composable_with_self(true)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::c_lexer",
        build: || {
            c11_lexer("haystack", "out_tok_types", "out_tok_starts", "out_tok_lens", "out_counts", 4096)
        },
        test_inputs: Some(lexer_bounded_identifier_inputs),
        expected_output: Some(lexer_bounded_identifier_expected),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::c11_lex_digraphs",
        build: || {
            c11_lex_digraphs("tok_types", "tok_starts", "tok_lens", 4096)
        },
        test_inputs: Some(digraph_inputs),
        expected_output: Some(digraph_expected),
        category: Some("parsing"),
    }
}

use crate::scan::dispatch_io::pack_u32_slice as pack_u32;

fn lexer_bounded_identifier_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![vec![
        vec![b'a'; 4_096 * 4],
        vec![0u8; 4_096 * 4],
        vec![0u8; 4_096 * 4],
        vec![0u8; 4_096 * 4],
        vec![0u8; 4],
    ]]
}

fn lexer_bounded_identifier_expected() -> Vec<Vec<Vec<u8>>> {
    let mut out_tok_types = vec![0u8; 4_096 * 4];
    out_tok_types[0..4].copy_from_slice(&TOK_IDENTIFIER.to_le_bytes());

    let mut out_tok_lens = vec![0u8; 4_096 * 4];
    out_tok_lens[0..4].copy_from_slice(&257u32.to_le_bytes());

    let mut out_counts = vec![0u8; 4];
    out_counts.copy_from_slice(&1u32.to_le_bytes());

    vec![vec![
        out_tok_types,
        vec![0u8; 4_096 * 4],
        out_tok_lens,
        out_counts,
    ]]
}

fn digraph_inputs() -> Vec<Vec<Vec<u8>>> {
    let mut tok_types = vec![0u32; 4096];
    tok_types[0..8].copy_from_slice(&[
        TOK_LT,
        TOK_COLON,
        TOK_LT,
        TOK_PERCENT,
        TOK_PERCENT,
        TOK_GT,
        TOK_PERCENT,
        TOK_COLON,
    ]);
    vec![vec![
        pack_u32(&tok_types),
        vec![0u8; 4 * 4096],
        vec![0u8; 4 * 4096],
    ]]
}

fn digraph_expected() -> Vec<Vec<Vec<u8>>> {
    let mut tok_types = vec![0u32; 4096];
    tok_types[0..8].copy_from_slice(&[
        TOK_LBRACKET,
        TOK_COMMENT,
        TOK_LBRACE,
        TOK_COMMENT,
        TOK_RBRACE,
        TOK_COMMENT,
        TOK_HASH,
        TOK_COMMENT,
    ]);
    vec![vec![
        pack_u32(&tok_types),
        vec![0u8; 4 * 4096],
        vec![0u8; 4 * 4096],
    ]]
}
