use crate::parsing::composition::child_phase;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Sparse-token sentinel: non-token byte positions stay zeroed.
pub const TOK_NONE: u32 = 0;
/// Identifier token.
pub const TOK_IDENTIFIER: u32 = 1;
/// Number literal token.
pub const TOK_NUMBER: u32 = 2;
/// String literal token.
pub const TOK_STRING: u32 = 3;
/// Newline token.
pub const TOK_NEWLINE: u32 = 4;
/// Comment token.
pub const TOK_COMMENT: u32 = 5;

/// `(` token.
pub const TOK_LPAREN: u32 = 10;
/// `)` token.
pub const TOK_RPAREN: u32 = 11;
/// `[` token.
pub const TOK_LBRACKET: u32 = 12;
/// `]` token.
pub const TOK_RBRACKET: u32 = 13;
/// `{` token.
pub const TOK_LBRACE: u32 = 14;
/// `}` token.
pub const TOK_RBRACE: u32 = 15;
/// `:` token.
pub const TOK_COLON: u32 = 16;
/// `,` token.
pub const TOK_COMMA: u32 = 17;
/// `.` token.
pub const TOK_DOT: u32 = 18;
/// `=` token.
pub const TOK_EQ: u32 = 19;
/// `@` token.
pub const TOK_AT: u32 = 20;
/// `*` token.
pub const TOK_STAR: u32 = 21;

/// `def` keyword token.
pub const TOK_DEF: u32 = 100;
/// `async` keyword token.
pub const TOK_ASYNC: u32 = 101;
/// `class` keyword token.
pub const TOK_CLASS: u32 = 102;
/// `import` keyword token.
pub const TOK_IMPORT: u32 = 103;
/// `from` keyword token.
pub const TOK_FROM: u32 = 104;
/// `as` keyword token.
pub const TOK_AS: u32 = 105;
/// `with` keyword token.
pub const TOK_WITH: u32 = 106;
/// `await` keyword token.
pub const TOK_AWAIT: u32 = 107;
/// `match` keyword token.
pub const TOK_MATCH: u32 = 108;
/// `case` keyword token.
pub const TOK_CASE: u32 = 109;
/// `except` keyword token.
pub const TOK_EXCEPT: u32 = 110;

fn load_byte(buffer: &str, index: Expr) -> Expr {
    Expr::bitand(Expr::load(buffer, index), Expr::u32(0xFF))
}

fn ascii(ch: u8) -> Expr {
    Expr::u32(ch as u32)
}

fn is_between(value: Expr, start: u8, end: u8) -> Expr {
    Expr::and(
        Expr::ge(value.clone(), ascii(start)),
        Expr::le(value, ascii(end)),
    )
}

fn is_alpha(value: Expr) -> Expr {
    Expr::or(
        is_between(value.clone(), b'a', b'z'),
        is_between(value, b'A', b'Z'),
    )
}

fn is_ident_continue(value: Expr) -> Expr {
    Expr::or(
        Expr::or(
            is_alpha(value.clone()),
            is_between(value.clone(), b'0', b'9'),
        ),
        Expr::eq(value, ascii(b'_')),
    )
}

fn is_ident_start(value: Expr) -> Expr {
    Expr::or(is_alpha(value.clone()), Expr::eq(value, ascii(b'_')))
}

fn keyword_match(haystack: &str, base: Expr, len_var: &str, word: &[u8]) -> Expr {
    let mut expr = Expr::eq(Expr::var(len_var), Expr::u32(word.len() as u32));
    for (offset, byte) in word.iter().enumerate() {
        expr = Expr::and(
            expr,
            Expr::eq(
                load_byte(haystack, Expr::add(base.clone(), Expr::u32(offset as u32))),
                ascii(*byte),
            ),
        );
    }
    expr
}

