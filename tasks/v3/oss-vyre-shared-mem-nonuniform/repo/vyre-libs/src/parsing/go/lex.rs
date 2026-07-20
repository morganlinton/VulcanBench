use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Sparse-token sentinel: non-token byte positions stay zeroed.
pub const TOK_NONE: u32 = 0;
/// Identifier token.
pub const TOK_IDENTIFIER: u32 = 1;
/// Double-quoted string literal token.
pub const TOK_STRING: u32 = 2;
/// `(` token.
pub const TOK_LPAREN: u32 = 10;
/// `)` token.
pub const TOK_RPAREN: u32 = 11;
/// `{` token.
pub const TOK_LBRACE: u32 = 12;
/// `}` token.
pub const TOK_RBRACE: u32 = 13;
/// `[` token.
pub const TOK_LBRACKET: u32 = 14;
/// `]` token.
pub const TOK_RBRACKET: u32 = 15;
/// `,` token.
pub const TOK_COMMA: u32 = 16;
/// `.` token.
pub const TOK_DOT: u32 = 17;
/// `;` token.
pub const TOK_SEMICOLON: u32 = 18;
/// `:` token.
pub const TOK_COLON: u32 = 19;
/// `=` token.
pub const TOK_ASSIGN: u32 = 20;
/// `*` token.
pub const TOK_STAR: u32 = 21;
/// `<-` token.
pub const TOK_ARROW: u32 = 22;

fn byte_load(buffer: &str, index: Expr) -> Expr {
    Expr::bitand(Expr::load(buffer, index), Expr::u32(0xFF))
}

fn byte_eq(expr: Expr, byte: u8) -> Expr {
    Expr::eq(expr, Expr::u32(u32::from(byte)))
}

fn byte_between(expr: Expr, low: u8, high: u8) -> Expr {
    Expr::and(
        Expr::ge(expr.clone(), Expr::u32(u32::from(low))),
        Expr::le(expr, Expr::u32(u32::from(high))),
    )
}

fn ident_start(expr: Expr) -> Expr {
    Expr::or(
        Expr::or(
            byte_between(expr.clone(), b'a', b'z'),
            byte_between(expr.clone(), b'A', b'Z'),
        ),
        byte_eq(expr, b'_'),
    )
}

fn ident_continue(expr: Expr) -> Expr {
    Expr::or(ident_start(expr.clone()), byte_between(expr, b'0', b'9'))
}

fn punctuation_token(byte: Expr, token: u32, chr: u8) -> Vec<Node> {
    vec![Node::if_then(
        byte_eq(byte, chr),
        vec![
            Node::assign("emit", Expr::u32(1)),
            Node::assign("tok_type", Expr::u32(token)),
            Node::assign("tok_len", Expr::u32(1)),
        ],
    )]
}

