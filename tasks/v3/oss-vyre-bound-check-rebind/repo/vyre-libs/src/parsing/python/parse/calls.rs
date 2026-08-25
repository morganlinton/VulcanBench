use super::{
    find_matching_delimiter, load_u32, search_next_token, search_next_token_into, search_prev_token,
};
use crate::parsing::composition::child_phase;
use crate::parsing::python::lex::{
    TOK_AWAIT, TOK_DOT, TOK_EQ, TOK_IDENTIFIER, TOK_LPAREN, TOK_NUMBER, TOK_RPAREN,
};
use crate::parsing::python::{
    CALL_RECORD_WORDS, INVALID_POS, KWARG_RECORD_WORDS, MAX_DOTTED_SEGMENTS,
};
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn store_words(buffer: &str, base_var: &str, words: &[Expr]) -> Vec<Node> {
    words
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            Node::store(
                buffer,
                Expr::add(Expr::var(base_var), Expr::u32(idx as u32)),
                value.clone(),
            )
        })
        .collect()
}

/// Extract Python call sites plus top-level keyword arguments.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn python312_extract_calls(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    out_calls: &str,
    out_call_counts: &str,
    out_kwargs: &str,
    out_kw_counts: &str,
    haystack_len: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let mut body = vec![
        Node::let_bind("tok", load_u32(tok_types, t.clone())),
        Node::let_bind("emit", Expr::u32(0)),
        // Hoist `name_end`, `cursor`, `dot_pos`, `after_dot` to the
        // outer body so they outlive the if_then block that assigns
        // them and remain in scope for the post-if_then `search_next_token`
        // call that reads `name_end`.
        Node::let_bind("is_call_head", Expr::u32(0)),
        Node::let_bind("name_end", t.clone()),
        Node::let_bind("cursor", t.clone()),
        Node::let_bind("dot_pos", Expr::u32(INVALID_POS)),
        Node::let_bind("after_dot", Expr::u32(INVALID_POS)),
    ];
    body.extend(search_prev_token("prev_tok", t.clone(), tok_types));
    body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::var("tok"), Expr::u32(TOK_IDENTIFIER)),
            Expr::ne(
                load_u32(tok_types, Expr::var("prev_tok")),
                Expr::u32(TOK_DOT),
            ),
        ),
        vec![
            Node::assign("is_call_head", Expr::u32(1)),
            Node::loop_for(
                "seg",
                Expr::u32(0),
                Expr::u32(MAX_DOTTED_SEGMENTS),
                vec![
                    Node::assign("dot_pos", Expr::u32(INVALID_POS)),
                    Node::assign("after_dot", Expr::u32(INVALID_POS)),
                    Node::if_then(
                        Expr::ne(Expr::var("cursor"), Expr::u32(INVALID_POS)),
                        search_next_token_into(
                            "dot_pos",
                            Expr::add(Expr::var("cursor"), Expr::u32(1)),
                            tok_types,
                            haystack_len,
                        ),
                    ),
                    Node::if_then(
                        Expr::eq(
                            load_u32(tok_types, Expr::var("dot_pos")),
                            Expr::u32(TOK_DOT),
                        ),
                        search_next_token_into(
                            "after_dot",
                            Expr::add(Expr::var("dot_pos"), Expr::u32(1)),
                            tok_types,
                            haystack_len,
                        ),
                    ),
                    Node::if_then(
                        Expr::eq(
                            load_u32(tok_types, Expr::var("after_dot")),
                            Expr::u32(TOK_IDENTIFIER),
                        ),
                        vec![
                            Node::assign("name_end", Expr::var("after_dot")),
                            Node::assign("cursor", Expr::var("after_dot")),
                        ],
                    ),
                    Node::if_then(
                        Expr::ne(
                            load_u32(tok_types, Expr::var("after_dot")),
                            Expr::u32(TOK_IDENTIFIER),
                        ),
                        vec![Node::assign("cursor", Expr::u32(INVALID_POS))],
                    ),
                ],
            ),
        ],
    ));
    body.extend(search_next_token(
        "after_name",
        Expr::add(Expr::var("name_end"), Expr::u32(1)),
        tok_types,
        haystack_len,
    ));
    body.extend(find_matching_delimiter(
        "rparen",
        Expr::var("after_name"),
        tok_types,
        haystack_len,
        TOK_LPAREN,
        TOK_RPAREN,
    ));
    body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::var("is_call_head"), Expr::u32(1)),
            Expr::and(
                Expr::eq(
                    load_u32(tok_types, Expr::var("after_name")),
                    Expr::u32(TOK_LPAREN),
                ),
                Expr::ne(Expr::var("rparen"), Expr::u32(INVALID_POS)),
            ),
        ),
        vec![Node::assign("emit", Expr::u32(1))],
    ));
    body.push(Node::if_then(
        Expr::eq(Expr::var("emit"), Expr::u32(1)),
        vec![
            Node::let_bind("kw_base", Expr::load(out_kw_counts, Expr::u32(0))),
            Node::let_bind("kw_count", Expr::u32(0)),
            Node::let_bind("paren_depth", Expr::u32(0)),
            Node::let_bind("bracket_depth", Expr::u32(0)),
            Node::loop_for(
                "scan",
                Expr::add(Expr::var("after_name"), Expr::u32(1)),
                Expr::var("rparen"),
                vec![
                    Node::let_bind("scan_tok", load_u32(tok_types, Expr::var("scan"))),
                    Node::if_then(
                        Expr::eq(Expr::var("scan_tok"), Expr::u32(TOK_LPAREN)),
                        vec![Node::assign(
                            "paren_depth",
                            Expr::add(Expr::var("paren_depth"), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var("scan_tok"), Expr::u32(TOK_RPAREN)),
                        vec![Node::if_then(
                            Expr::gt(Expr::var("paren_depth"), Expr::u32(0)),
                            vec![Node::assign(
                                "paren_depth",
                                Expr::sub(Expr::var("paren_depth"), Expr::u32(1)),
                            )],
                        )],
                    ),
                    Node::if_then(
                        Expr::eq(
                            Expr::var("scan_tok"),
                            Expr::u32(crate::parsing::python::lex::TOK_LBRACKET),
                        ),
                        vec![Node::assign(
                            "bracket_depth",
                            Expr::add(Expr::var("bracket_depth"), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::eq(
                            Expr::var("scan_tok"),
                            Expr::u32(crate::parsing::python::lex::TOK_RBRACKET),
                        ),
                        vec![Node::if_then(
                            Expr::gt(Expr::var("bracket_depth"), Expr::u32(0)),
                            vec![Node::assign(
                                "bracket_depth",
                                Expr::sub(Expr::var("bracket_depth"), Expr::u32(1)),
                            )],
                        )],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::and(
                                Expr::eq(Expr::var("scan_tok"), Expr::u32(TOK_IDENTIFIER)),
                                Expr::eq(Expr::var("paren_depth"), Expr::u32(0)),
                            ),
                            Expr::eq(Expr::var("bracket_depth"), Expr::u32(0)),
                        ),
                        // Drop the explicit `Node::let_bind` siblings  -
                        // `search_next_token` / `search_prev_token` each
                        // emit their own outer let_bind, so the manual
                        // ones here were duplicate-sibling V032 errors.
                        search_next_token(
                            "kw_eq_pos",
                            Expr::add(Expr::var("scan"), Expr::u32(1)),
                            tok_types,
                            haystack_len,
                        )
                        .into_iter()
                        .chain(search_prev_token("kw_prev", Expr::var("scan"), tok_types))
                        .chain(vec![Node::if_then(
                            Expr::and(
                                Expr::eq(
                                    load_u32(tok_types, Expr::var("kw_eq_pos")),
                                    Expr::u32(TOK_EQ),
                                ),
                                Expr::ne(
                                    load_u32(tok_types, Expr::var("kw_prev")),
                                    Expr::u32(TOK_DOT),
                                ),
                            ),
                            vec![
                                Node::let_bind(
                                    "kw_slot",
                                    Expr::atomic_add(
                                        out_kw_counts,
                                        Expr::u32(0),
                                        Expr::u32(KWARG_RECORD_WORDS),
                                    ),
                                ),
                                Node::store(
                                    out_kwargs,
                                    Expr::var("kw_slot"),
                                    load_u32(tok_starts, Expr::var("scan")),
                                ),
                                Node::store(
                                    out_kwargs,
                                    Expr::add(Expr::var("kw_slot"), Expr::u32(1)),
                                    load_u32(tok_lens, Expr::var("scan")),
                                ),
                                Node::assign(
                                    "kw_count",
                                    Expr::add(Expr::var("kw_count"), Expr::u32(1)),
                                ),
                            ],
                        )])
                        .collect(),
                    ),
                ],
            ),
            Node::let_bind(
                "call_slot",
                Expr::atomic_add(out_call_counts, Expr::u32(0), Expr::u32(CALL_RECORD_WORDS)),
            ),
        ]
        .into_iter()
        .chain(store_words(
            out_calls,
            "call_slot",
            &[
                load_u32(tok_starts, t.clone()),
                Expr::add(
                    Expr::sub(
                        load_u32(tok_starts, Expr::var("name_end")),
                        load_u32(tok_starts, t.clone()),
                    ),
                    load_u32(tok_lens, Expr::var("name_end")),
                ),
                Expr::var("after_name"),
                Expr::var("rparen"),
                Expr::var("kw_base"),
                Expr::var("kw_count"),
                Expr::select(
                    Expr::eq(
                        load_u32(tok_types, Expr::var("prev_tok")),
                        Expr::u32(TOK_AWAIT),
                    ),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            ],
        ))
        .collect(),
    ));

    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(haystack_len),
            BufferDecl::storage(tok_starts, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(haystack_len),
            BufferDecl::storage(tok_lens, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(haystack_len),
            BufferDecl::storage(out_calls, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(haystack_len.saturating_mul(CALL_RECORD_WORDS)),
            BufferDecl::storage(out_call_counts, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
            BufferDecl::storage(out_kwargs, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(haystack_len.saturating_mul(KWARG_RECORD_WORDS)),
            BufferDecl::storage(out_kw_counts, 6, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::python312_extract_calls",
            vec![child_phase(
                "vyre-libs::parsing::python312_extract_calls",
                vyre_primitives::parsing::core_delimiter_match::OP_ID,
                vec![Node::if_then(
                    Expr::lt(t.clone(), Expr::u32(haystack_len)),
                    body,
                )],
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::python312_extract_calls")
    .with_non_composable_with_self(true)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::python312_extract_calls",
        build: || python312_extract_calls(
            "tok_types", "tok_starts", "tok_lens", "out_calls", "out_call_counts", "out_kwargs", "out_kw_counts", 16
        ),
        test_inputs: Some(call_fixture_inputs),
        expected_output: Some(call_fixture_expected),
        category: Some("parsing"),
    }
}

fn call_fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    let mut tok_types = vec![0u8; 16 * 4];
    let mut tok_starts = vec![0u8; 16 * 4];
    let mut tok_lens = vec![0u8; 16 * 4];
    for (pos, tok, len) in [
        (0usize, TOK_AWAIT, 5u32),
        (6, TOK_IDENTIFIER, 3),
        (9, TOK_LPAREN, 1),
        (10, TOK_IDENTIFIER, 1),
        (11, TOK_EQ, 1),
        (12, TOK_NUMBER, 1),
        (13, TOK_RPAREN, 1),
    ] {
        let base = pos * 4;
        tok_types[base..base + 4].copy_from_slice(&tok.to_le_bytes());
        tok_starts[base..base + 4].copy_from_slice(&(pos as u32).to_le_bytes());
        tok_lens[base..base + 4].copy_from_slice(&len.to_le_bytes());
    }

    vec![vec![
        tok_types,
        tok_starts,
        tok_lens,
        vec![0u8; 16 * CALL_RECORD_WORDS as usize * 4],
        vec![0u8; 4],
        vec![0u8; 16 * KWARG_RECORD_WORDS as usize * 4],
        vec![0u8; 4],
    ]]
}

fn write_words(dst: &mut [u8], words: &[u32]) {
    for (idx, word) in words.iter().enumerate() {
        let base = idx * 4;
        dst[base..base + 4].copy_from_slice(&word.to_le_bytes());
    }
}

fn call_fixture_expected() -> Vec<Vec<Vec<u8>>> {
    let mut calls = vec![0u8; 16 * CALL_RECORD_WORDS as usize * 4];
    write_words(&mut calls, &[6, 3, 9, 13, 0, 1, 1]);

    let mut kwargs = vec![0u8; 16 * KWARG_RECORD_WORDS as usize * 4];
    write_words(&mut kwargs, &[10, 1]);

    vec![vec![
        calls,
        CALL_RECORD_WORDS.to_le_bytes().to_vec(),
        kwargs,
        KWARG_RECORD_WORDS.to_le_bytes().to_vec(),
    ]]
}