fn classify_keyword(haystack: &str, base: Expr) -> Vec<Node> {
    vec![
        Node::if_then(
            keyword_match(haystack, base.clone(), "token_len", b"def"),
            vec![Node::assign("token_type", Expr::u32(TOK_DEF))],
        ),
        Node::if_then(
            keyword_match(haystack, base.clone(), "token_len", b"async"),
            vec![Node::assign("token_type", Expr::u32(TOK_ASYNC))],
        ),
        Node::if_then(
            keyword_match(haystack, base.clone(), "token_len", b"class"),
            vec![Node::assign("token_type", Expr::u32(TOK_CLASS))],
        ),
        Node::if_then(
            keyword_match(haystack, base.clone(), "token_len", b"import"),
            vec![Node::assign("token_type", Expr::u32(TOK_IMPORT))],
        ),
        Node::if_then(
            keyword_match(haystack, base.clone(), "token_len", b"from"),
            vec![Node::assign("token_type", Expr::u32(TOK_FROM))],
        ),
        Node::if_then(
            keyword_match(haystack, base.clone(), "token_len", b"as"),
            vec![Node::assign("token_type", Expr::u32(TOK_AS))],
        ),
        Node::if_then(
            keyword_match(haystack, base.clone(), "token_len", b"with"),
            vec![Node::assign("token_type", Expr::u32(TOK_WITH))],
        ),
        Node::if_then(
            keyword_match(haystack, base.clone(), "token_len", b"await"),
            vec![Node::assign("token_type", Expr::u32(TOK_AWAIT))],
        ),
        Node::if_then(
            keyword_match(haystack, base.clone(), "token_len", b"match"),
            vec![Node::assign("token_type", Expr::u32(TOK_MATCH))],
        ),
        Node::if_then(
            keyword_match(haystack, base.clone(), "token_len", b"case"),
            vec![Node::assign("token_type", Expr::u32(TOK_CASE))],
        ),
        Node::if_then(
            keyword_match(haystack, base, "token_len", b"except"),
            vec![Node::assign("token_type", Expr::u32(TOK_EXCEPT))],
        ),
    ]
}

