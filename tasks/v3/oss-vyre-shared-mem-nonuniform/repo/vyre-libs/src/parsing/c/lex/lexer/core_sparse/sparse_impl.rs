use super::*;
use crate::parsing::c::lex::lexer::sections;

pub(super) fn c11_lexer_regular_sparse_impl(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
    suppress_span_readback: bool,
    emit_flags: bool,
    haystack_layout: SparseHaystackLayout,
    track_preproc_lines: bool,
    track_literals: bool,
    block_totals: Option<&str>,
) -> Program {
    let workgroup_lanes = if block_totals.is_some() {
        vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES
    } else {
        256
    };
    let t = Expr::InvocationId { axis: 0 };
    let byte_at = |index: Expr| match haystack_layout {
        SparseHaystackLayout::PackedU32 => {
            let word = Expr::load(haystack, Expr::shr(index.clone(), Expr::u32(2)));
            let shift = Expr::shl(Expr::bitand(index.clone(), Expr::u32(3)), Expr::u32(3));
            Expr::select(
                Expr::lt(index, Expr::u32(haystack_len)),
                Expr::bitand(Expr::shr(word, shift), Expr::u32(0xFF)),
                Expr::u32(0),
            )
        }
        SparseHaystackLayout::ExpandedU32 => byte_at_or_zero(haystack, index, haystack_len),
        SparseHaystackLayout::RawU8 => {
            let runtime_len = Expr::min(Expr::buf_len(haystack), Expr::u32(haystack_len));
            let in_bounds = Expr::lt(index.clone(), runtime_len.clone());
            let safe_index = Expr::select(
                in_bounds.clone(),
                index,
                Expr::saturating_sub(runtime_len, Expr::u32(1)),
            );
            let byte = Expr::bitand(
                Expr::cast(DataType::U32, Expr::load(haystack, safe_index)),
                Expr::u32(0xFF),
            );
            Expr::select(in_bounds, byte, Expr::u32(0))
        }
    };
    let is_space = |value: Expr| {
        Expr::or(
            byte_eq(value.clone(), 0),
            Expr::or(
                byte_eq(value.clone(), b' '),
                Expr::or(
                    byte_eq(value.clone(), b'\n'),
                    Expr::or(byte_eq(value.clone(), b'\r'), byte_eq(value, b'\t')),
                ),
            ),
        )
    };
    let is_operator_tail = |index: Expr| {
        let b = byte_at(index.clone());
        let prev = Expr::select(
            Expr::gt(index.clone(), Expr::u32(0)),
            byte_at(Expr::saturating_sub(index.clone(), Expr::u32(1))),
            Expr::u32(0),
        );
        let prev2 = Expr::select(
            Expr::gt(index.clone(), Expr::u32(1)),
            byte_at(Expr::saturating_sub(index.clone(), Expr::u32(2))),
            Expr::u32(0),
        );
        Expr::or(
            Expr::and(byte_eq(b.clone(), b'>'), byte_eq(prev.clone(), b'-')),
            Expr::or(
                Expr::and(
                    byte_eq(b.clone(), b'='),
                    Expr::or(
                        byte_eq(prev.clone(), b'+'),
                        Expr::or(
                            byte_eq(prev.clone(), b'-'),
                            Expr::or(
                                byte_eq(prev.clone(), b'*'),
                                Expr::or(
                                    byte_eq(prev.clone(), b'/'),
                                    Expr::or(
                                        byte_eq(prev.clone(), b'%'),
                                        Expr::or(
                                            byte_eq(prev.clone(), b'&'),
                                            Expr::or(
                                                byte_eq(prev.clone(), b'|'),
                                                Expr::or(
                                                    byte_eq(prev.clone(), b'^'),
                                                    Expr::or(
                                                        byte_eq(prev.clone(), b'='),
                                                        Expr::or(
                                                            byte_eq(prev.clone(), b'!'),
                                                            Expr::or(
                                                                byte_eq(prev.clone(), b'<'),
                                                                byte_eq(prev.clone(), b'>'),
                                                            ),
                                                        ),
                                                    ),
                                                ),
                                            ),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
                Expr::or(
                    Expr::and(byte_eq(b.clone(), b'.'), byte_eq(prev.clone(), b'.')),
                    Expr::or(
                        Expr::and(byte_eq(b.clone(), b'+'), byte_eq(prev.clone(), b'+')),
                        Expr::or(
                            Expr::and(byte_eq(b.clone(), b'-'), byte_eq(prev.clone(), b'-')),
                            Expr::or(
                                Expr::and(byte_eq(b.clone(), b'&'), byte_eq(prev.clone(), b'&')),
                                Expr::or(
                                    Expr::and(
                                        byte_eq(b.clone(), b'|'),
                                        byte_eq(prev.clone(), b'|'),
                                    ),
                                    Expr::or(
                                        Expr::and(
                                            byte_eq(b.clone(), b'<'),
                                            byte_eq(prev.clone(), b'<'),
                                        ),
                                        Expr::or(
                                            Expr::and(
                                                byte_eq(b.clone(), b'>'),
                                                byte_eq(prev.clone(), b'>'),
                                            ),
                                            Expr::and(
                                                byte_eq(b, b'='),
                                                Expr::or(
                                                    Expr::and(
                                                        byte_eq(prev.clone(), b'<'),
                                                        byte_eq(prev2.clone(), b'<'),
                                                    ),
                                                    Expr::and(
                                                        byte_eq(prev, b'>'),
                                                        byte_eq(prev2, b'>'),
                                                    ),
                                                ),
                                            ),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        )
    };
    let is_token_start_at = |index: Expr| {
        let b = byte_at(index.clone());
        let prev = Expr::select(
            Expr::gt(index.clone(), Expr::u32(0)),
            byte_at(Expr::saturating_sub(index.clone(), Expr::u32(1))),
            Expr::u32(0),
        );
        Expr::and(
            Expr::lt(index.clone(), Expr::u32(haystack_len)),
            Expr::and(
                Expr::not(is_space(b.clone())),
                Expr::and(
                    Expr::not(Expr::and(is_ident_continue(b), is_ident_continue(prev))),
                    Expr::not(is_operator_tail(index)),
                ),
            ),
        )
    };

    let preliminary_start = Expr::and(
        Expr::lt(t.clone(), Expr::u32(haystack_len)),
        is_token_start_at(t.clone()),
    );
    let mut string_state_prefix = vec![
        Node::let_bind("sparse_preliminary_start", preliminary_start),
        Node::let_bind("sparse_inside_string", Expr::u32(0)),
        Node::let_bind("sparse_inside_char", Expr::u32(0)),
        Node::let_bind("sparse_inside_line_comment", Expr::u32(0)),
        Node::let_bind("sparse_inside_block_comment", Expr::u32(0)),
        Node::let_bind("sparse_literal_escape", Expr::u32(0)),
    ];
    if track_literals {
        string_state_prefix.push(Node::if_then(
            Expr::var("sparse_preliminary_start"),
            vec![Node::loop_for(
                "sparse_literal_backscan",
                Expr::saturating_sub(t.clone(), Expr::u32(MAX_SPARSE_TOKEN_SCAN)),
                t.clone(),
                vec![
                    Node::let_bind(
                        "sparse_literal_scan_byte",
                        byte_at(Expr::var("sparse_literal_backscan")),
                    ),
                    Node::let_bind(
                        "sparse_literal_scan_prev",
                        Expr::select(
                            Expr::gt(Expr::var("sparse_literal_backscan"), Expr::u32(0)),
                            byte_at(Expr::saturating_sub(
                                Expr::var("sparse_literal_backscan"),
                                Expr::u32(1),
                            )),
                            Expr::u32(0),
                        ),
                    ),
                    Node::let_bind(
                        "sparse_literal_scan_next",
                        byte_at(Expr::add(
                            Expr::var("sparse_literal_backscan"),
                            Expr::u32(1),
                        )),
                    ),
                    Node::if_then_else(
                        Expr::eq(Expr::var("sparse_inside_line_comment"), Expr::u32(1)),
                        vec![Node::if_then(
                            Expr::or(
                                byte_eq(Expr::var("sparse_literal_scan_byte"), b'\n'),
                                byte_eq(Expr::var("sparse_literal_scan_byte"), b'\r'),
                            ),
                            vec![Node::assign("sparse_inside_line_comment", Expr::u32(0))],
                        )],
                        vec![Node::if_then_else(
                            Expr::eq(Expr::var("sparse_inside_block_comment"), Expr::u32(1)),
                            vec![Node::if_then(
                                Expr::and(
                                    byte_eq(Expr::var("sparse_literal_scan_prev"), b'*'),
                                    byte_eq(Expr::var("sparse_literal_scan_byte"), b'/'),
                                ),
                                vec![Node::assign("sparse_inside_block_comment", Expr::u32(0))],
                            )],
                            vec![Node::if_then_else(
                                Expr::eq(Expr::var("sparse_literal_escape"), Expr::u32(1)),
                                vec![Node::assign("sparse_literal_escape", Expr::u32(0))],
                                vec![Node::if_then_else(
                                    Expr::and(
                                        byte_eq(Expr::var("sparse_literal_scan_byte"), b'\\'),
                                        Expr::or(
                                            Expr::eq(
                                                Expr::var("sparse_inside_string"),
                                                Expr::u32(1),
                                            ),
                                            Expr::eq(Expr::var("sparse_inside_char"), Expr::u32(1)),
                                        ),
                                    ),
                                    vec![Node::assign("sparse_literal_escape", Expr::u32(1))],
                                    vec![
                                        Node::if_then(
                                            Expr::and(
                                                Expr::and(
                                                    byte_eq(
                                                        Expr::var("sparse_literal_scan_byte"),
                                                        b'/',
                                                    ),
                                                    byte_eq(
                                                        Expr::var("sparse_literal_scan_next"),
                                                        b'/',
                                                    ),
                                                ),
                                                Expr::and(
                                                    Expr::eq(
                                                        Expr::var("sparse_inside_string"),
                                                        Expr::u32(0),
                                                    ),
                                                    Expr::eq(
                                                        Expr::var("sparse_inside_char"),
                                                        Expr::u32(0),
                                                    ),
                                                ),
                                            ),
                                            vec![Node::assign(
                                                "sparse_inside_line_comment",
                                                Expr::u32(1),
                                            )],
                                        ),
                                        Node::if_then(
                                            Expr::and(
                                                Expr::and(
                                                    byte_eq(
                                                        Expr::var("sparse_literal_scan_byte"),
                                                        b'/',
                                                    ),
                                                    byte_eq(
                                                        Expr::var("sparse_literal_scan_next"),
                                                        b'*',
                                                    ),
                                                ),
                                                Expr::and(
                                                    Expr::eq(
                                                        Expr::var("sparse_inside_string"),
                                                        Expr::u32(0),
                                                    ),
                                                    Expr::eq(
                                                        Expr::var("sparse_inside_char"),
                                                        Expr::u32(0),
                                                    ),
                                                ),
                                            ),
                                            vec![Node::assign(
                                                "sparse_inside_block_comment",
                                                Expr::u32(1),
                                            )],
                                        ),
                                        Node::if_then(
                                            Expr::and(
                                                byte_eq(
                                                    Expr::var("sparse_literal_scan_byte"),
                                                    b'"',
                                                ),
                                                Expr::eq(
                                                    Expr::var("sparse_inside_char"),
                                                    Expr::u32(0),
                                                ),
                                            ),
                                            vec![Node::assign(
                                                "sparse_inside_string",
                                                Expr::select(
                                                    Expr::eq(
                                                        Expr::var("sparse_inside_string"),
                                                        Expr::u32(0),
                                                    ),
                                                    Expr::u32(1),
                                                    Expr::u32(0),
                                                ),
                                            )],
                                        ),
                                        Node::if_then(
                                            Expr::and(
                                                byte_eq(
                                                    Expr::var("sparse_literal_scan_byte"),
                                                    b'\'',
                                                ),
                                                Expr::eq(
                                                    Expr::var("sparse_inside_string"),
                                                    Expr::u32(0),
                                                ),
                                            ),
                                            vec![Node::assign(
                                                "sparse_inside_char",
                                                Expr::select(
                                                    Expr::eq(
                                                        Expr::var("sparse_inside_char"),
                                                        Expr::u32(0),
                                                    ),
                                                    Expr::u32(1),
                                                    Expr::u32(0),
                                                ),
                                            )],
                                        ),
                                    ],
                                )],
                            )],
                        )],
                    ),
                ],
            )],
        ));
    }
    if track_preproc_lines {
        string_state_prefix.extend([
            Node::let_bind("sparse_line_allows_directive", Expr::u32(1)),
            Node::let_bind("sparse_inside_preproc_line", Expr::u32(0)),
            Node::loop_for(
                "sparse_preproc_state_scan",
                Expr::u32(0),
                t.clone(),
                vec![
                    Node::let_bind(
                        "sparse_preproc_state_byte",
                        byte_at(Expr::var("sparse_preproc_state_scan")),
                    ),
                    Node::if_then_else(
                        Expr::or(
                            byte_eq(Expr::var("sparse_preproc_state_byte"), b'\n'),
                            byte_eq(Expr::var("sparse_preproc_state_byte"), b'\r'),
                        ),
                        vec![
                            Node::assign("sparse_inside_preproc_line", Expr::u32(0)),
                            Node::assign("sparse_line_allows_directive", Expr::u32(1)),
                        ],
                        vec![Node::if_then(
                            Expr::eq(Expr::var("sparse_inside_preproc_line"), Expr::u32(0)),
                            vec![Node::if_then(
                                Expr::eq(Expr::var("sparse_line_allows_directive"), Expr::u32(1)),
                                vec![Node::if_then_else(
                                    Expr::or(
                                        byte_eq(Expr::var("sparse_preproc_state_byte"), b' '),
                                        byte_eq(Expr::var("sparse_preproc_state_byte"), b'\t'),
                                    ),
                                    Vec::new(),
                                    vec![Node::if_then_else(
                                        byte_eq(Expr::var("sparse_preproc_state_byte"), b'#'),
                                        vec![Node::assign(
                                            "sparse_inside_preproc_line",
                                            Expr::u32(1),
                                        )],
                                        vec![Node::assign(
                                            "sparse_line_allows_directive",
                                            Expr::u32(0),
                                        )],
                                    )],
                                )],
                            )],
                        )],
                    ),
                ],
            ),
        ]);
    } else {
        string_state_prefix.extend([
            Node::let_bind("sparse_line_allows_directive", Expr::u32(0)),
            Node::let_bind("sparse_inside_preproc_line", Expr::u32(0)),
        ]);
    }

    let mut classify_at_pos = vec![
        Node::let_bind("pos", t.clone()),
        Node::let_bind("byte", byte_at(t.clone())),
        Node::let_bind(
            "prev_byte",
            Expr::select(
                Expr::gt(t.clone(), Expr::u32(0)),
                byte_at(Expr::saturating_sub(t.clone(), Expr::u32(1))),
                Expr::u32(0),
            ),
        ),
        Node::let_bind("next_byte", byte_at(Expr::add(t.clone(), Expr::u32(1)))),
        Node::let_bind("next2_byte", byte_at(Expr::add(t.clone(), Expr::u32(2)))),
        Node::let_bind("emit", Expr::u32(0)),
        Node::let_bind("tok_type", Expr::u32(TOK_WHITESPACE)),
        Node::let_bind("tok_len", Expr::u32(1)),
    ];
    classify_at_pos.push(set_token(
        Expr::and(
            is_ident_start(Expr::var("byte")),
            Expr::not(is_ident_continue(Expr::var("prev_byte"))),
        ),
        TOK_IDENTIFIER,
        Expr::u32(1),
    ));
    if track_literals {
        classify_at_pos.push(set_token(
            byte_eq(Expr::var("byte"), b'"'),
            TOK_STRING,
            Expr::u32(1),
        ));
        classify_at_pos.push(set_token(
            Expr::and(
                byte_eq(Expr::var("byte"), b'\''),
                Expr::not(is_ident_continue(Expr::var("prev_byte"))),
            ),
            TOK_CHAR,
            Expr::u32(1),
        ));
        classify_at_pos.push(set_token(
            Expr::and(
                byte_eq(Expr::var("byte"), b'/'),
                byte_eq(Expr::var("next_byte"), b'/'),
            ),
            TOK_COMMENT,
            Expr::u32(2),
        ));
        if !suppress_span_readback {
            classify_at_pos.push(Node::if_then(
                Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_COMMENT)),
                vec![
                    Node::let_bind("sparse_comment_done", Expr::u32(0)),
                    Node::loop_for(
                        "sparse_scan_line_comment",
                        Expr::add(Expr::var("pos"), Expr::u32(2)),
                        Expr::min(
                            Expr::add(Expr::var("pos"), Expr::u32(MAX_SPARSE_TOKEN_SCAN)),
                            Expr::u32(haystack_len),
                        ),
                        vec![Node::if_then(
                            Expr::eq(Expr::var("sparse_comment_done"), Expr::u32(0)),
                            vec![
                                Node::let_bind(
                                    "scan_byte",
                                    byte_at(Expr::var("sparse_scan_line_comment")),
                                ),
                                Node::if_then_else(
                                    byte_eq(Expr::var("scan_byte"), b'\n'),
                                    vec![Node::assign("sparse_comment_done", Expr::u32(1))],
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
        }
        classify_at_pos.push(set_token(
            Expr::and(
                byte_eq(Expr::var("byte"), b'/'),
                byte_eq(Expr::var("next_byte"), b'*'),
            ),
            TOK_COMMENT,
            Expr::u32(2),
        ));
        if !suppress_span_readback {
            classify_at_pos.push(Node::if_then(
                Expr::and(
                    Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_COMMENT)),
                    byte_eq(Expr::var("next_byte"), b'*'),
                ),
                vec![
                    Node::let_bind("sparse_block_comment_done", Expr::u32(0)),
                    Node::loop_for(
                        "sparse_scan_block_comment",
                        Expr::add(Expr::var("pos"), Expr::u32(2)),
                        Expr::min(
                            Expr::add(Expr::var("pos"), Expr::u32(MAX_SPARSE_TOKEN_SCAN)),
                            Expr::u32(haystack_len),
                        ),
                        vec![Node::if_then(
                            Expr::eq(Expr::var("sparse_block_comment_done"), Expr::u32(0)),
                            vec![
                                Node::assign(
                                    "tok_len",
                                    Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                                ),
                                Node::let_bind(
                                    "scan_byte",
                                    byte_at(Expr::var("sparse_scan_block_comment")),
                                ),
                                Node::let_bind(
                                    "scan_next",
                                    byte_at(Expr::add(
                                        Expr::var("sparse_scan_block_comment"),
                                        Expr::u32(1),
                                    )),
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
                                        Node::assign("sparse_block_comment_done", Expr::u32(1)),
                                    ],
                                ),
                            ],
                        )],
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var("sparse_block_comment_done"), Expr::u32(0)),
                        vec![Node::assign(
                            "tok_type",
                            Expr::u32(TOK_ERR_UNTERMINATED_COMMENT),
                        )],
                    ),
                ],
            ));
            classify_at_pos.push(Node::if_then(
                Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_STRING)),
                vec![
                    Node::let_bind("sparse_string_done", Expr::u32(0)),
                    Node::let_bind("sparse_string_literal_escape", Expr::u32(0)),
                    Node::loop_for(
                        "sparse_scan_string",
                        Expr::add(Expr::var("pos"), Expr::u32(1)),
                        Expr::min(
                            Expr::add(Expr::var("pos"), Expr::u32(MAX_SPARSE_TOKEN_SCAN)),
                            Expr::u32(haystack_len),
                        ),
                        vec![Node::if_then(
                            Expr::eq(Expr::var("sparse_string_done"), Expr::u32(0)),
                            vec![
                                Node::let_bind(
                                    "scan_byte",
                                    byte_at(Expr::var("sparse_scan_string")),
                                ),
                                Node::assign(
                                    "tok_len",
                                    Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                                ),
                                Node::if_then_else(
                                    Expr::eq(
                                        Expr::var("sparse_string_literal_escape"),
                                        Expr::u32(1),
                                    ),
                                    vec![Node::assign(
                                        "sparse_string_literal_escape",
                                        Expr::u32(0),
                                    )],
                                    vec![Node::if_then_else(
                                        byte_eq(Expr::var("scan_byte"), b'\\'),
                                        vec![Node::assign(
                                            "sparse_string_literal_escape",
                                            Expr::u32(1),
                                        )],
                                        vec![Node::if_then(
                                            byte_eq(Expr::var("scan_byte"), b'"'),
                                            vec![Node::assign("sparse_string_done", Expr::u32(1))],
                                        )],
                                    )],
                                ),
                            ],
                        )],
                    ),
                ],
            ));
            classify_at_pos.push(Node::if_then(
                Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_CHAR)),
                vec![
                    Node::let_bind("sparse_char_done", Expr::u32(0)),
                    Node::let_bind("sparse_char_literal_escape", Expr::u32(0)),
                    Node::loop_for(
                        "sparse_scan_char",
                        Expr::add(Expr::var("pos"), Expr::u32(1)),
                        Expr::min(
                            Expr::add(Expr::var("pos"), Expr::u32(MAX_SPARSE_TOKEN_SCAN)),
                            Expr::u32(haystack_len),
                        ),
                        vec![Node::if_then(
                            Expr::eq(Expr::var("sparse_char_done"), Expr::u32(0)),
                            vec![
                                Node::let_bind("scan_byte", byte_at(Expr::var("sparse_scan_char"))),
                                Node::assign(
                                    "tok_len",
                                    Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                                ),
                                Node::if_then_else(
                                    Expr::eq(Expr::var("sparse_char_literal_escape"), Expr::u32(1)),
                                    vec![Node::assign("sparse_char_literal_escape", Expr::u32(0))],
                                    vec![Node::if_then_else(
                                        byte_eq(Expr::var("scan_byte"), b'\\'),
                                        vec![Node::assign(
                                            "sparse_char_literal_escape",
                                            Expr::u32(1),
                                        )],
                                        vec![Node::if_then(
                                            byte_eq(Expr::var("scan_byte"), b'\''),
                                            vec![Node::assign("sparse_char_done", Expr::u32(1))],
                                        )],
                                    )],
                                ),
                            ],
                        )],
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var("sparse_char_done"), Expr::u32(0)),
                        vec![Node::assign(
                            "tok_type",
                            Expr::u32(TOK_ERR_UNTERMINATED_CHAR),
                        )],
                    ),
                ],
            ));
        }
    }
    if !suppress_span_readback {
        classify_at_pos.push(Node::if_then(
            Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_IDENTIFIER)),
            vec![
                Node::let_bind("sparse_ident_done", Expr::u32(0)),
                Node::loop_for(
                    "sparse_scan_ident",
                    Expr::add(Expr::var("pos"), Expr::u32(1)),
                    Expr::min(
                        Expr::add(Expr::var("pos"), Expr::u32(MAX_SPARSE_TOKEN_SCAN)),
                        Expr::u32(haystack_len),
                    ),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("sparse_ident_done"), Expr::u32(0)),
                        vec![
                            Node::let_bind("scan_byte", byte_at(Expr::var("sparse_scan_ident"))),
                            Node::if_then_else(
                                Expr::or(
                                    is_ident_continue(Expr::var("scan_byte")),
                                    byte_eq(Expr::var("scan_byte"), b'\''),
                                ),
                                vec![Node::assign(
                                    "tok_len",
                                    Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                                )],
                                vec![Node::assign("sparse_ident_done", Expr::u32(1))],
                            ),
                        ],
                    )],
                ),
            ],
        ));
    }
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
    if !suppress_span_readback {
        classify_at_pos.push(Node::if_then(
            Expr::or(
                Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_INTEGER)),
                Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_FLOAT)),
            ),
            vec![
                Node::let_bind("sparse_number_done", Expr::u32(0)),
                Node::let_bind(
                    "sparse_number_is_float",
                    Expr::select(
                        Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_FLOAT)),
                        Expr::u32(1),
                        Expr::u32(0),
                    ),
                ),
                Node::loop_for(
                    "sparse_scan_number",
                    Expr::add(Expr::var("pos"), Expr::u32(1)),
                    Expr::min(
                        Expr::add(Expr::var("pos"), Expr::u32(MAX_SPARSE_TOKEN_SCAN)),
                        Expr::u32(haystack_len),
                    ),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("sparse_number_done"), Expr::u32(0)),
                        vec![
                            Node::let_bind("scan_byte", byte_at(Expr::var("sparse_scan_number"))),
                            Node::let_bind(
                                "scan_prev",
                                byte_at(Expr::saturating_sub(
                                    Expr::var("sparse_scan_number"),
                                    Expr::u32(1),
                                )),
                            ),
                            Node::let_bind(
                                "scan_next",
                                byte_at(Expr::add(Expr::var("sparse_scan_number"), Expr::u32(1))),
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
                            Node::let_bind(
                                "scan_is_float_dot",
                                byte_eq(Expr::var("scan_byte"), b'.'),
                            ),
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
                                        vec![Node::assign("sparse_number_is_float", Expr::u32(1))],
                                    ),
                                ],
                                vec![Node::assign("sparse_number_done", Expr::u32(1))],
                            ),
                        ],
                    )],
                ),
                Node::if_then(
                    Expr::eq(Expr::var("sparse_number_is_float"), Expr::u32(1)),
                    vec![Node::assign("tok_type", Expr::u32(TOK_FLOAT))],
                ),
            ],
        ));
    }
    classify_at_pos.push(set_token(
        Expr::and(
            byte_eq(Expr::var("byte"), b'#'),
            Expr::eq(Expr::var("sparse_line_allows_directive"), Expr::u32(1)),
        ),
        TOK_PREPROC,
        Expr::u32(1),
    ));
    if !suppress_span_readback {
        classify_at_pos.push(Node::if_then(
            Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_PREPROC)),
            vec![
                Node::let_bind("sparse_preproc_done", Expr::u32(0)),
                Node::loop_for(
                    "sparse_scan_preproc",
                    Expr::add(Expr::var("pos"), Expr::u32(1)),
                    Expr::min(
                        Expr::add(Expr::var("pos"), Expr::u32(MAX_SPARSE_TOKEN_SCAN)),
                        Expr::u32(haystack_len),
                    ),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("sparse_preproc_done"), Expr::u32(0)),
                        vec![
                            Node::let_bind(
                                "sparse_preproc_scan_byte",
                                byte_at(Expr::var("sparse_scan_preproc")),
                            ),
                            Node::if_then_else(
                                Expr::or(
                                    byte_eq(Expr::var("sparse_preproc_scan_byte"), b'\n'),
                                    byte_eq(Expr::var("sparse_preproc_scan_byte"), b'\r'),
                                ),
                                vec![Node::assign("sparse_preproc_done", Expr::u32(1))],
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
    }
    classify_at_pos.extend(sections::operator_punct_pushes());
    let mut emit_stores = vec![Node::store(out_tok_types, t.clone(), Expr::var("tok_type"))];
    if !suppress_span_readback {
        emit_stores.push(Node::store(out_tok_starts, t.clone(), Expr::var("pos")));
        emit_stores.push(Node::store(out_tok_lens, t.clone(), Expr::var("tok_len")));
    }
    if emit_flags {
        emit_stores.push(Node::store(
            out_counts,
            t.clone(),
            Expr::var("sparse_visible_emit"),
        ));
    }
    classify_at_pos.push(Node::let_bind(
        "sparse_visible_emit",
        Expr::select(
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("emit"), Expr::u32(1)),
                    Expr::ne(Expr::var("tok_type"), Expr::u32(0)),
                ),
                Expr::ne(Expr::var("tok_type"), Expr::u32(TOK_COMMENT)),
            ),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));
    classify_at_pos.push(Node::if_then(
        Expr::eq(Expr::var("sparse_visible_emit"), Expr::u32(1)),
        emit_stores,
    ));
    if block_totals.is_some() {
        classify_at_pos.push(Node::store(
            "__sparse_lexer_block_count",
            Expr::LocalId { axis: 0 },
            Expr::var("sparse_visible_emit"),
        ));
    }

    let mut out_tok_starts_decl =
        BufferDecl::storage(out_tok_starts, 2, BufferAccess::ReadWrite, DataType::U32).with_count(
            if suppress_span_readback {
                1
            } else {
                haystack_len
            },
        );
    let mut out_tok_lens_decl =
        BufferDecl::storage(out_tok_lens, 3, BufferAccess::ReadWrite, DataType::U32).with_count(
            if suppress_span_readback {
                1
            } else {
                haystack_len
            },
        );
    if suppress_span_readback {
        out_tok_starts_decl = out_tok_starts_decl.with_output_byte_range(0..0);
        out_tok_lens_decl = out_tok_lens_decl.with_output_byte_range(0..0);
    }
    let mut out_counts_decl =
        BufferDecl::storage(out_counts, 4, BufferAccess::ReadWrite, DataType::U32)
            .with_count(if emit_flags { haystack_len } else { 1 });
    if !emit_flags {
        out_counts_decl = out_counts_decl.with_output_byte_range(0..0);
    }
    let is_start = Expr::and(
        Expr::var("sparse_preliminary_start"),
        Expr::and(
            Expr::eq(Expr::var("sparse_inside_string"), Expr::u32(0)),
            Expr::and(
                Expr::eq(Expr::var("sparse_inside_char"), Expr::u32(0)),
                Expr::and(
                    Expr::eq(Expr::var("sparse_inside_line_comment"), Expr::u32(0)),
                    Expr::and(
                        Expr::eq(Expr::var("sparse_inside_block_comment"), Expr::u32(0)),
                        Expr::eq(Expr::var("sparse_inside_preproc_line"), Expr::u32(0)),
                    ),
                ),
            ),
        ),
    );
    let region_body = if let Some(block_totals) = block_totals {
        let lane = Expr::var("lane");
        let block = Expr::var("block");
        let scratch_a = "__sparse_lexer_block_count";
        let scratch_b = "__sparse_lexer_block_count_reduce";
        let mut body = string_state_prefix.clone();
        body.push(Node::store(out_tok_types, t.clone(), Expr::u32(0)));
        if emit_flags {
            body.push(Node::store(out_counts, t.clone(), Expr::u32(0)));
        }
        body.extend([
            Node::let_bind("lane", Expr::LocalId { axis: 0 }),
            Node::let_bind("block", Expr::WorkgroupId { axis: 0 }),
            Node::store(scratch_a, lane.clone(), Expr::u32(0)),
            Node::if_then(is_start, classify_at_pos),
            Node::Barrier {
                ordering: vyre_foundation::memory_model::MemoryOrdering::SeqCst,
            },
        ]);
        let mut stride = 1_u32;
        while stride < workgroup_lanes {
            body.push(Node::store(
                scratch_b,
                lane.clone(),
                Expr::load(scratch_a, lane.clone()),
            ));
            let previous_lane = Expr::add(lane.clone(), Expr::u32(0u32.wrapping_sub(stride)));
            body.push(Node::if_then(
                Expr::lt(Expr::u32(stride.saturating_sub(1)), lane.clone()),
                vec![Node::store(
                    scratch_b,
                    lane.clone(),
                    Expr::add(
                        Expr::load(scratch_a, lane.clone()),
                        Expr::load(scratch_a, previous_lane),
                    ),
                )],
            ));
            body.push(Node::Barrier {
                ordering: vyre_foundation::memory_model::MemoryOrdering::SeqCst,
            });
            body.push(Node::store(
                scratch_a,
                lane.clone(),
                Expr::load(scratch_b, lane.clone()),
            ));
            body.push(Node::Barrier {
                ordering: vyre_foundation::memory_model::MemoryOrdering::SeqCst,
            });
            stride *= 2;
        }
        body.push(Node::if_then(
            Expr::eq(lane, Expr::u32(workgroup_lanes - 1)),
            vec![Node::store(
                block_totals,
                block,
                Expr::load(scratch_a, Expr::u32(workgroup_lanes - 1)),
            )],
        ));
        body
    } else {
        let mut body = string_state_prefix;
        body.push(Node::store(out_tok_types, t.clone(), Expr::u32(0)));
        if emit_flags {
            body.push(Node::store(out_counts, t.clone(), Expr::u32(0)));
        }
        body.push(Node::if_then(is_start, classify_at_pos));
        body
    };

    let sparse_types_decl = if block_totals.is_some() {
        BufferDecl::output(out_tok_types, 1, DataType::U32).with_count(haystack_len)
    } else {
        BufferDecl::storage(out_tok_types, 1, BufferAccess::ReadWrite, DataType::U32)
            .with_count(haystack_len)
    };
    if block_totals.is_some() {
        out_tok_starts_decl = BufferDecl::output(out_tok_starts, 2, DataType::U32).with_count(
            if suppress_span_readback {
                1
            } else {
                haystack_len
            },
        );
        out_tok_lens_decl = BufferDecl::output(out_tok_lens, 3, DataType::U32).with_count(
            if suppress_span_readback {
                1
            } else {
                haystack_len
            },
        );
    }
    let haystack_element = match haystack_layout {
        SparseHaystackLayout::RawU8 => DataType::U8,
        SparseHaystackLayout::PackedU32 | SparseHaystackLayout::ExpandedU32 => DataType::U32,
    };
    let haystack_count = match haystack_layout {
        SparseHaystackLayout::PackedU32 => haystack_len.max(1).div_ceil(4).max(1),
        SparseHaystackLayout::ExpandedU32 => haystack_len.max(1),
        SparseHaystackLayout::RawU8 => 0,
    };
    let mut buffers = vec![
        BufferDecl::storage(haystack, 0, BufferAccess::ReadOnly, haystack_element)
            .with_count(haystack_count),
        sparse_types_decl,
        out_tok_starts_decl,
        out_tok_lens_decl,
    ];
    if block_totals.is_none() || emit_flags {
        buffers.push(out_counts_decl);
    }
    if let Some(block_totals) = block_totals {
        buffers.push(
            BufferDecl::output(block_totals, if emit_flags { 5 } else { 4 }, DataType::U32)
                .with_count(haystack_len.div_ceil(workgroup_lanes).max(1)),
        );
        buffers.push(BufferDecl::workgroup(
            "__sparse_lexer_block_count",
            workgroup_lanes,
            DataType::U32,
        ));
        buffers.push(BufferDecl::workgroup(
            "__sparse_lexer_block_count_reduce",
            workgroup_lanes,
            DataType::U32,
        ));
    }
    Program::wrapped(
        buffers,
        [workgroup_lanes, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::c_lexer_regular_sparse",
            region_body,
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::c_lexer_regular_sparse")
    .with_non_composable_with_self(true)
}

#[cfg(test)]

mod tests {
    #[test]
    fn sparse_visible_emit_requires_nonzero_token_type() {
        let source = include_str!("sparse_impl.rs");
        assert!(
            source.contains("Expr::ne(Expr::var(\"tok_type\"), Expr::u32(0))"),
            "Fix: sparse lexer flags must not count zero token rows as visible tokens."
        );
    }
}
