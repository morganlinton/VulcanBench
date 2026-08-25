use super::{
    program_helpers::source_byte_load, TRANSFORM_BLOCK_COMMENT, TRANSFORM_LINE_COMMENT,
    TRANSFORM_LINE_SPLICE, TRANSFORM_LITERAL_QUOTE,
};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

pub(super) fn transform_candidate_program(n: u32) -> Program {
    let i = Expr::var("i");
    let load_next = |addr: Expr| -> Expr {
        Expr::select(
            Expr::lt(addr.clone(), Expr::load("transform_n_real", Expr::u32(0))),
            source_byte_load("bytes_in", addr),
            Expr::u32(0),
        )
    };
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::load("transform_n_real", Expr::u32(0))),
            vec![
                Node::let_bind("b0", source_byte_load("bytes_in", i.clone())),
                Node::let_bind("b1", load_next(Expr::add(i.clone(), Expr::u32(1)))),
                Node::let_bind(
                    "opens_line_comment",
                    Expr::and(
                        Expr::eq(Expr::var("b0"), Expr::u32(b'/' as u32)),
                        Expr::eq(Expr::var("b1"), Expr::u32(b'/' as u32)),
                    ),
                ),
                Node::let_bind(
                    "opens_block_comment",
                    Expr::and(
                        Expr::eq(Expr::var("b0"), Expr::u32(b'/' as u32)),
                        Expr::eq(Expr::var("b1"), Expr::u32(b'*' as u32)),
                    ),
                ),
                Node::let_bind(
                    "opens_splice",
                    Expr::and(
                        Expr::eq(Expr::var("b0"), Expr::u32(b'\\' as u32)),
                        Expr::or(
                            Expr::eq(Expr::var("b1"), Expr::u32(b'\n' as u32)),
                            Expr::eq(Expr::var("b1"), Expr::u32(b'\r' as u32)),
                        ),
                    ),
                ),
                Node::if_then(
                    Expr::var("opens_line_comment"),
                    vec![Node::let_bind(
                        "line_comment_flag_old",
                        Expr::atomic_or(
                            "transform_flag",
                            Expr::u32(0),
                            Expr::u32(TRANSFORM_LINE_COMMENT),
                        ),
                    )],
                ),
                Node::if_then(
                    Expr::var("opens_block_comment"),
                    vec![Node::let_bind(
                        "block_comment_flag_old",
                        Expr::atomic_or(
                            "transform_flag",
                            Expr::u32(0),
                            Expr::u32(TRANSFORM_BLOCK_COMMENT),
                        ),
                    )],
                ),
                Node::if_then(
                    Expr::var("opens_splice"),
                    vec![Node::let_bind(
                        "line_splice_flag_old",
                        Expr::atomic_or(
                            "transform_flag",
                            Expr::u32(0),
                            Expr::u32(TRANSFORM_LINE_SPLICE),
                        ),
                    )],
                ),
                Node::if_then(
                    Expr::or(
                        Expr::eq(Expr::var("b0"), Expr::u32(b'"' as u32)),
                        Expr::eq(Expr::var("b0"), Expr::u32(b'\'' as u32)),
                    ),
                    vec![Node::let_bind(
                        "literal_quote_flag_old",
                        Expr::atomic_or(
                            "transform_flag",
                            Expr::u32(0),
                            Expr::u32(TRANSFORM_LITERAL_QUOTE),
                        ),
                    )],
                ),
            ],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage("bytes_in", 0, BufferAccess::ReadOnly, DataType::U8).with_count(0),
            BufferDecl::storage("transform_flag", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::storage("transform_n_real", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        body,
    )
    .with_entry_op_id("vyre-libs::parsing::c::preprocess::filter_transform_preflight")
}