/// GPU Python 3.12 sparse lexer.
///
/// Each invocation owns one byte offset. Token starts write their
/// classification to the same index; all other offsets stay zero.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn python312_lexer(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let body = vec![
        Node::let_bind("ch", load_byte(haystack, t.clone())),
        Node::let_bind(
            "prev",
            Expr::select(
                Expr::gt(t.clone(), Expr::u32(0)),
                load_byte(haystack, Expr::sub(t.clone(), Expr::u32(1))),
                Expr::u32(0),
            ),
        ),
        Node::let_bind("comment_scan_active", Expr::u32(1)),
        Node::let_bind("in_comment_tail", Expr::u32(0)),
        Node::loop_for(
            "comment_rev",
            Expr::u32(0),
            t.clone(),
            vec![Node::if_then(
                Expr::eq(Expr::var("comment_scan_active"), Expr::u32(1)),
                vec![
                    Node::let_bind(
                        "comment_pos",
                        Expr::sub(Expr::sub(t.clone(), Expr::u32(1)), Expr::var("comment_rev")),
                    ),
                    Node::let_bind("comment_ch", load_byte(haystack, Expr::var("comment_pos"))),
                    Node::if_then(
                        Expr::eq(Expr::var("comment_ch"), ascii(b'\n')),
                        vec![Node::assign("comment_scan_active", Expr::u32(0))],
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var("comment_ch"), ascii(b'#')),
                        vec![
                            Node::assign("in_comment_tail", Expr::u32(1)),
                            Node::assign("comment_scan_active", Expr::u32(0)),
                        ],
                    ),
                ],
            )],
        ),
        Node::let_bind("emit", Expr::u32(0)),
        Node::let_bind("token_type", Expr::u32(TOK_NONE)),
        Node::let_bind("token_len", Expr::u32(0)),
        // Store as u32(0|1) so later sites can `Expr::eq(_, Expr::u32(0))`
        // without the validator rejecting bool/u32 mismatches. The bool-
        // valued helpers `is_ident_start` / `is_ident_continue` return
        // genuine boolean exprs; coercing through `select` here keeps the
        // downstream call sites uniform with the surrounding u32 vars.
        Node::let_bind(
            "is_ident_start",
            Expr::select(is_ident_start(Expr::var("ch")), Expr::u32(1), Expr::u32(0)),
        ),
        Node::let_bind(
            "prev_identish",
            Expr::select(
                is_ident_continue(Expr::var("prev")),
                Expr::u32(1),
                Expr::u32(0),
            ),
        ),
        Node::if_then(
            Expr::eq(Expr::var("ch"), ascii(b'\n')),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_NEWLINE)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b'#')),
            ),
            vec![
                Node::let_bind("active", Expr::u32(1)),
                Node::let_bind("scan_len", Expr::u32(1)),
                Node::loop_for(
                    "j",
                    Expr::add(t.clone(), Expr::u32(1)),
                    Expr::u32(haystack_len),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("active"), Expr::u32(1)),
                        vec![
                            Node::let_bind("cur", load_byte(haystack, Expr::var("j"))),
                            Node::if_then_else(
                                Expr::eq(Expr::var("cur"), ascii(b'\n')),
                                vec![Node::assign("active", Expr::u32(0))],
                                vec![Node::assign(
                                    "scan_len",
                                    Expr::add(Expr::var("scan_len"), Expr::u32(1)),
                                )],
                            ),
                        ],
                    )],
                ),
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_COMMENT)),
                Node::assign("token_len", Expr::var("scan_len")),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::or(
                    Expr::eq(Expr::var("ch"), ascii(b'\'')),
                    Expr::eq(Expr::var("ch"), ascii(b'"')),
                ),
            ),
            vec![
                Node::let_bind("quote", Expr::var("ch")),
                Node::let_bind("active", Expr::u32(1)),
                Node::let_bind("escaped", Expr::u32(0)),
                Node::let_bind("scan_len", Expr::u32(1)),
                Node::loop_for(
                    "j",
                    Expr::add(t.clone(), Expr::u32(1)),
                    Expr::u32(haystack_len),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("active"), Expr::u32(1)),
                        vec![
                            Node::let_bind("cur", load_byte(haystack, Expr::var("j"))),
                            Node::assign(
                                "scan_len",
                                Expr::add(Expr::var("scan_len"), Expr::u32(1)),
                            ),
                            Node::if_then_else(
                                Expr::eq(Expr::var("escaped"), Expr::u32(1)),
                                vec![Node::assign("escaped", Expr::u32(0))],
                                vec![
                                    Node::if_then(
                                        Expr::eq(Expr::var("cur"), ascii(b'\\')),
                                        vec![Node::assign("escaped", Expr::u32(1))],
                                    ),
                                    Node::if_then(
                                        Expr::eq(Expr::var("cur"), Expr::var("quote")),
                                        vec![Node::assign("active", Expr::u32(0))],
                                    ),
                                ],
                            ),
                        ],
                    )],
                ),
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_STRING)),
                Node::assign("token_len", Expr::var("scan_len")),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::and(
                    Expr::eq(Expr::var("is_ident_start"), Expr::u32(1)),
                    Expr::eq(Expr::var("prev_identish"), Expr::u32(0)),
                ),
            ),
            vec![
                Node::let_bind("active", Expr::u32(1)),
                Node::let_bind("scan_len", Expr::u32(0)),
                Node::loop_for(
                    "j",
                    t.clone(),
                    Expr::u32(haystack_len),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("active"), Expr::u32(1)),
                        vec![
                            Node::let_bind("cur", load_byte(haystack, Expr::var("j"))),
                            Node::if_then_else(
                                is_ident_continue(Expr::var("cur")),
                                vec![Node::assign(
                                    "scan_len",
                                    Expr::add(Expr::var("scan_len"), Expr::u32(1)),
                                )],
                                vec![Node::assign("active", Expr::u32(0))],
                            ),
                        ],
                    )],
                ),
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_IDENTIFIER)),
                Node::assign("token_len", Expr::var("scan_len")),
            ]
            .into_iter()
            .chain(classify_keyword(haystack, t.clone()))
            .collect(),
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::and(
                    is_between(Expr::var("ch"), b'0', b'9'),
                    Expr::eq(Expr::var("prev_identish"), Expr::u32(0)),
                ),
            ),
            vec![
                Node::let_bind("active", Expr::u32(1)),
                Node::let_bind("scan_len", Expr::u32(0)),
                Node::loop_for(
                    "j",
                    t.clone(),
                    Expr::u32(haystack_len),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("active"), Expr::u32(1)),
                        vec![
                            Node::let_bind("cur", load_byte(haystack, Expr::var("j"))),
                            Node::if_then_else(
                                Expr::or(
                                    Expr::or(
                                        is_between(Expr::var("cur"), b'0', b'9'),
                                        Expr::eq(Expr::var("cur"), ascii(b'_')),
                                    ),
                                    Expr::eq(Expr::var("cur"), ascii(b'.')),
                                ),
                                vec![Node::assign(
                                    "scan_len",
                                    Expr::add(Expr::var("scan_len"), Expr::u32(1)),
                                )],
                                vec![Node::assign("active", Expr::u32(0))],
                            ),
                        ],
                    )],
                ),
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_NUMBER)),
                Node::assign("token_len", Expr::var("scan_len")),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b'(')),
            ),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_LPAREN)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b')')),
            ),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_RPAREN)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b'[')),
            ),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_LBRACKET)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b']')),
            ),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_RBRACKET)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b'{')),
            ),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_LBRACE)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b'}')),
            ),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_RBRACE)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b':')),
            ),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_COLON)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b',')),
            ),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_COMMA)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b'.')),
            ),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_DOT)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b'=')),
            ),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_EQ)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b'@')),
            ),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_AT)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                Expr::eq(Expr::var("ch"), ascii(b'*')),
            ),
            vec![
                Node::assign("emit", Expr::u32(1)),
                Node::assign("token_type", Expr::u32(TOK_STAR)),
                Node::assign("token_len", Expr::u32(1)),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("in_comment_tail"), Expr::u32(1)),
                Expr::ne(Expr::var("ch"), ascii(b'\n')),
            ),
            vec![Node::assign("emit", Expr::u32(0))],
        ),
        Node::if_then(
            Expr::eq(Expr::var("emit"), Expr::u32(1)),
            vec![
                Node::store(out_tok_types, t.clone(), Expr::var("token_type")),
                Node::store(out_tok_starts, t.clone(), t.clone()),
                Node::store(out_tok_lens, t.clone(), Expr::var("token_len")),
                Node::let_bind(
                    "token_slot",
                    Expr::atomic_add(out_counts, Expr::u32(0), Expr::u32(1)),
                ),
                Node::assign("token_slot", Expr::var("token_slot")),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(haystack, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(haystack_len),
            BufferDecl::storage(out_tok_types, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(haystack_len),
            BufferDecl::storage(out_tok_starts, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(haystack_len),
            BufferDecl::storage(out_tok_lens, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(haystack_len),
            BufferDecl::storage(out_counts, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::python312_lexer",
            vec![child_phase(
                "vyre-libs::parsing::python312_lexer",
                vyre_primitives::text::line_index::OP_ID,
                vec![Node::if_then(
                    Expr::lt(t.clone(), Expr::u32(haystack_len)),
                    body,
                )],
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::python312_lexer")
    .with_non_composable_with_self(true)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::python312_lexer",
        build: || python312_lexer("haystack", "tok_types", "tok_starts", "tok_lens", "counts", 16),
        test_inputs: Some(lexer_fixture_inputs),
        expected_output: Some(lexer_fixture_expected),
        category: Some("parsing"),
    }
}

fn lexer_fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    let source = b"def f(x):\n#z\n";
    let mut haystack = vec![0u8; 16 * 4];
    for (idx, byte) in source.iter().enumerate() {
        haystack[idx * 4..idx * 4 + 4].copy_from_slice(&u32::from(*byte).to_le_bytes());
    }
    vec![vec![
        haystack,
        vec![0u8; 16 * 4],
        vec![0u8; 16 * 4],
        vec![0u8; 16 * 4],
        vec![0u8; 4],
    ]]
}

fn write_sparse_token(
    tok_types: &mut [u8],
    tok_starts: &mut [u8],
    tok_lens: &mut [u8],
    pos: usize,
    tok: u32,
    len: u32,
) {
    let base = pos * 4;
    tok_types[base..base + 4].copy_from_slice(&tok.to_le_bytes());
    tok_starts[base..base + 4].copy_from_slice(&(pos as u32).to_le_bytes());
    tok_lens[base..base + 4].copy_from_slice(&len.to_le_bytes());
}

fn lexer_fixture_expected() -> Vec<Vec<Vec<u8>>> {
    let mut tok_types = vec![0u8; 16 * 4];
    let mut tok_starts = vec![0u8; 16 * 4];
    let mut tok_lens = vec![0u8; 16 * 4];
    for (pos, tok, len) in [
        (0usize, TOK_DEF, 3u32),
        (4, TOK_IDENTIFIER, 1),
        (5, TOK_LPAREN, 1),
        (6, TOK_IDENTIFIER, 1),
        (7, TOK_RPAREN, 1),
        (8, TOK_COLON, 1),
        (9, TOK_NEWLINE, 1),
        (10, TOK_COMMENT, 2),
        (12, TOK_NEWLINE, 1),
    ] {
        write_sparse_token(
            &mut tok_types,
            &mut tok_starts,
            &mut tok_lens,
            pos,
            tok,
            len,
        );
    }

    vec![vec![
        tok_types,
        tok_starts,
        tok_lens,
        9u32.to_le_bytes().to_vec(),
    ]]
}