/// Byte-oriented Go lexer over a `u32`-encoded byte stream.
///
/// Each invocation owns one source byte and emits at most one token. Identifiers
/// and string literals are maximally munched by a forward scan from the start
/// byte. Punctuation is emitted as fixed-width one-byte tokens, with `<-`
/// treated as a dedicated channel operator token.
#[must_use]
pub fn go_lexer(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    let t = Expr::gid_x();

    let mut body = vec![
        Node::let_bind("byte", byte_load(haystack, t.clone())),
        Node::let_bind(
            "prev_byte",
            Expr::select(
                Expr::gt(t.clone(), Expr::u32(0)),
                byte_load(haystack, Expr::sub(t.clone(), Expr::u32(1))),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "next_byte",
            Expr::select(
                Expr::lt(Expr::add(t.clone(), Expr::u32(1)), Expr::u32(haystack_len)),
                byte_load(haystack, Expr::add(t.clone(), Expr::u32(1))),
                Expr::u32(0),
            ),
        ),
        Node::let_bind("emit", Expr::u32(0)),
        Node::let_bind("tok_type", Expr::u32(TOK_NONE)),
        Node::let_bind("tok_len", Expr::u32(0)),
    ];

    body.push(Node::if_then(
        Expr::and(
            ident_start(Expr::var("byte")),
            Expr::not(ident_continue(Expr::var("prev_byte"))),
        ),
        vec![
            Node::assign("emit", Expr::u32(1)),
            Node::assign("tok_type", Expr::u32(TOK_IDENTIFIER)),
            Node::assign("tok_len", Expr::u32(1)),
            Node::let_bind("still_ident", Expr::u32(1)),
            Node::loop_for(
                "scan",
                Expr::add(t.clone(), Expr::u32(1)),
                Expr::u32(haystack_len),
                vec![Node::if_then(
                    Expr::eq(Expr::var("still_ident"), Expr::u32(1)),
                    vec![
                        Node::let_bind("scan_byte", byte_load(haystack, Expr::var("scan"))),
                        Node::if_then_else(
                            ident_continue(Expr::var("scan_byte")),
                            vec![Node::assign(
                                "tok_len",
                                Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                            )],
                            vec![Node::assign("still_ident", Expr::u32(0))],
                        ),
                    ],
                )],
            ),
        ],
    ));

    body.push(Node::if_then(
        byte_eq(Expr::var("byte"), b'"'),
        vec![
            Node::assign("emit", Expr::u32(1)),
            Node::assign("tok_type", Expr::u32(TOK_STRING)),
            Node::assign("tok_len", Expr::u32(1)),
            Node::let_bind("string_done", Expr::u32(0)),
            Node::loop_for(
                "scan",
                Expr::add(t.clone(), Expr::u32(1)),
                Expr::u32(haystack_len),
                vec![Node::if_then(
                    Expr::eq(Expr::var("string_done"), Expr::u32(0)),
                    vec![
                        Node::assign("tok_len", Expr::add(Expr::var("tok_len"), Expr::u32(1))),
                        Node::let_bind("scan_byte", byte_load(haystack, Expr::var("scan"))),
                        Node::if_then(
                            byte_eq(Expr::var("scan_byte"), b'"'),
                            vec![Node::assign("string_done", Expr::u32(1))],
                        ),
                    ],
                )],
            ),
        ],
    ));

    body.push(Node::if_then(
        Expr::and(
            byte_eq(Expr::var("byte"), b'<'),
            byte_eq(Expr::var("next_byte"), b'-'),
        ),
        vec![
            Node::assign("emit", Expr::u32(1)),
            Node::assign("tok_type", Expr::u32(TOK_ARROW)),
            Node::assign("tok_len", Expr::u32(2)),
        ],
    ));

    body.extend(punctuation_token(Expr::var("byte"), TOK_LPAREN, b'('));
    body.extend(punctuation_token(Expr::var("byte"), TOK_RPAREN, b')'));
    body.extend(punctuation_token(Expr::var("byte"), TOK_LBRACE, b'{'));
    body.extend(punctuation_token(Expr::var("byte"), TOK_RBRACE, b'}'));
    body.extend(punctuation_token(Expr::var("byte"), TOK_LBRACKET, b'['));
    body.extend(punctuation_token(Expr::var("byte"), TOK_RBRACKET, b']'));
    body.extend(punctuation_token(Expr::var("byte"), TOK_COMMA, b','));
    body.extend(punctuation_token(Expr::var("byte"), TOK_DOT, b'.'));
    body.extend(punctuation_token(Expr::var("byte"), TOK_SEMICOLON, b';'));
    body.extend(punctuation_token(Expr::var("byte"), TOK_COLON, b':'));
    body.extend(punctuation_token(Expr::var("byte"), TOK_ASSIGN, b'='));
    body.extend(punctuation_token(Expr::var("byte"), TOK_STAR, b'*'));

    body.push(Node::if_then(
        Expr::eq(Expr::var("emit"), Expr::u32(1)),
        vec![
            Node::let_bind(
                "tok_idx",
                Expr::atomic_add(out_counts, Expr::u32(0), Expr::u32(1)),
            ),
            Node::store(out_tok_types, Expr::var("tok_idx"), Expr::var("tok_type")),
            Node::store(out_tok_starts, Expr::var("tok_idx"), t.clone()),
            Node::store(out_tok_lens, Expr::var("tok_idx"), Expr::var("tok_len")),
        ],
    ));

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
            "vyre-libs::parsing::go_lexer",
            vec![Node::if_then(Expr::lt(t, Expr::u32(haystack_len)), body)],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::go_lexer")
    .with_non_composable_with_self(true)
    .with_entry_op_id("vyre-libs::parsing::go_lexer")
    .with_non_composable_with_self(true)
}
