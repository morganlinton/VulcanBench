use super::*;

pub fn c11_annotate_typedef_names(
    vast_nodes: &str,
    haystack: &str,
    haystack_len: Expr,
    num_nodes: Expr,
    out_annotated_vast_nodes: &str,
) -> Program {
    c11_annotate_typedef_names_impl(
        vast_nodes,
        haystack,
        haystack_len,
        num_nodes,
        out_annotated_vast_nodes,
        false,
        false,
        None,
    )
}

pub fn c11_annotate_typedef_names_packed_haystack(
    vast_nodes: &str,
    haystack: &str,
    haystack_len: Expr,
    num_nodes: Expr,
    out_annotated_vast_nodes: &str,
) -> Program {
    c11_annotate_typedef_names_impl(
        vast_nodes,
        haystack,
        haystack_len,
        num_nodes,
        out_annotated_vast_nodes,
        true,
        false,
        None,
    )
}

pub fn c11_annotate_typedef_names_precomputed_scope(
    vast_nodes: &str,
    haystack: &str,
    haystack_len: Expr,
    num_nodes: Expr,
    out_annotated_vast_nodes: &str,
) -> Program {
    c11_annotate_typedef_names_impl(
        vast_nodes,
        haystack,
        haystack_len,
        num_nodes,
        out_annotated_vast_nodes,
        false,
        true,
        None,
    )
}

pub fn c11_annotate_typedef_names_precomputed_scope_packed_haystack(
    vast_nodes: &str,
    haystack: &str,
    haystack_len: Expr,
    num_nodes: Expr,
    out_annotated_vast_nodes: &str,
) -> Program {
    c11_annotate_typedef_names_impl(
        vast_nodes,
        haystack,
        haystack_len,
        num_nodes,
        out_annotated_vast_nodes,
        true,
        true,
        None,
    )
}

pub fn c11_annotate_typedef_names_precomputed_context(
    vast_nodes: &str,
    haystack: &str,
    decl_contexts: &str,
    haystack_len: Expr,
    num_nodes: Expr,
    out_annotated_vast_nodes: &str,
) -> Program {
    c11_annotate_typedef_names_impl(
        vast_nodes,
        haystack,
        haystack_len,
        num_nodes,
        out_annotated_vast_nodes,
        false,
        true,
        Some(decl_contexts),
    )
}

pub fn c11_annotate_typedef_names_precomputed_context_packed_haystack(
    vast_nodes: &str,
    haystack: &str,
    decl_contexts: &str,
    haystack_len: Expr,
    num_nodes: Expr,
    out_annotated_vast_nodes: &str,
) -> Program {
    c11_annotate_typedef_names_impl(
        vast_nodes,
        haystack,
        haystack_len,
        num_nodes,
        out_annotated_vast_nodes,
        true,
        true,
        Some(decl_contexts),
    )
}

