//! Vyre IR program for Rust nano-subset tokenization.

mod batch;
mod expr;

pub use batch::rust_lexer_batch;

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::parsing::rust::lex::tokens::*;

use expr::{
    byte_before_or_zero, byte_eq, byte_load, is_ascii_whitespace, is_digit, is_ident_continue,
    is_ident_start, keyword_or_ident_until, unhandled,
};

pub(super) const WORKGROUP_SIZE: u32 = 256;
const MAX_TOKEN_LEN: u32 = u16::MAX as u32;

/// Build a compacting lexer for the Rust nano-subset.
///
/// Outputs are `u32` columns: token kind, start byte, byte length, and a single
/// count word. The emitted stream includes the terminal EOF token so it can feed
/// the existing parser shape directly after host-side decoding.
#[must_use]
pub fn rust_lexer(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    let lane = Expr::InvocationId { axis: 0 };
    let source_len = Expr::u32(haystack_len);
    let token_capacity = haystack_len.saturating_add(1).max(1);

    let mut body = vec![
        Node::let_bind("cursor", Expr::u32(0)),
        Node::let_bind("tok_idx", Expr::u32(0)),
        Node::loop_for(
            "scan_iter",
            Expr::u32(0),
            Expr::add(source_len.clone(), Expr::u32(1)),
            vec![Node::if_then(
                Expr::lt(Expr::var("cursor"), source_len.clone()),
                scan_one_token(
                    haystack,
                    Expr::u32(0),
                    source_len.clone(),
                    Expr::u32(0),
                    out_tok_types,
                    out_tok_starts,
                    out_tok_lens,
                ),
            )],
        ),
    ];
    body.extend(emit_token(
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        Expr::u32(0),
        Expr::u32(u32::from(EOF)),
        source_len,
        Expr::u32(0),
    ));
    body.push(Node::store(out_counts, Expr::u32(0), Expr::var("tok_idx")));

    Program::wrapped(
        vec![
            BufferDecl::storage(haystack, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(haystack_len.max(1)),
            BufferDecl::storage(out_tok_types, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(token_capacity),
            BufferDecl::storage(out_tok_starts, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(token_capacity),
            BufferDecl::storage(out_tok_lens, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(token_capacity),
            BufferDecl::storage(out_counts, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [WORKGROUP_SIZE, 1, 1],
        vec![Node::if_then(Expr::eq(lane, Expr::u32(0)), body)],
    )
    .with_entry_op_id("vyre-libs::parsing::rust_lexer")
    .with_non_composable_with_self(true)
}

pub(super) fn scan_one_token(
    haystack: &str,
    source_start: Expr,
    source_end: Expr,
    token_index_base: Expr,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
) -> Vec<Node> {
    let pos = Expr::var("cursor");
    let byte = byte_load(haystack, pos.clone());
    let next = byte_before_or_zero(
        haystack,
        source_end.clone(),
        Expr::add(pos.clone(), Expr::u32(1)),
    );
    let next2 = byte_before_or_zero(
        haystack,
        source_end.clone(),
        Expr::add(pos.clone(), Expr::u32(2)),
    );
    let next3 = byte_before_or_zero(
        haystack,
        source_end.clone(),
        Expr::add(pos.clone(), Expr::u32(3)),
    );

    let mut nodes = vec![
        Node::let_bind("pos", pos.clone()),
        Node::let_bind("byte", byte),
        Node::let_bind("next_byte", next),
        Node::let_bind("next2_byte", next2),
        Node::let_bind("next3_byte", next3),
        Node::let_bind("handled", Expr::u32(0)),
        Node::let_bind("tok_type", Expr::u32(u32::from(ERROR))),
        Node::let_bind("tok_len", Expr::u32(1)),
    ];

    nodes.push(Node::if_then(
        Expr::and(unhandled(), is_ascii_whitespace(Expr::var("byte"))),
        vec![
            Node::assign("cursor", Expr::add(Expr::var("cursor"), Expr::u32(1))),
            Node::assign("handled", Expr::u32(1)),
        ],
    ));

    nodes.push(Node::if_then(
        Expr::and(
            unhandled(),
            Expr::and(
                byte_eq(Expr::var("byte"), b'/'),
                byte_eq(Expr::var("next_byte"), b'/'),
            ),
        ),
        line_comment_skip(haystack, source_end.clone()),
    ));

    nodes.push(Node::if_then(
        Expr::and(
            unhandled(),
            Expr::and(
                byte_eq(Expr::var("byte"), b'/'),
                byte_eq(Expr::var("next_byte"), b'*'),
            ),
        ),
        block_comment_skip(haystack, source_end.clone()),
    ));

    nodes.push(Node::if_then(
        Expr::and(unhandled(), is_ident_start(Expr::var("byte"))),
        scan_identifier(haystack, source_end.clone()),
    ));

    nodes.push(Node::if_then(
        Expr::and(unhandled(), is_digit(Expr::var("byte"))),
        scan_integer(haystack, source_end.clone()),
    ));

    nodes.extend(operator_classifiers());

    nodes.push(Node::if_then(unhandled(), {
        let mut emit = Vec::new();
        emit.extend(emit_token(
            out_tok_types,
            out_tok_starts,
            out_tok_lens,
            token_index_base.clone(),
            Expr::var("tok_type"),
            Expr::sub(Expr::var("pos"), source_start.clone()),
            Expr::var("tok_len"),
        ));
        emit.push(Node::assign(
            "cursor",
            Expr::add(Expr::var("cursor"), Expr::var("tok_len")),
        ));
        emit.push(Node::assign("handled", Expr::u32(1)));
        emit
    }));

    nodes
}

fn line_comment_skip(haystack: &str, source_end: Expr) -> Vec<Node> {
    vec![
        Node::assign("cursor", Expr::add(Expr::var("cursor"), Expr::u32(2))),
        Node::let_bind("comment_done", Expr::u32(0)),
        Node::loop_for(
            "line_comment_i",
            Expr::var("cursor"),
            source_end,
            vec![Node::if_then(
                Expr::eq(Expr::var("comment_done"), Expr::u32(0)),
                vec![
                    Node::let_bind(
                        "comment_byte",
                        byte_load(haystack, Expr::var("line_comment_i")),
                    ),
                    Node::if_then_else(
                        byte_eq(Expr::var("comment_byte"), b'\n'),
                        vec![
                            Node::assign("cursor", Expr::var("line_comment_i")),
                            Node::assign("comment_done", Expr::u32(1)),
                        ],
                        vec![Node::assign(
                            "cursor",
                            Expr::add(Expr::var("line_comment_i"), Expr::u32(1)),
                        )],
                    ),
                ],
            )],
        ),
        Node::assign("handled", Expr::u32(1)),
    ]
}

fn block_comment_skip(haystack: &str, source_end: Expr) -> Vec<Node> {
    vec![
        Node::assign("cursor", Expr::add(Expr::var("cursor"), Expr::u32(2))),
        Node::let_bind("block_done", Expr::u32(0)),
        Node::loop_for(
            "block_comment_i",
            Expr::var("cursor"),
            source_end.clone(),
            vec![Node::if_then(
                Expr::eq(Expr::var("block_done"), Expr::u32(0)),
                vec![
                    Node::let_bind(
                        "block_byte",
                        byte_load(haystack, Expr::var("block_comment_i")),
                    ),
                    Node::let_bind(
                        "block_next",
                        byte_before_or_zero(
                            haystack,
                            source_end.clone(),
                            Expr::add(Expr::var("block_comment_i"), Expr::u32(1)),
                        ),
                    ),
                    Node::if_then_else(
                        Expr::and(
                            byte_eq(Expr::var("block_byte"), b'*'),
                            byte_eq(Expr::var("block_next"), b'/'),
                        ),
                        vec![
                            Node::assign(
                                "cursor",
                                Expr::add(Expr::var("block_comment_i"), Expr::u32(2)),
                            ),
                            Node::assign("block_done", Expr::u32(1)),
                        ],
                        vec![Node::assign(
                            "cursor",
                            Expr::add(Expr::var("block_comment_i"), Expr::u32(1)),
                        )],
                    ),
                ],
            )],
        ),
        Node::assign("handled", Expr::u32(1)),
    ]
}

fn scan_identifier(haystack: &str, source_end: Expr) -> Vec<Node> {
    vec![
        Node::assign("tok_type", Expr::u32(u32::from(IDENT))),
        Node::assign("tok_len", Expr::u32(1)),
        Node::let_bind("ident_done", Expr::u32(0)),
        Node::loop_for(
            "ident_i",
            Expr::add(Expr::var("pos"), Expr::u32(1)),
            source_end.clone(),
            vec![Node::if_then(
                Expr::eq(Expr::var("ident_done"), Expr::u32(0)),
                vec![
                    Node::let_bind("ident_byte", byte_load(haystack, Expr::var("ident_i"))),
                    Node::if_then_else(
                        is_ident_continue(Expr::var("ident_byte")),
                        vec![Node::assign(
                            "tok_len",
                            Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                        )],
                        vec![Node::assign("ident_done", Expr::u32(1))],
                    ),
                ],
            )],
        ),
        Node::assign(
            "tok_type",
            Expr::select(
                Expr::gt(Expr::var("tok_len"), Expr::u32(MAX_TOKEN_LEN)),
                Expr::u32(u32::from(ERROR)),
                keyword_or_ident_until(
                    haystack,
                    source_end,
                    Expr::var("pos"),
                    Expr::var("tok_len"),
                ),
            ),
        ),
    ]
}

fn scan_integer(haystack: &str, source_end: Expr) -> Vec<Node> {
    vec![
        Node::assign("tok_type", Expr::u32(u32::from(LITERAL_INT))),
        Node::assign("tok_len", Expr::u32(1)),
        Node::let_bind("int_done", Expr::u32(0)),
        Node::loop_for(
            "int_i",
            Expr::add(Expr::var("pos"), Expr::u32(1)),
            source_end,
            vec![Node::if_then(
                Expr::eq(Expr::var("int_done"), Expr::u32(0)),
                vec![
                    Node::let_bind("int_byte", byte_load(haystack, Expr::var("int_i"))),
                    Node::if_then_else(
                        is_digit(Expr::var("int_byte")),
                        vec![Node::assign(
                            "tok_len",
                            Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                        )],
                        vec![Node::assign("int_done", Expr::u32(1))],
                    ),
                ],
            )],
        ),
        Node::assign(
            "tok_type",
            Expr::select(
                Expr::gt(Expr::var("tok_len"), Expr::u32(MAX_TOKEN_LEN)),
                Expr::u32(u32::from(ERROR)),
                Expr::u32(u32::from(LITERAL_INT)),
            ),
        ),
    ]
}

fn operator_classifiers() -> Vec<Node> {
    let cases: &[(u8, u8, u16, u32)] = &[
        (b'=', b'=', EQ, 2),
        (b'+', b'=', PLUS_EQ, 2),
        (b'-', b'=', MINUS_EQ, 2),
        (b'<', b'=', LE, 2),
        (b'>', b'=', GE, 2),
        (b'!', b'=', NE, 2),
        (b'&', b'&', ANDAND, 2),
        (b'|', b'|', OROR, 2),
        (b'-', b'>', ARROW, 2),
        (b'.', b'.', DOTDOT, 2),
    ];
    let mut nodes = Vec::new();
    for &(a, b, kind, len) in cases {
        nodes.push(set_token(
            Expr::and(
                byte_eq(Expr::var("byte"), a),
                byte_eq(Expr::var("next_byte"), b),
            ),
            kind,
            len,
        ));
    }
    nodes.push(set_token(
        Expr::and(
            Expr::and(
                byte_eq(Expr::var("byte"), b'&'),
                byte_eq(Expr::var("next_byte"), b'm'),
            ),
            Expr::and(
                byte_eq(Expr::var("next2_byte"), b'u'),
                byte_eq(Expr::var("next3_byte"), b't'),
            ),
        ),
        AMP_MUT,
        4,
    ));

    for &(byte, kind) in &[
        (b'+', PLUS),
        (b'-', MINUS),
        (b'*', STAR),
        (b'/', SLASH),
        (b'%', PERCENT),
        (b'=', ASSIGN),
        (b'<', LT),
        (b'>', GT),
        (b';', SEMI),
        (b':', COLON),
        (b',', COMMA),
        (b'&', AMP),
        (b'!', BANG),
        (b'(', LPAREN),
        (b')', RPAREN),
        (b'{', LBRACE),
        (b'}', RBRACE),
    ] {
        nodes.push(set_token(byte_eq(Expr::var("byte"), byte), kind, 1));
    }
    nodes
}

fn set_token(cond: Expr, kind: u16, len: u32) -> Node {
    Node::if_then(
        Expr::and(
            unhandled(),
            Expr::and(
                Expr::eq(Expr::var("tok_type"), Expr::u32(u32::from(ERROR))),
                cond,
            ),
        ),
        vec![
            Node::assign("tok_type", Expr::u32(u32::from(kind))),
            Node::assign("tok_len", Expr::u32(len)),
        ],
    )
}

pub(super) fn emit_token(
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    token_index_base: Expr,
    kind: Expr,
    start: Expr,
    len: Expr,
) -> Vec<Node> {
    let token_index = Expr::add(token_index_base, Expr::var("tok_idx"));
    vec![
        Node::store(out_tok_types, token_index.clone(), kind),
        Node::store(out_tok_starts, token_index.clone(), start),
        Node::store(out_tok_lens, token_index, len),
        Node::assign("tok_idx", Expr::add(Expr::var("tok_idx"), Expr::u32(1))),
    ]
}
