use super::*;
use crate::parsing::c::lex::lexer::sections;

pub fn c11_lexer(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };

    let next_byte = |offset: u32| {
        Expr::select(
            Expr::lt(
                Expr::add(Expr::var("pos"), Expr::u32(offset)),
                Expr::buf_len(haystack),
            ),
            byte_load(haystack, Expr::add(Expr::var("pos"), Expr::u32(offset))),
            Expr::u32(0),
        )
    };

    let mut classify_at_pos = vec![
        Node::let_bind("byte", byte_load(haystack, Expr::var("pos"))),
        Node::let_bind(
            "prev_byte",
            Expr::select(
                Expr::gt(Expr::var("pos"), Expr::u32(0)),
                byte_load(
                    haystack,
                    Expr::saturating_sub(Expr::var("pos"), Expr::u32(1)),
                ),
                Expr::u32(0),
            ),
        ),
        Node::let_bind("next_byte", next_byte(1)),
        Node::let_bind("next2_byte", next_byte(2)),
        Node::let_bind("emit", Expr::u32(0)),
        Node::let_bind("tok_type", Expr::u32(TOK_WHITESPACE)),
        Node::let_bind("tok_len", Expr::u32(1)),
    ];

    classify_at_pos.push(set_token(
        Expr::and(
            byte_eq(Expr::var("byte"), b'#'),
            Expr::eq(Expr::var("line_allows_directive"), Expr::u32(1)),
        ),
        TOK_PREPROC,
        Expr::u32(1),
    ));
    classify_at_pos.push(Node::if_then(
        Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_PREPROC)),
        vec![
            Node::let_bind("preproc_done", Expr::u32(0)),
            Node::let_bind("preproc_spliced_cr", Expr::u32(0)),
            Node::loop_for(
                "scan_preproc",
                Expr::add(Expr::var("pos"), Expr::u32(1)),
                scan_upper_bound_with_cap(
                    haystack,
                    Expr::add(Expr::var("pos"), Expr::u32(1)),
                    MAX_PREPROC_SCAN,
                ),
                vec![Node::if_then(
                    Expr::eq(Expr::var("preproc_done"), Expr::u32(0)),
                    vec![
                        Node::let_bind("scan_byte", byte_load(haystack, Expr::var("scan_preproc"))),
                        Node::let_bind(
                            "scan_prev",
                            Expr::select(
                                Expr::gt(Expr::var("scan_preproc"), Expr::var("pos")),
                                byte_load(
                                    haystack,
                                    Expr::saturating_sub(Expr::var("scan_preproc"), Expr::u32(1)),
                                ),
                                Expr::u32(0),
                            ),
                        ),
                        Node::if_then_else(
                            Expr::or(
                                byte_eq(Expr::var("scan_byte"), b'\n'),
                                byte_eq(Expr::var("scan_byte"), b'\r'),
                            ),
                            vec![Node::if_then_else(
                                Expr::or(
                                    byte_eq(Expr::var("scan_prev"), b'\\'),
                                    Expr::and(
                                        byte_eq(Expr::var("scan_byte"), b'\n'),
                                        Expr::eq(Expr::var("preproc_spliced_cr"), Expr::u32(1)),
                                    ),
                                ),
                                vec![
                                    Node::assign(
                                        "tok_len",
                                        Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                                    ),
                                    Node::assign(
                                        "preproc_spliced_cr",
                                        Expr::select(
                                            byte_eq(Expr::var("scan_byte"), b'\r'),
                                            Expr::u32(1),
                                            Expr::u32(0),
                                        ),
                                    ),
                                ],
                                vec![Node::assign("preproc_done", Expr::u32(1))],
                            )],
                            vec![Node::assign(
                                "tok_len",
                                Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                            )],
                        ),
                    ],
                )],
            ),
        ],
    ));

    classify_at_pos.push(set_token(
        Expr::and(
            byte_eq(Expr::var("byte"), b'/'),
            byte_eq(Expr::var("next_byte"), b'/'),
        ),
        TOK_COMMENT,
        Expr::u32(2),
    ));
    classify_at_pos.push(Node::if_then(
        Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_COMMENT)),
        vec![
            Node::let_bind("comment_done", Expr::u32(0)),
            Node::loop_for(
                "scan_comment",
                Expr::add(Expr::var("pos"), Expr::u32(2)),
                scan_upper_bound_with_cap(
                    haystack,
                    Expr::add(Expr::var("pos"), Expr::u32(2)),
                    MAX_COMMENT_SCAN,
                ),
                vec![Node::if_then(
                    Expr::eq(Expr::var("comment_done"), Expr::u32(0)),
                    vec![
                        Node::let_bind("scan_byte", byte_load(haystack, Expr::var("scan_comment"))),
                        Node::if_then_else(
                            byte_eq(Expr::var("scan_byte"), b'\n'),
                            vec![Node::assign("comment_done", Expr::u32(1))],
                            vec![Node::assign(
                                "tok_len",
                                Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                            )],
                        ),
                    ],
                )],
            ),
        ],
    ));

    classify_at_pos.push(set_token(
        Expr::and(
            byte_eq(Expr::var("byte"), b'/'),
            byte_eq(Expr::var("next_byte"), b'*'),
        ),
        TOK_COMMENT,
        Expr::u32(2),
    ));
    classify_at_pos.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_COMMENT)),
            byte_eq(Expr::var("next_byte"), b'*'),
        ),
        vec![
            Node::let_bind("block_done", Expr::u32(0)),
            Node::loop_for(
                "scan_block_comment",
                Expr::add(Expr::var("pos"), Expr::u32(2)),
                scan_upper_bound_with_cap(
                    haystack,
                    Expr::add(Expr::var("pos"), Expr::u32(2)),
                    MAX_BLOCK_COMMENT_SCAN,
                ),
                vec![Node::if_then(
                    Expr::eq(Expr::var("block_done"), Expr::u32(0)),
                    vec![
                        Node::assign("tok_len", Expr::add(Expr::var("tok_len"), Expr::u32(1))),
                        Node::let_bind(
                            "scan_byte",
                            byte_load(haystack, Expr::var("scan_block_comment")),
                        ),
                        Node::let_bind(
                            "scan_next",
                            Expr::select(
                                Expr::lt(
                                    Expr::add(Expr::var("scan_block_comment"), Expr::u32(1)),
                                    Expr::buf_len(haystack),
                                ),
                                byte_load(
                                    haystack,
                                    Expr::add(Expr::var("scan_block_comment"), Expr::u32(1)),
                                ),
                                Expr::u32(0),
                            ),
                        ),
                        Node::if_then(
                            Expr::and(
                                byte_eq(Expr::var("scan_byte"), b'*'),
                                byte_eq(Expr::var("scan_next"), b'/'),
                            ),
                            vec![
                                Node::assign(
                                    "tok_len",
                                    Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                                ),
                                Node::assign("block_done", Expr::u32(1)),
                            ],
                        ),
                    ],
                )],
            ),
            Node::if_then(
                Expr::eq(Expr::var("block_done"), Expr::u32(0)),
                vec![Node::assign(
                    "tok_type",
                    Expr::u32(TOK_ERR_UNTERMINATED_COMMENT),
                )],
            ),
        ],
    ));

    classify_at_pos.push(set_token(
        Expr::or(
            Expr::and(
                Expr::or(
                    byte_eq(Expr::var("byte"), b'L'),
                    Expr::or(
                        byte_eq(Expr::var("byte"), b'u'),
                        byte_eq(Expr::var("byte"), b'U'),
                    ),
                ),
                byte_eq(Expr::var("next_byte"), b'"'),
            ),
            Expr::and(
                Expr::and(
                    byte_eq(Expr::var("byte"), b'u'),
                    byte_eq(Expr::var("next_byte"), b'8'),
                ),
                byte_eq(Expr::var("next2_byte"), b'"'),
            ),
        ),
        TOK_STRING,
        Expr::select(
            Expr::and(
                byte_eq(Expr::var("byte"), b'u'),
                byte_eq(Expr::var("next_byte"), b'8'),
            ),
            Expr::u32(3),
            Expr::u32(2),
        ),
    ));
    classify_at_pos.push(set_token(
        Expr::or(
            Expr::and(
                Expr::or(
                    byte_eq(Expr::var("byte"), b'L'),
                    Expr::or(
                        byte_eq(Expr::var("byte"), b'u'),
                        byte_eq(Expr::var("byte"), b'U'),
                    ),
                ),
                byte_eq(Expr::var("next_byte"), b'\''),
            ),
            Expr::and(
                Expr::and(
                    byte_eq(Expr::var("byte"), b'u'),
                    byte_eq(Expr::var("next_byte"), b'8'),
                ),
                byte_eq(Expr::var("next2_byte"), b'\''),
            ),
        ),
        TOK_CHAR,
        Expr::select(
            Expr::and(
                byte_eq(Expr::var("byte"), b'u'),
                byte_eq(Expr::var("next_byte"), b'8'),
            ),
            Expr::u32(3),
            Expr::u32(2),
        ),
    ));

    classify_at_pos.push(set_token(
        Expr::and(
            is_ident_start(Expr::var("byte")),
            Expr::not(is_ident_continue(Expr::var("prev_byte"))),
        ),
        TOK_IDENTIFIER,
        Expr::u32(1),
    ));
    classify_at_pos.push(Node::if_then(
        Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_IDENTIFIER)),
        vec![
            Node::let_bind("ident_done", Expr::u32(0)),
            Node::loop_for(
                "scan_ident",
                Expr::add(Expr::var("pos"), Expr::u32(1)),
                scan_upper_bound_with_cap(
                    haystack,
                    Expr::add(Expr::var("pos"), Expr::u32(1)),
                    MAX_IDENT_SCAN,
                ),
                vec![Node::if_then(
                    Expr::eq(Expr::var("ident_done"), Expr::u32(0)),
                    vec![
                        Node::let_bind("scan_byte", byte_load(haystack, Expr::var("scan_ident"))),
                        Node::if_then_else(
                            is_ident_continue(Expr::var("scan_byte")),
                            vec![Node::assign(
                                "tok_len",
                                Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                            )],
                            vec![Node::assign("ident_done", Expr::u32(1))],
                        ),
                    ],
                )],
            ),
        ],
    ));
    classify_at_pos.push(set_token(
        Expr::and(
            is_digit(Expr::var("byte")),
            Expr::not(is_ident_continue(Expr::var("prev_byte"))),
        ),
        TOK_INTEGER,
        Expr::u32(1),
    ));
    classify_at_pos.push(set_token(
        Expr::and(
            byte_eq(Expr::var("byte"), b'.'),
            is_digit(Expr::var("next_byte")),
        ),
        TOK_FLOAT,
        Expr::u32(1),
    ));
    classify_at_pos.push(Node::if_then(
        Expr::or(
            Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_INTEGER)),
            Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_FLOAT)),
        ),
        vec![
            Node::let_bind("number_done", Expr::u32(0)),
            Node::let_bind(
                "number_is_float",
                Expr::select(
                    Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_FLOAT)),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            ),
            Node::loop_for(
                "scan_number",
                Expr::add(Expr::var("pos"), Expr::u32(1)),
                scan_upper_bound_with_cap(
                    haystack,
                    Expr::add(Expr::var("pos"), Expr::u32(1)),
                    MAX_NUMBER_SCAN,
                ),
                vec![Node::if_then(
                    Expr::eq(Expr::var("number_done"), Expr::u32(0)),
                    vec![
                        Node::let_bind("scan_byte", byte_load(haystack, Expr::var("scan_number"))),
                        Node::let_bind(
                            "scan_prev",
                            byte_load(
                                haystack,
                                Expr::saturating_sub(Expr::var("scan_number"), Expr::u32(1)),
                            ),
                        ),
                        Node::let_bind(
                            "scan_next",
                            Expr::select(
                                Expr::lt(
                                    Expr::add(Expr::var("scan_number"), Expr::u32(1)),
                                    Expr::buf_len(haystack),
                                ),
                                byte_load(
                                    haystack,
                                    Expr::add(Expr::var("scan_number"), Expr::u32(1)),
                                ),
                                Expr::u32(0),
                            ),
                        ),
                        Node::let_bind(
                            "scan_can_start_exponent",
                            Expr::and(
                                Expr::or(
                                    byte_eq(Expr::var("scan_byte"), b'e'),
                                    Expr::or(
                                        byte_eq(Expr::var("scan_byte"), b'E'),
                                        Expr::or(
                                            byte_eq(Expr::var("scan_byte"), b'p'),
                                            byte_eq(Expr::var("scan_byte"), b'P'),
                                        ),
                                    ),
                                ),
                                Expr::or(
                                    is_digit(Expr::var("scan_next")),
                                    Expr::or(
                                        byte_eq(Expr::var("scan_next"), b'+'),
                                        byte_eq(Expr::var("scan_next"), b'-'),
                                    ),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "scan_is_exponent_sign",
                            Expr::and(
                                Expr::or(
                                    byte_eq(Expr::var("scan_byte"), b'+'),
                                    byte_eq(Expr::var("scan_byte"), b'-'),
                                ),
                                Expr::or(
                                    byte_eq(Expr::var("scan_prev"), b'e'),
                                    Expr::or(
                                        byte_eq(Expr::var("scan_prev"), b'E'),
                                        Expr::or(
                                            byte_eq(Expr::var("scan_prev"), b'p'),
                                            byte_eq(Expr::var("scan_prev"), b'P'),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                        Node::let_bind("scan_is_float_dot", byte_eq(Expr::var("scan_byte"), b'.')),
                        Node::let_bind(
                            "scan_is_number_tail",
                            Expr::or(
                                is_ident_continue(Expr::var("scan_byte")),
                                Expr::or(
                                    Expr::var("scan_is_float_dot"),
                                    Expr::var("scan_is_exponent_sign"),
                                ),
                            ),
                        ),
                        Node::if_then_else(
                            Expr::var("scan_is_number_tail"),
                            vec![
                                Node::assign(
                                    "tok_len",
                                    Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                                ),
                                Node::if_then(
                                    Expr::or(
                                        Expr::var("scan_is_float_dot"),
                                        Expr::var("scan_can_start_exponent"),
                                    ),
                                    vec![Node::assign("number_is_float", Expr::u32(1))],
                                ),
                            ],
                            vec![Node::assign("number_done", Expr::u32(1))],
                        ),
                    ],
                )],
            ),
            Node::if_then(
                Expr::eq(Expr::var("number_is_float"), Expr::u32(1)),
                vec![Node::assign("tok_type", Expr::u32(TOK_FLOAT))],
            ),
        ],
    ));

    classify_at_pos.push(set_token(
        byte_eq(Expr::var("byte"), b'"'),
        TOK_STRING,
        Expr::u32(1),
    ));
    classify_at_pos.push(set_token(
        byte_eq(Expr::var("byte"), b'\''),
        TOK_CHAR,
        Expr::u32(1),
    ));
    classify_at_pos.push(Node::if_then(
        Expr::or(
            Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_STRING)),
            Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_CHAR)),
        ),
        vec![
            Node::let_bind(
                "literal_quote_offset",
                Expr::select(
                    Expr::or(
                        byte_eq(Expr::var("byte"), b'"'),
                        byte_eq(Expr::var("byte"), b'\''),
                    ),
                    Expr::u32(0),
                    Expr::select(
                        Expr::and(
                            byte_eq(Expr::var("byte"), b'u'),
                            byte_eq(Expr::var("next_byte"), b'8'),
                        ),
                        Expr::u32(2),
                        Expr::u32(1),
                    ),
                ),
            ),
            Node::let_bind(
                "quote",
                byte_load(
                    haystack,
                    Expr::add(Expr::var("pos"), Expr::var("literal_quote_offset")),
                ),
            ),
            Node::let_bind("literal_done", Expr::u32(0)),
            Node::let_bind("escaped", Expr::u32(0)),
            Node::let_bind("literal_unterminated", Expr::u32(0)),
            Node::let_bind("invalid_escape", Expr::u32(0)),
            Node::loop_for(
                "scan_literal",
                Expr::add(
                    Expr::add(Expr::var("pos"), Expr::var("literal_quote_offset")),
                    Expr::u32(1),
                ),
                scan_upper_bound_with_cap(
                    haystack,
                    Expr::add(
                        Expr::add(Expr::var("pos"), Expr::var("literal_quote_offset")),
                        Expr::u32(1),
                    ),
                    MAX_LITERAL_SCAN,
                ),
                vec![Node::if_then(
                    Expr::eq(Expr::var("literal_done"), Expr::u32(0)),
                    vec![
                        Node::assign("tok_len", Expr::add(Expr::var("tok_len"), Expr::u32(1))),
                        Node::let_bind("scan_byte", byte_load(haystack, Expr::var("scan_literal"))),
                        Node::if_then_else(
                            Expr::eq(Expr::var("escaped"), Expr::u32(1)),
                            vec![
                                Node::if_then(
                                    Expr::not(is_valid_escape_byte(
                                        haystack,
                                        Expr::var("scan_literal"),
                                        Expr::var("scan_byte"),
                                        haystack_len,
                                    )),
                                    vec![Node::assign("invalid_escape", Expr::u32(1))],
                                ),
                                Node::assign("escaped", Expr::u32(0)),
                            ],
                            vec![Node::if_then_else(
                                byte_eq(Expr::var("scan_byte"), b'\\'),
                                vec![Node::assign("escaped", Expr::u32(1))],
                                vec![Node::if_then_else(
                                    Expr::eq(Expr::var("scan_byte"), Expr::var("quote")),
                                    vec![Node::assign("literal_done", Expr::u32(1))],
                                    vec![Node::if_then(
                                        Expr::or(
                                            byte_eq(Expr::var("scan_byte"), b'\n'),
                                            byte_eq(Expr::var("scan_byte"), b'\r'),
                                        ),
                                        vec![
                                            Node::assign("literal_unterminated", Expr::u32(1)),
                                            Node::assign("literal_done", Expr::u32(1)),
                                        ],
                                    )],
                                )],
                            )],
                        ),
                    ],
                )],
            ),
            Node::if_then(
                Expr::eq(Expr::var("literal_done"), Expr::u32(0)),
                vec![Node::assign("literal_unterminated", Expr::u32(1))],
            ),
            Node::if_then(
                Expr::eq(Expr::var("literal_unterminated"), Expr::u32(1)),
                vec![Node::assign(
                    "tok_type",
                    Expr::select(
                        Expr::eq(Expr::var("quote"), ascii(b'"')),
                        Expr::u32(TOK_ERR_UNTERMINATED_STRING),
                        Expr::u32(TOK_ERR_UNTERMINATED_CHAR),
                    ),
                )],
            ),
            Node::if_then(
                Expr::and(
                    Expr::eq(Expr::var("literal_unterminated"), Expr::u32(0)),
                    Expr::eq(Expr::var("invalid_escape"), Expr::u32(1)),
                ),
                vec![Node::assign("tok_type", Expr::u32(TOK_ERR_INVALID_ESCAPE))],
            ),
        ],
    ));

    classify_at_pos.extend(sections::operator_punct_pushes());
    classify_at_pos.extend(sections::store_token_and_advance_pushes(
        haystack,
        haystack_len,
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
    ));

    Program::wrapped(
        vec![
            BufferDecl::storage(haystack, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(haystack_len.max(1)),
            BufferDecl::storage(out_tok_types, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(haystack_len.max(1)),
            BufferDecl::storage(out_tok_starts, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(haystack_len.max(1)),
            BufferDecl::storage(out_tok_lens, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(haystack_len.max(1)),
            BufferDecl::storage(out_counts, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        {
            let entry_body = vec![Node::if_then(
                Expr::eq(t.clone(), Expr::u32(0)),
                vec![
                    Node::let_bind("cursor", Expr::u32(0)),
                    Node::let_bind("line_allows_directive", Expr::u32(1)),
                    Node::let_bind("tok_idx", Expr::u32(0)),
                    Node::loop_for(
                        "token_iter",
                        Expr::u32(0),
                        Expr::buf_len(haystack),
                        vec![Node::if_then(
                            Expr::lt(Expr::var("cursor"), Expr::buf_len(haystack)),
                            {
                                let mut body = vec![Node::let_bind("pos", Expr::var("cursor"))];
                                body.push(child_phase(
                                    "vyre-libs::parsing::c_lexer",
                                    "vyre-libs::parsing::c_lexer::classify_at_pos",
                                    classify_at_pos,
                                ));
                                body
                            },
                        )],
                    ),
                    Node::store(out_counts, Expr::u32(0), Expr::var("tok_idx")),
                ],
            )];
            vec![wrap_anonymous("vyre-libs::parsing::c_lexer", entry_body)]
        },
    )
    .with_entry_op_id("vyre-libs::parsing::c_lexer")
    .with_non_composable_with_self(true)
}