pub(super) fn c11_annotate_typedef_names_impl(
    vast_nodes: &str,
    haystack: &str,
    haystack_len: Expr,
    num_nodes: Expr,
    out_annotated_vast_nodes: &str,
    packed_haystack: bool,
    precomputed_scope: bool,
    decl_contexts: Option<&str>,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let base = Expr::mul(t.clone(), Expr::u32(VAST_NODE_STRIDE_U32));

    let mut loop_body = vec![
        Node::let_bind("raw_kind", Expr::load(vast_nodes, base.clone())),
        Node::let_bind(
            "tok_start",
            Expr::load(vast_nodes, Expr::add(base.clone(), Expr::u32(5))),
        ),
        Node::let_bind(
            "tok_len",
            Expr::load(vast_nodes, Expr::add(base.clone(), Expr::u32(6))),
        ),
        Node::let_bind(
            "name_hash",
            Expr::load(
                vast_nodes,
                Expr::add(base.clone(), Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD)),
            ),
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
                Expr::eq(Expr::var("name_hash"), Expr::u32(0)),
            ),
            vec![
                Node::assign("name_hash", Expr::u32(0x811c9dc5)),
                Node::loop_for(
                    "hash_i",
                    Expr::u32(0),
                    Expr::var("tok_len"),
                    vec![Node::if_then(
                        Expr::lt(
                            Expr::add(Expr::var("tok_start"), Expr::var("hash_i")),
                            haystack_len.clone(),
                        ),
                        vec![
                            Node::let_bind(
                                "hash_byte",
                                load_source_byte(
                                    haystack,
                                    Expr::add(Expr::var("tok_start"), Expr::var("hash_i")),
                                    packed_haystack,
                                ),
                            ),
                            Node::assign(
                                "name_hash",
                                Expr::bitxor(Expr::var("name_hash"), Expr::var("hash_byte")),
                            ),
                            Node::assign(
                                "name_hash",
                                Expr::mul(Expr::var("name_hash"), Expr::u32(0x01000193)),
                            ),
                        ],
                    )],
                ),
            ],
        ),
        Node::let_bind(
            "scope_open",
            if precomputed_scope {
                Expr::load(
                    vast_nodes,
                    Expr::add(base.clone(), Expr::u32(VAST_TYPEDEF_SCOPE_FIELD)),
                )
            } else {
                Expr::u32(SENTINEL)
            },
        ),
        Node::let_bind("scope_depth", Expr::u32(0)),
        Node::let_bind("last_decl_kind", Expr::u32(0)),
        Node::let_bind("typedef_flags", Expr::u32(0)),
        Node::let_bind("annot_num_nodes", num_nodes.clone()),
    ];

    // The scope walker must run for EVERY row, not just for IDENTIFIER rows.
    // CPU oracle (`reference_c11_annotate_typedef_names_from_words`) writes
    // `scope_open_before(node_idx)` to the SCOPE field unconditionally, so the
    // GPU annotation must populate the scope_open carrier on every invocation
    // before the unconditional store-back loop reads `scope_open` at the end.
    // Gating it inside the `raw_kind == TOK_IDENTIFIER` branch (where it used
    // to live) leaves scope_open at its initial SENTINEL on every non-identifier
    // row, diverging from the CPU oracle on every brace, paren, semicolon, etc.
    if !precomputed_scope {
        loop_body.push(Node::loop_for(
            "scope_scan",
            Expr::u32(0),
            t.clone(),
            vec![
                Node::let_bind(
                    "scope_rev",
                    Expr::sub(Expr::sub(t.clone(), Expr::u32(1)), Expr::var("scope_scan")),
                ),
                Node::let_bind(
                    "scope_kind",
                    Expr::load(
                        vast_nodes,
                        Expr::mul(Expr::var("scope_rev"), Expr::u32(VAST_NODE_STRIDE_U32)),
                    ),
                ),
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("scope_open"), Expr::u32(SENTINEL)),
                        Expr::eq(Expr::var("scope_kind"), Expr::u32(TOK_RBRACE)),
                    ),
                    vec![Node::assign(
                        "scope_depth",
                        Expr::add(Expr::var("scope_depth"), Expr::u32(1)),
                    )],
                ),
                Node::if_then(
                    Expr::eq(Expr::var("scope_open"), Expr::u32(SENTINEL)),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("scope_kind"), Expr::u32(TOK_LBRACE)),
                        vec![Node::if_then_else(
                            Expr::eq(Expr::var("scope_depth"), Expr::u32(0)),
                            vec![Node::assign("scope_open", Expr::var("scope_rev"))],
                            vec![Node::assign(
                                "scope_depth",
                                Expr::sub(Expr::var("scope_depth"), Expr::u32(1)),
                            )],
                        )],
                    )],
                ),
            ],
        ));
    }
    let mut identifier_annotation: Vec<Node> = Vec::new();
    identifier_annotation.extend([
        Node::let_bind(
            "prev_idx",
            Expr::select(
                Expr::gt(t.clone(), Expr::u32(0)),
                Expr::sub(t.clone(), Expr::u32(1)),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "prev_kind_loaded",
            Expr::load(
                vast_nodes,
                Expr::mul(Expr::var("prev_idx"), Expr::u32(VAST_NODE_STRIDE_U32)),
            ),
        ),
        Node::let_bind(
            "prev_kind",
            Expr::select(
                Expr::gt(t.clone(), Expr::u32(0)),
                Expr::var("prev_kind_loaded"),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(
            "next_idx",
            Expr::select(
                Expr::lt(Expr::add(t.clone(), Expr::u32(1)), num_nodes.clone()),
                Expr::add(t.clone(), Expr::u32(1)),
                t.clone(),
            ),
        ),
        Node::let_bind(
            "next_kind_loaded",
            Expr::load(
                vast_nodes,
                Expr::mul(Expr::var("next_idx"), Expr::u32(VAST_NODE_STRIDE_U32)),
            ),
        ),
        Node::let_bind(
            "next_kind",
            Expr::select(
                Expr::lt(Expr::add(t.clone(), Expr::u32(1)), num_nodes.clone()),
                Expr::var("next_kind_loaded"),
                Expr::u32(SENTINEL),
            ),
        ),
    ]);
    // The CPU oracle (`reference_c11_annotate_typedef_names_from_words`)
    // resolves typedef visibility for every IDENTIFIER row that is not itself
    // a declarator, regardless of preceding tokens. The previous
    // `needs_typedef_visibility` gate excluded identifiers preceded by
    // STRUCT/UNION/ENUM/DOT/ARROW/GOTO or followed by COLON, which made
    // GPU output 0 in the TYPEDEF_FLAGS field for visible typedef names
    // appearing as struct/union/enum tags (e.g. row 5 of the tags fixture
    // where `typedef int S;` is later reused as `struct S { ... }`). The
    // scan produces a per-row result via the carrier; gating it here
    // diverged from the CPU contract on every tag spot.
    if let Some(decl_contexts) = decl_contexts {
        identifier_annotation.extend(emit_typedef_visibility_scan_precomputed_context(
            vast_nodes,
            decl_contexts,
            t.clone(),
        ));
    } else {
        identifier_annotation.extend(emit_typedef_visibility_scan(
            vast_nodes,
            haystack,
            decl_contexts,
            &haystack_len,
            &num_nodes,
            t.clone(),
            packed_haystack,
        ));
    }
    identifier_annotation.extend([
        Node::let_bind(
            "possible_declarator",
            any_token_eq(
                Expr::var("next_kind"),
                &[
                    TOK_SEMICOLON,
                    TOK_COMMA,
                    TOK_ASSIGN,
                    TOK_LPAREN,
                    TOK_LBRACKET,
                    TOK_COLON,
                    TOK_RPAREN,
                    TOK_RBRACKET,
                ],
            ),
        ),
        Node::let_bind("current_decl_flags", Expr::u32(0)),
        Node::let_bind(
            "declaration_candidate",
            Expr::and(
                Expr::var("possible_declarator"),
                Expr::and(
                    Expr::not(any_token_eq(
                        Expr::var("prev_kind"),
                        &[
                            TOK_STRUCT, TOK_UNION, TOK_ENUM, TOK_DOT, TOK_ARROW, TOK_GOTO,
                        ],
                    )),
                    Expr::ne(Expr::var("next_kind"), Expr::u32(TOK_COLON)),
                ),
            ),
        ),
    ]);
    let mut declaration_annotation = if let Some(decl_contexts) = decl_contexts {
        let mut nodes = emit_precomputed_declaration_kind_for_index(
            vast_nodes,
            decl_contexts,
            t.clone(),
            "current_decl_result_kind",
            "current_decl_precomputed",
        );
        nodes.push(Node::assign(
            "current_decl_flags",
            Expr::select(
                Expr::eq(Expr::var("current_decl_result_kind"), Expr::u32(1)),
                Expr::u32(C_TYPEDEF_FLAG_TYPEDEF_DECLARATOR),
                Expr::select(
                    Expr::eq(Expr::var("current_decl_result_kind"), Expr::u32(2)),
                    Expr::u32(C_TYPEDEF_FLAG_ORDINARY_DECLARATOR),
                    Expr::u32(0),
                ),
            ),
        ));
        nodes
    } else {
        emit_current_declaration_annotation(
            vast_nodes,
            haystack,
            &haystack_len,
            t.clone(),
            &num_nodes,
            packed_haystack,
            decl_contexts,
        )
    };

    declaration_annotation.extend([
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
                Expr::and(
                    Expr::eq(Expr::var("last_decl_kind"), Expr::u32(1)),
                    Expr::eq(Expr::var("current_decl_result_kind"), Expr::u32(0)),
                ),
            ),
            vec![Node::assign(
                "typedef_flags",
                Expr::bitor(
                    Expr::var("typedef_flags"),
                    Expr::u32(C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME),
                ),
            )],
        ),
        Node::if_then(
            is_typedef_declarator_annotation(Expr::var("current_decl_flags")),
            vec![Node::assign(
                "typedef_flags",
                Expr::bitor(
                    Expr::var("typedef_flags"),
                    Expr::u32(C_TYPEDEF_FLAG_TYPEDEF_DECLARATOR),
                ),
            )],
        ),
        Node::if_then(
            is_ordinary_declarator_annotation(Expr::var("current_decl_flags")),
            vec![Node::assign(
                "typedef_flags",
                Expr::bitor(
                    Expr::var("typedef_flags"),
                    Expr::u32(C_TYPEDEF_FLAG_ORDINARY_DECLARATOR),
                ),
            )],
        ),
    ]);
    identifier_annotation.push(Node::if_then(
        Expr::var("declaration_candidate"),
        declaration_annotation,
    ));
    identifier_annotation.push(Node::if_then(
        Expr::and(
            Expr::not(Expr::var("declaration_candidate")),
            Expr::eq(Expr::var("last_decl_kind"), Expr::u32(1)),
        ),
        vec![Node::assign(
            "typedef_flags",
            Expr::bitor(
                Expr::var("typedef_flags"),
                Expr::u32(C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME),
            ),
        )],
    ));
    loop_body.push(Node::if_then(
        Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
        identifier_annotation,
    ));

    for field in 0..VAST_NODE_STRIDE_U32 {
        let value = match field {
            VAST_TYPEDEF_FLAGS_FIELD => Expr::var("typedef_flags"),
            VAST_TYPEDEF_SCOPE_FIELD => Expr::var("scope_open"),
            VAST_TYPEDEF_SYMBOL_FIELD => Expr::var("name_hash"),
            _ => Expr::load(vast_nodes, Expr::add(base.clone(), Expr::u32(field))),
        };
        loop_body.push(Node::store(
            out_annotated_vast_nodes,
            Expr::add(base.clone(), Expr::u32(field)),
            value,
        ));
    }

    let n = node_count(&num_nodes).max(1);
    let mut buffers = vec![
        BufferDecl::storage(vast_nodes, 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
        BufferDecl::storage(haystack, 1, BufferAccess::ReadOnly, DataType::U32)
            .with_count(haystack_word_count(&haystack_len, packed_haystack)),
    ];
    let out_binding = if let Some(decl_contexts) = decl_contexts {
        buffers.push(
            BufferDecl::storage(decl_contexts, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.saturating_mul(VAST_DECL_CONTEXT_STRIDE_U32)),
        );
        3
    } else {
        2
    };
    buffers.push(
        BufferDecl::output(out_annotated_vast_nodes, out_binding, DataType::U32)
            .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
    );
    Program::wrapped(
        buffers,
        [256, 1, 1],
        vec![wrap_anonymous(
            ANNOTATE_TYPEDEF_OP_ID,
            vec![Node::if_then(Expr::lt(t, num_nodes), loop_body)],
        )],
    )
    .with_entry_op_id(ANNOTATE_TYPEDEF_OP_ID)
    .with_non_composable_with_self(true)
}
