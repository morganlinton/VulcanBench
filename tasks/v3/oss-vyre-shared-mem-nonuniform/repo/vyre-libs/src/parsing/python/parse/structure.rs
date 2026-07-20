use super::{
    find_matching_delimiter, find_matching_delimiter_into, load_u32, search_next_token,
    search_next_token_into, search_prev_token,
};
use crate::parsing::composition::child_phase;
use crate::parsing::python::lex::{
    TOK_ASYNC, TOK_CLASS, TOK_COLON, TOK_COMMA, TOK_DEF, TOK_DOT, TOK_FROM, TOK_IDENTIFIER,
    TOK_IMPORT, TOK_LBRACKET, TOK_LPAREN, TOK_RBRACKET, TOK_WITH,
};
use crate::parsing::python::{
    DEF_RECORD_WORDS, IMPORT_RECORD_WORDS, INVALID_POS, WITH_RECORD_WORDS,
};
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn store_record_words(out_buffer: &str, base_var: &str, values: &[Expr]) -> Vec<Node> {
    values
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            Node::store(
                out_buffer,
                Expr::add(Expr::var(base_var), Expr::u32(idx as u32)),
                value.clone(),
            )
        })
        .collect()
}

/// Extract `def`, `async def`, and `class` declarations.
#[must_use]
pub fn python312_extract_structure(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    out_records: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let mut body = vec![
        Node::let_bind("tok", load_u32(tok_types, t.clone())),
        Node::let_bind("emit_kind", Expr::u32(0)),
        Node::let_bind("keyword_pos", Expr::u32(INVALID_POS)),
        Node::if_then(
            Expr::eq(Expr::var("tok"), Expr::u32(TOK_DEF)),
            vec![
                Node::assign("emit_kind", Expr::u32(1)),
                Node::assign("keyword_pos", t.clone()),
            ],
        ),
        Node::if_then(
            Expr::eq(Expr::var("tok"), Expr::u32(TOK_CLASS)),
            vec![
                Node::assign("emit_kind", Expr::u32(3)),
                Node::assign("keyword_pos", t.clone()),
            ],
        ),
    ];
    body.extend(search_next_token(
        "async_next",
        Expr::add(t.clone(), Expr::u32(1)),
        tok_types,
        haystack_len,
    ));
    body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::var("tok"), Expr::u32(TOK_ASYNC)),
            Expr::eq(
                load_u32(tok_types, Expr::var("async_next")),
                Expr::u32(TOK_DEF),
            ),
        ),
        vec![
            Node::assign("emit_kind", Expr::u32(2)),
            Node::assign("keyword_pos", Expr::var("async_next")),
        ],
    ));
    body.extend(search_next_token(
        "name_pos",
        Expr::add(Expr::var("keyword_pos"), Expr::u32(1)),
        tok_types,
        haystack_len,
    ));
    body.extend(search_next_token(
        "post_name",
        Expr::add(Expr::var("name_pos"), Expr::u32(1)),
        tok_types,
        haystack_len,
    ));
    body.extend(find_matching_delimiter(
        "type_params_end",
        Expr::var("post_name"),
        tok_types,
        haystack_len,
        TOK_LBRACKET,
        TOK_RBRACKET,
    ));
    body.push(Node::if_then(
        Expr::and(
            Expr::ne(Expr::var("emit_kind"), Expr::u32(0)),
            Expr::eq(
                load_u32(tok_types, Expr::var("name_pos")),
                Expr::u32(TOK_IDENTIFIER),
            ),
        ),
        vec![
            Node::let_bind("params_start", Expr::u32(INVALID_POS)),
            Node::let_bind("params_end", Expr::u32(INVALID_POS)),
            Node::let_bind("colon_pos", Expr::u32(INVALID_POS)),
            // Hoist `after_type_params` and `after_params` to the
            // outer scope so the if-block bodies (which assign them)
            // and the later if-blocks (which read them) share one
            // binding. Pre-T-V2 the per-branch `Node::let_bind` lived
            // inside each block, the validator scoped the binding to
            // the block, and the read sites failed with "reference to
            // undeclared variable `after_type_params`" / `after_params`.
            Node::let_bind("after_type_params", Expr::u32(INVALID_POS)),
            Node::let_bind("after_params", Expr::u32(INVALID_POS)),
            Node::if_then_else(
                Expr::eq(
                    load_u32(tok_types, Expr::var("post_name")),
                    Expr::u32(TOK_LBRACKET),
                ),
                search_next_token_into(
                    "after_type_params",
                    Expr::add(Expr::var("type_params_end"), Expr::u32(1)),
                    tok_types,
                    haystack_len,
                ),
                vec![Node::assign("after_type_params", Expr::var("post_name"))],
            ),
            Node::if_then(
                Expr::eq(
                    load_u32(tok_types, Expr::var("after_type_params")),
                    Expr::u32(TOK_LPAREN),
                ),
                vec![
                    Node::assign("params_start", Expr::var("after_type_params")),
                    Node::assign("params_end", Expr::u32(INVALID_POS)),
                ]
                .into_iter()
                .chain(find_matching_delimiter_into(
                    "params_end",
                    Expr::var("after_type_params"),
                    tok_types,
                    haystack_len,
                    TOK_LPAREN,
                    crate::parsing::python::lex::TOK_RPAREN,
                ))
                .collect(),
            ),
            Node::if_then_else(
                Expr::ne(Expr::var("params_end"), Expr::u32(INVALID_POS)),
                search_next_token_into(
                    "after_params",
                    Expr::add(Expr::var("params_end"), Expr::u32(1)),
                    tok_types,
                    haystack_len,
                ),
                vec![Node::assign("after_params", Expr::var("after_type_params"))],
            ),
            Node::if_then(
                Expr::eq(
                    load_u32(tok_types, Expr::var("after_params")),
                    Expr::u32(TOK_COLON),
                ),
                vec![Node::assign("colon_pos", Expr::var("after_params"))],
            ),
            Node::let_bind(
                "slot",
                Expr::atomic_add(out_counts, Expr::u32(0), Expr::u32(DEF_RECORD_WORDS)),
            ),
        ]
        .into_iter()
        .chain(store_record_words(
            out_records,
            "slot",
            &[
                Expr::var("emit_kind"),
                load_u32(tok_starts, Expr::var("name_pos")),
                load_u32(tok_lens, Expr::var("name_pos")),
                Expr::var("params_start"),
                Expr::var("params_end"),
                Expr::var("colon_pos"),
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
            BufferDecl::storage(out_records, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(haystack_len.saturating_mul(DEF_RECORD_WORDS)),
            BufferDecl::storage(out_counts, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::python312_extract_structure",
            vec![child_phase(
                "vyre-libs::parsing::python312_extract_structure",
                vyre_primitives::text::line_index::OP_ID,
                vec![Node::if_then(
                    Expr::lt(t.clone(), Expr::u32(haystack_len)),
                    body,
                )],
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::python312_extract_structure")
    .with_non_composable_with_self(true)
}

/// Extract `import` and `from ... import ...` statements.
#[must_use]
pub fn python312_extract_imports(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    out_records: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let mut body = vec![
        Node::let_bind("tok", load_u32(tok_types, t.clone())),
        Node::let_bind("record_kind", Expr::u32(0)),
    ];
    body.extend(search_prev_token("prev_tok", t.clone(), tok_types));
    body.extend(search_next_token(
        "next_tok",
        Expr::add(t.clone(), Expr::u32(1)),
        tok_types,
        haystack_len,
    ));
    body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::var("tok"), Expr::u32(TOK_IDENTIFIER)),
            Expr::or(
                Expr::eq(
                    load_u32(tok_types, Expr::var("prev_tok")),
                    Expr::u32(TOK_IMPORT),
                ),
                Expr::eq(
                    load_u32(tok_types, Expr::var("prev_tok")),
                    Expr::u32(TOK_FROM),
                ),
            ),
        ),
        vec![Node::assign(
            "record_kind",
            Expr::select(
                Expr::eq(
                    load_u32(tok_types, Expr::var("prev_tok")),
                    Expr::u32(TOK_IMPORT),
                ),
                Expr::u32(1),
                Expr::u32(2),
            ),
        )],
    ));
    body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::var("tok"), Expr::u32(TOK_IDENTIFIER)),
            Expr::eq(
                load_u32(tok_types, Expr::var("prev_tok")),
                Expr::u32(TOK_COMMA),
            ),
        ),
        vec![Node::assign("record_kind", Expr::u32(1))],
    ));
    body.push(Node::if_then(
        Expr::ne(Expr::var("record_kind"), Expr::u32(0)),
        vec![
            Node::let_bind("name_end", t.clone()),
            Node::let_bind("cursor", t.clone()),
            Node::let_bind("dot_pos", Expr::u32(INVALID_POS)),
            Node::let_bind("after_dot", Expr::u32(INVALID_POS)),
            Node::loop_for(
                "seg",
                Expr::u32(0),
                Expr::u32(crate::parsing::python::MAX_DOTTED_SEGMENTS),
                vec![
                    // Reset per iteration via assign  -  the outer
                    // let_bind lives BEFORE the loop_for so the
                    // validator doesn't see a re-declaration each
                    // pass (V008). search_next_token_into is the
                    // assign-only variant for the same reason.
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
            Node::let_bind(
                "slot",
                Expr::atomic_add(out_counts, Expr::u32(0), Expr::u32(IMPORT_RECORD_WORDS)),
            ),
        ]
        .into_iter()
        .chain(store_record_words(
            out_records,
            "slot",
            &[
                Expr::var("record_kind"),
                load_u32(tok_starts, t.clone()),
                Expr::add(
                    Expr::sub(
                        load_u32(tok_starts, Expr::var("name_end")),
                        load_u32(tok_starts, t.clone()),
                    ),
                    load_u32(tok_lens, Expr::var("name_end")),
                ),
                Expr::var("prev_tok"),
                Expr::var("name_end"),
                Expr::var("next_tok"),
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
            BufferDecl::storage(out_records, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(haystack_len.saturating_mul(IMPORT_RECORD_WORDS)),
            BufferDecl::storage(out_counts, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::python312_extract_imports",
            vec![child_phase(
                "vyre-libs::parsing::python312_extract_imports",
                vyre_primitives::text::line_index::OP_ID,
                vec![Node::if_then(
                    Expr::lt(t.clone(), Expr::u32(haystack_len)),
                    body,
                )],
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::python312_extract_imports")
    .with_non_composable_with_self(true)
}

/// Extract `with` / `async with` headers.
#[must_use]
pub fn python312_extract_with_blocks(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    out_records: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let mut body = vec![
        Node::let_bind("tok", load_u32(tok_types, t.clone())),
        Node::let_bind("with_pos", Expr::u32(INVALID_POS)),
        Node::let_bind("flags", Expr::u32(0)),
    ];
    body.extend(search_prev_token("prev_tok", t.clone(), tok_types));
    body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::var("tok"), Expr::u32(TOK_WITH)),
            Expr::ne(
                load_u32(tok_types, Expr::var("prev_tok")),
                Expr::u32(TOK_ASYNC),
            ),
        ),
        vec![Node::assign("with_pos", t.clone())],
    ));
    body.extend(search_next_token(
        "async_next",
        Expr::add(t.clone(), Expr::u32(1)),
        tok_types,
        haystack_len,
    ));
    body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::var("tok"), Expr::u32(TOK_ASYNC)),
            Expr::eq(
                load_u32(tok_types, Expr::var("async_next")),
                Expr::u32(TOK_WITH),
            ),
        ),
        vec![
            Node::assign("with_pos", Expr::var("async_next")),
            Node::assign("flags", Expr::u32(1)),
        ],
    ));
    body.extend(search_next_token(
        "manager_pos",
        Expr::add(Expr::var("with_pos"), Expr::u32(1)),
        tok_types,
        haystack_len,
    ));
    body.extend(search_next_token(
        "after_manager",
        Expr::add(Expr::var("manager_pos"), Expr::u32(1)),
        tok_types,
        haystack_len,
    ));
    body.push(Node::if_then(
        Expr::and(
            Expr::ne(Expr::var("with_pos"), Expr::u32(INVALID_POS)),
            Expr::eq(
                load_u32(tok_types, Expr::var("manager_pos")),
                Expr::u32(TOK_IDENTIFIER),
            ),
        ),
        vec![
            Node::let_bind("manager_end", Expr::var("manager_pos")),
            Node::let_bind("cursor", Expr::var("manager_pos")),
            Node::let_bind("dot_pos", Expr::u32(INVALID_POS)),
            Node::let_bind("after_dot", Expr::u32(INVALID_POS)),
            Node::loop_for(
                "seg",
                Expr::u32(0),
                Expr::u32(crate::parsing::python::MAX_DOTTED_SEGMENTS),
                vec![
                    // Reset per iteration via assign  -  the outer
                    // let_bind lives BEFORE the loop_for so the
                    // validator doesn't see a re-declaration each
                    // pass (V008). search_next_token_into is the
                    // assign-only variant for the same reason.
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
                            Node::assign("manager_end", Expr::var("after_dot")),
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
            Node::let_bind("colon_pos", Expr::u32(INVALID_POS)),
            Node::loop_for(
                "scan",
                Expr::add(Expr::var("manager_end"), Expr::u32(1)),
                Expr::u32(haystack_len),
                vec![Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("colon_pos"), Expr::u32(INVALID_POS)),
                        Expr::eq(load_u32(tok_types, Expr::var("scan")), Expr::u32(TOK_COLON)),
                    ),
                    vec![Node::assign("colon_pos", Expr::var("scan"))],
                )],
            ),
            Node::let_bind(
                "slot",
                Expr::atomic_add(out_counts, Expr::u32(0), Expr::u32(WITH_RECORD_WORDS)),
            ),
        ]
        .into_iter()
        .chain(store_record_words(
            out_records,
            "slot",
            &[
                load_u32(tok_starts, Expr::var("manager_pos")),
                Expr::add(
                    Expr::sub(
                        load_u32(tok_starts, Expr::var("manager_end")),
                        load_u32(tok_starts, Expr::var("manager_pos")),
                    ),
                    load_u32(tok_lens, Expr::var("manager_end")),
                ),
                Expr::var("with_pos"),
                Expr::var("colon_pos"),
                Expr::var("flags"),
                Expr::u32(0),
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
            BufferDecl::storage(out_records, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(haystack_len.saturating_mul(WITH_RECORD_WORDS)),
            BufferDecl::storage(out_counts, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::python312_extract_with_blocks",
            vec![child_phase(
                "vyre-libs::parsing::python312_extract_with_blocks",
                vyre_primitives::text::line_index::OP_ID,
                vec![Node::if_then(
                    Expr::lt(t.clone(), Expr::u32(haystack_len)),
                    body,
                )],
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::python312_extract_with_blocks")
    .with_non_composable_with_self(true)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::python312_extract_structure",
        build: || python312_extract_structure("tok_types", "tok_starts", "tok_lens", "out_records", "out_counts", 16),
        test_inputs: Some(structure_fixture_inputs),
        expected_output: Some(structure_fixture_expected),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::python312_extract_imports",
        build: || python312_extract_imports("tok_types", "tok_starts", "tok_lens", "out_records", "out_counts", 16),
        test_inputs: Some(import_fixture_inputs),
        expected_output: Some(import_fixture_expected),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::python312_extract_with_blocks",
        build: || python312_extract_with_blocks("tok_types", "tok_starts", "tok_lens", "out_records", "out_counts", 16),
        test_inputs: Some(with_fixture_inputs),
        expected_output: Some(with_fixture_expected),
        category: Some("parsing"),
    }
}

fn pack_sparse_tokens(tokens: &[(usize, u32, u32)]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut tok_types = vec![0u8; 16 * 4];
    let mut tok_starts = vec![0u8; 16 * 4];
    let mut tok_lens = vec![0u8; 16 * 4];
    for &(pos, tok, len) in tokens {
        let base = pos * 4;
        tok_types[base..base + 4].copy_from_slice(&tok.to_le_bytes());
        tok_starts[base..base + 4].copy_from_slice(&(pos as u32).to_le_bytes());
        tok_lens[base..base + 4].copy_from_slice(&len.to_le_bytes());
    }
    (tok_types, tok_starts, tok_lens)
}

fn write_words(dst: &mut [u8], words: &[u32]) {
    for (idx, word) in words.iter().enumerate() {
        let base = idx * 4;
        dst[base..base + 4].copy_from_slice(&word.to_le_bytes());
    }
}

fn structure_fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    let (tok_types, tok_starts, tok_lens) = pack_sparse_tokens(&[
        (0, TOK_DEF, 3),
        (4, TOK_IDENTIFIER, 1),
        (5, TOK_LPAREN, 1),
        (6, crate::parsing::python::lex::TOK_RPAREN, 1),
        (7, TOK_COLON, 1),
    ]);
    vec![vec![
        tok_types,
        tok_starts,
        tok_lens,
        vec![0u8; 16 * DEF_RECORD_WORDS as usize * 4],
        vec![0u8; 4],
    ]]
}

fn structure_fixture_expected() -> Vec<Vec<Vec<u8>>> {
    let mut records = vec![0u8; 16 * DEF_RECORD_WORDS as usize * 4];
    write_words(&mut records, &[1, 4, 1, 5, 6, 7]);
    vec![vec![records, DEF_RECORD_WORDS.to_le_bytes().to_vec()]]
}

fn import_fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    let (tok_types, tok_starts, tok_lens) =
        pack_sparse_tokens(&[(0, TOK_IMPORT, 6), (7, TOK_IDENTIFIER, 2)]);
    vec![vec![
        tok_types,
        tok_starts,
        tok_lens,
        vec![0u8; 16 * IMPORT_RECORD_WORDS as usize * 4],
        vec![0u8; 4],
    ]]
}

fn import_fixture_expected() -> Vec<Vec<Vec<u8>>> {
    let mut records = vec![0u8; 16 * IMPORT_RECORD_WORDS as usize * 4];
    write_words(
        &mut records,
        &[1, 7, 2, 0, 7, crate::parsing::python::INVALID_POS],
    );
    vec![vec![records, IMPORT_RECORD_WORDS.to_le_bytes().to_vec()]]
}

fn with_fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    let (tok_types, tok_starts, tok_lens) = pack_sparse_tokens(&[
        (0, TOK_ASYNC, 5),
        (6, TOK_WITH, 4),
        (11, TOK_IDENTIFIER, 3),
        (14, TOK_COLON, 1),
    ]);
    vec![vec![
        tok_types,
        tok_starts,
        tok_lens,
        vec![0u8; 16 * WITH_RECORD_WORDS as usize * 4],
        vec![0u8; 4],
    ]]
}

fn with_fixture_expected() -> Vec<Vec<Vec<u8>>> {
    let mut records = vec![0u8; 16 * WITH_RECORD_WORDS as usize * 4];
    write_words(&mut records, &[11, 3, 6, 14, 1, 0]);
    vec![vec![records, WITH_RECORD_WORDS.to_le_bytes().to_vec()]]
}
