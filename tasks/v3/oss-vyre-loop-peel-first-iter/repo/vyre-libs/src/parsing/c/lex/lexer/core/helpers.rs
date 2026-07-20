use super::*;
use crate::parsing::c::lex::lexer::sections;

pub fn c11_lexer_regular(
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
    classify_at_pos.push(Node::if_then(
        Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_INTEGER)),
        vec![
            Node::let_bind("number_done", Expr::u32(0)),
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
                        Node::if_then_else(
                            is_digit(Expr::var("scan_byte")),
                            vec![Node::assign(
                                "tok_len",
                                Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                            )],
                            vec![Node::assign("number_done", Expr::u32(1))],
                        ),
                    ],
                )],
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
                Expr::eq(t, Expr::u32(0)),
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
                                    "vyre-libs::parsing::c_lexer_regular",
                                    "vyre-libs::parsing::c_lexer_regular::classify_at_pos",
                                    classify_at_pos,
                                ));
                                body
                            },
                        )],
                    ),
                    Node::store(out_counts, Expr::u32(0), Expr::var("tok_idx")),
                ],
            )];
            vec![wrap_anonymous(
                "vyre-libs::parsing::c_lexer_regular",
                entry_body,
            )]
        },
    )
    .with_entry_op_id("vyre-libs::parsing::c_lexer_regular")
    .with_non_composable_with_self(true)
}
