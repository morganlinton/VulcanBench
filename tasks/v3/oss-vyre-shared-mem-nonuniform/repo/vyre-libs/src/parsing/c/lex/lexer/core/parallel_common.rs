use super::*;
use crate::parsing::c::lex::lexer::sections;

pub(super) const REGULAR_PARALLEL_WORKGROUP_SIZE: u32 = 256;

#[derive(Clone, Copy)]
pub(super) enum RegularParallelMode {
    Ranked,
    Sparse,
}

impl RegularParallelMode {
    fn scan_prefix(self) -> &'static str {
        match self {
            RegularParallelMode::Ranked => "ranked",
            RegularParallelMode::Sparse => "sparse",
        }
    }
}

pub(super) fn regular_parallel_byte_at(haystack: &str, haystack_len: u32, index: Expr) -> Expr {
    byte_at_or_zero(haystack, index, haystack_len)
}

fn regular_parallel_space_expr(value: Expr) -> Expr {
    Expr::or(
        byte_eq(value.clone(), b' '),
        Expr::or(
            byte_eq(value.clone(), b'\n'),
            Expr::or(byte_eq(value.clone(), b'\r'), byte_eq(value, b'\t')),
        ),
    )
}

fn regular_parallel_operator_tail_expr(haystack: &str, haystack_len: u32, index: Expr) -> Expr {
    let byte_at = |idx: Expr| regular_parallel_byte_at(haystack, haystack_len, idx);
    let b = byte_at(index.clone());
    let prev = Expr::select(
        Expr::gt(index.clone(), Expr::u32(0)),
        byte_at(Expr::saturating_sub(index.clone(), Expr::u32(1))),
        Expr::u32(0),
    );
    let prev2 = Expr::select(
        Expr::gt(index.clone(), Expr::u32(1)),
        byte_at(Expr::saturating_sub(index, Expr::u32(2))),
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
                Expr::and(byte_eq(b.clone(), b'+'), byte_eq(prev.clone(), b'+')),
                Expr::or(
                    Expr::and(byte_eq(b.clone(), b'-'), byte_eq(prev.clone(), b'-')),
                    Expr::or(
                        Expr::and(byte_eq(b.clone(), b'&'), byte_eq(prev.clone(), b'&')),
                        Expr::or(
                            Expr::and(byte_eq(b.clone(), b'|'), byte_eq(prev.clone(), b'|')),
                            Expr::or(
                                Expr::and(byte_eq(b.clone(), b'<'), byte_eq(prev.clone(), b'<')),
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
                                            Expr::and(byte_eq(prev, b'>'), byte_eq(prev2, b'>')),
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
}

pub(super) fn regular_parallel_token_start_expr(
    haystack: &str,
    haystack_len: u32,
    index: Expr,
) -> Expr {
    let byte_at = |idx: Expr| regular_parallel_byte_at(haystack, haystack_len, idx);
    let b = byte_at(index.clone());
    let prev = Expr::select(
        Expr::gt(index.clone(), Expr::u32(0)),
        byte_at(Expr::saturating_sub(index.clone(), Expr::u32(1))),
        Expr::u32(0),
    );
    Expr::and(
        Expr::lt(index.clone(), Expr::buf_len(haystack)),
        Expr::and(
            Expr::not(regular_parallel_space_expr(b.clone())),
            Expr::and(
                Expr::not(Expr::and(is_ident_continue(b), is_ident_continue(prev))),
                Expr::not(regular_parallel_operator_tail_expr(
                    haystack,
                    haystack_len,
                    index,
                )),
            ),
        ),
    )
}

pub(super) fn regular_parallel_classifier(
    haystack: &str,
    haystack_len: u32,
    t: Expr,
    mode: RegularParallelMode,
) -> Vec<Node> {
    let byte_at = |idx: Expr| regular_parallel_byte_at(haystack, haystack_len, idx);
    let prefix = mode.scan_prefix();
    let ident_done = format!("{prefix}_ident_done");
    let ident_scan = format!("{prefix}_scan_ident");
    let number_done = format!("{prefix}_number_done");
    let number_scan = format!("{prefix}_scan_number");

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

    if matches!(mode, RegularParallelMode::Ranked) {
        classify_at_pos.push(Node::let_bind("rank", Expr::u32(0)));
        classify_at_pos.push(Node::loop_for(
            "rank_scan",
            Expr::u32(0),
            t.clone(),
            vec![Node::if_then(
                regular_parallel_token_start_expr(haystack, haystack_len, Expr::var("rank_scan")),
                vec![Node::assign(
                    "rank",
                    Expr::add(Expr::var("rank"), Expr::u32(1)),
                )],
            )],
        ));
    }

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
            Node::let_bind(&ident_done, Expr::u32(0)),
            Node::loop_for(
                &ident_scan,
                Expr::add(Expr::var("pos"), Expr::u32(1)),
                scan_upper_bound_with_cap(
                    haystack,
                    Expr::add(Expr::var("pos"), Expr::u32(1)),
                    MAX_IDENT_SCAN,
                ),
                vec![Node::if_then(
                    Expr::eq(Expr::var(&ident_done), Expr::u32(0)),
                    vec![
                        Node::let_bind("scan_byte", byte_at(Expr::var(&ident_scan))),
                        Node::if_then_else(
                            is_ident_continue(Expr::var("scan_byte")),
                            vec![Node::assign(
                                "tok_len",
                                Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                            )],
                            vec![Node::assign(&ident_done, Expr::u32(1))],
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
    classify_at_pos.push(Node::if_then(
        Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_INTEGER)),
        vec![
            Node::let_bind(&number_done, Expr::u32(0)),
            Node::loop_for(
                &number_scan,
                Expr::add(Expr::var("pos"), Expr::u32(1)),
                scan_upper_bound_with_cap(
                    haystack,
                    Expr::add(Expr::var("pos"), Expr::u32(1)),
                    MAX_NUMBER_SCAN,
                ),
                vec![Node::if_then(
                    Expr::eq(Expr::var(&number_done), Expr::u32(0)),
                    vec![
                        Node::let_bind("scan_byte", byte_at(Expr::var(&number_scan))),
                        Node::if_then_else(
                            is_digit(Expr::var("scan_byte")),
                            vec![Node::assign(
                                "tok_len",
                                Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                            )],
                            vec![Node::assign(&number_done, Expr::u32(1))],
                        ),
                    ],
                )],
            ),
        ],
    ));
    classify_at_pos.extend(sections::operator_punct_pushes());
    classify_at_pos
}

pub(super) fn regular_parallel_buffers(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Vec<BufferDecl> {
    vec![
        BufferDecl::storage(haystack, 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(haystack_len.max(1)),
        BufferDecl::storage(out_tok_types, 1, BufferAccess::ReadWrite, DataType::U32)
            .with_count(haystack_len.max(1)),
        BufferDecl::storage(out_tok_starts, 2, BufferAccess::ReadWrite, DataType::U32)
            .with_count(haystack_len.max(1)),
        BufferDecl::storage(out_tok_lens, 3, BufferAccess::ReadWrite, DataType::U32)
            .with_count(haystack_len.max(1)),
        BufferDecl::storage(out_counts, 4, BufferAccess::ReadWrite, DataType::U32).with_count(1),
    ]
}
