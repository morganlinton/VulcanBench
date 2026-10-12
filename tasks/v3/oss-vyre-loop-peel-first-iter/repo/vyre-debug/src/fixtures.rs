use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

pub fn loop_carry_smoke() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("haystack", 0, BufferAccess::ReadOnly, DataType::U32).with_count(8),
            BufferDecl::storage("out_count", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("cursor", Expr::u32(0)),
                Node::let_bind("tok_idx", Expr::u32(0)),
                Node::loop_for(
                    "iter",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![Node::if_then(
                        Expr::lt(Expr::var("cursor"), Expr::u32(8)),
                        vec![
                            Node::let_bind("byte", Expr::load("haystack", Expr::var("cursor"))),
                            Node::let_bind("emit", Expr::u32(0)),
                            Node::if_then(
                                Expr::and(
                                    Expr::eq(Expr::var("emit"), Expr::u32(0)),
                                    Expr::ne(Expr::var("byte"), Expr::u32(0)),
                                ),
                                vec![Node::assign("emit", Expr::u32(1))],
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var("emit"), Expr::u32(1)),
                                vec![Node::assign(
                                    "tok_idx",
                                    Expr::add(Expr::var("tok_idx"), Expr::u32(1)),
                                )],
                            ),
                            Node::assign("cursor", Expr::add(Expr::var("cursor"), Expr::u32(1))),
                        ],
                    )],
                ),
                Node::store("out_count", Expr::u32(0), Expr::var("tok_idx")),
            ],
        )],
    )
}
