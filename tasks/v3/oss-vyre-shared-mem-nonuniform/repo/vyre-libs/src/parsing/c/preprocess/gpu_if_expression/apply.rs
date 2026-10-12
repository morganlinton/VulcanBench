use super::*;

pub(super) fn apply_top_op() -> Vec<Node> {
    let mut nodes = Vec::new();
    nodes.push(Node::let_bind("apply_op", Expr::u32(0)));
    nodes.extend(pop_stack("op_stack", "osp", "apply_op"));
    // Always pop one value (the unary operand or the binary RHS).
    nodes.push(Node::let_bind("apply_rhs", Expr::u32(0)));
    nodes.extend(pop_stack("val_stack", "vsp", "apply_rhs"));
    // Pop a second value only for binary opcodes. Unary opcodes
    // are >= 100; for those `apply_lhs` stays 0 and is unused
    // because the unary result is computed solely from
    // `apply_rhs` (the operand) below.
    nodes.push(Node::let_bind("apply_lhs", Expr::u32(0)));
    nodes.push(Node::if_then(
        Expr::lt(Expr::var("apply_op"), Expr::u32(100)),
        pop_stack("val_stack", "vsp", "apply_lhs"),
    ));
    // Unary computation overrides apply_result for unary ops.
    nodes.push(Node::let_bind(
        "unary_result",
        Expr::select(
            Expr::eq(Expr::var("apply_op"), Expr::u32(OP_UN_NOT)),
            Expr::select(
                Expr::eq(Expr::var("apply_rhs"), Expr::u32(0)),
                Expr::u32(1),
                Expr::u32(0),
            ),
            Expr::select(
                Expr::eq(Expr::var("apply_op"), Expr::u32(OP_UN_BNOT)),
                Expr::sub(Expr::u32(0xFFFF_FFFF), Expr::var("apply_rhs")),
                Expr::select(
                    Expr::eq(Expr::var("apply_op"), Expr::u32(OP_UN_NEG)),
                    Expr::sub(Expr::u32(0), Expr::var("apply_rhs")),
                    Expr::var("apply_rhs"),
                ),
            ),
        ),
    ));
    // Compute result based on apply_op. Division/modulo use guarded IR
    // arithmetic so kernels never issue undefined divide instructions.
    nodes.push(Node::if_then(
        Expr::and(
            Expr::or(
                Expr::eq(Expr::var("apply_op"), Expr::u32(OP_DIV)),
                Expr::eq(Expr::var("apply_op"), Expr::u32(OP_MOD)),
            ),
            Expr::eq(Expr::var("apply_rhs"), Expr::u32(0)),
        ),
        vec![Node::assign("expr_invalid", Expr::u32(1))],
    ));
    nodes.push(Node::let_bind(
            "apply_result",
            Expr::select(
                Expr::eq(Expr::var("apply_op"), Expr::u32(OP_ADD)),
                Expr::add(Expr::var("apply_lhs"), Expr::var("apply_rhs")),
                Expr::select(
                    Expr::eq(Expr::var("apply_op"), Expr::u32(OP_SUB)),
                    Expr::sub(Expr::var("apply_lhs"), Expr::var("apply_rhs")),
                    Expr::select(
                        Expr::eq(Expr::var("apply_op"), Expr::u32(OP_MUL)),
                        Expr::mul(Expr::var("apply_lhs"), Expr::var("apply_rhs")),
                        Expr::select(
                            Expr::and(
                                Expr::eq(Expr::var("apply_op"), Expr::u32(OP_DIV)),
                                Expr::ne(Expr::var("apply_rhs"), Expr::u32(0)),
                            ),
                            Expr::div(Expr::var("apply_lhs"), Expr::var("apply_rhs")),
                            Expr::select(
                                Expr::and(
                                    Expr::eq(Expr::var("apply_op"), Expr::u32(OP_MOD)),
                                    Expr::ne(Expr::var("apply_rhs"), Expr::u32(0)),
                                ),
                                Expr::rem(Expr::var("apply_lhs"), Expr::var("apply_rhs")),
                                Expr::select(
                                    Expr::eq(Expr::var("apply_op"), Expr::u32(OP_BAND)),
                                    Expr::bitand(Expr::var("apply_lhs"), Expr::var("apply_rhs")),
                                    Expr::select(
                                        Expr::eq(Expr::var("apply_op"), Expr::u32(OP_BOR)),
                                        Expr::bitor(Expr::var("apply_lhs"), Expr::var("apply_rhs")),
                                        Expr::select(
                                            Expr::eq(Expr::var("apply_op"), Expr::u32(OP_BXOR)),
                                            Expr::bitxor(
                                                Expr::var("apply_lhs"),
                                                Expr::var("apply_rhs"),
                                            ),
                                            Expr::select(
                                                Expr::eq(Expr::var("apply_op"), Expr::u32(OP_LAND)),
                                                Expr::select(
                                                    Expr::and(
                                                        Expr::ne(
                                                            Expr::var("apply_lhs"),
                                                            Expr::u32(0),
                                                        ),
                                                        Expr::ne(
                                                            Expr::var("apply_rhs"),
                                                            Expr::u32(0),
                                                        ),
                                                    ),
                                                    Expr::u32(1),
                                                    Expr::u32(0),
                                                ),
                                                Expr::select(
                                                    Expr::eq(Expr::var("apply_op"), Expr::u32(OP_LOR)),
                                                    Expr::select(
                                                        Expr::or(
                                                            Expr::ne(
                                                                Expr::var("apply_lhs"),
                                                                Expr::u32(0),
                                                            ),
                                                            Expr::ne(
                                                                Expr::var("apply_rhs"),
                                                                Expr::u32(0),
                                                            ),
                                                        ),
                                                        Expr::u32(1),
                                                        Expr::u32(0),
                                                    ),
                                                    Expr::select(
                                                        Expr::eq(
                                                            Expr::var("apply_op"),
                                                            Expr::u32(OP_EQ),
                                                        ),
                                                        Expr::select(
                                                            Expr::eq(
                                                                Expr::var("apply_lhs"),
                                                                Expr::var("apply_rhs"),
                                                            ),
                                                            Expr::u32(1),
                                                            Expr::u32(0),
                                                        ),
                                                        Expr::select(
                                                            Expr::eq(
                                                                Expr::var("apply_op"),
                                                                Expr::u32(OP_NE),
                                                            ),
                                                            Expr::select(
                                                                Expr::ne(
                                                                    Expr::var("apply_lhs"),
                                                                    Expr::var("apply_rhs"),
                                                                ),
                                                                Expr::u32(1),
                                                                Expr::u32(0),
                                                            ),
                                                            Expr::select(
                                                                Expr::eq(
                                                                    Expr::var("apply_op"),
                                                                    Expr::u32(OP_LT),
                                                                ),
                                                                Expr::select(
                                                                    Expr::lt(
                                                                        Expr::var("apply_lhs"),
                                                                        Expr::var("apply_rhs"),
                                                                    ),
                                                                    Expr::u32(1),
                                                                    Expr::u32(0),
                                                                ),
                                                                Expr::select(
                                                                    Expr::eq(
                                                                        Expr::var("apply_op"),
                                                                        Expr::u32(OP_LE),
                                                                    ),
                                                                    Expr::select(
                                                                        Expr::le(
                                                                            Expr::var("apply_lhs"),
                                                                            Expr::var("apply_rhs"),
                                                                        ),
                                                                        Expr::u32(1),
                                                                        Expr::u32(0),
                                                                    ),
                                                                    Expr::select(
                                                                        Expr::eq(
                                                                            Expr::var("apply_op"),
                                                                            Expr::u32(OP_GT),
                                                                        ),
                                                                        Expr::select(
                                                                            Expr::gt(
                                                                                Expr::var("apply_lhs"),
                                                                                Expr::var("apply_rhs"),
                                                                            ),
                                                                            Expr::u32(1),
                                                                            Expr::u32(0),
                                                                        ),
                                                                        Expr::select(
                                                                            Expr::eq(
                                                                                Expr::var("apply_op"),
                                                                                Expr::u32(OP_GE),
                                                                            ),
                                                                            Expr::select(
                                                                                Expr::ge(
                                                                                    Expr::var("apply_lhs"),
                                                                                    Expr::var("apply_rhs"),
                                                                                ),
                                                                                Expr::u32(1),
                                                                                Expr::u32(0),
                                                                            ),
                                                                            Expr::select(
                                                                                Expr::eq(
                                                                                    Expr::var(
                                                                                        "apply_op",
                                                                                    ),
                                                                                    Expr::u32(OP_SHL),
                                                                                ),
                                                                                Expr::shl(
                                                                                    Expr::var(
                                                                                        "apply_lhs",
                                                                                    ),
                                                                                    Expr::bitand(
                                                                                        Expr::var(
                                                                                            "apply_rhs",
                                                                                        ),
                                                                                        Expr::u32(31),
                                                                                    ),
                                                                                ),
                                                                                Expr::select(
                                                                                    Expr::eq(
                                                                                        Expr::var(
                                                                                            "apply_op",
                                                                                        ),
                                                                                        Expr::u32(OP_SHR),
                                                                                    ),
                                                                                    Expr::shr(
                                                                                        Expr::var(
                                                                                            "apply_lhs",
                                                                                        ),
                                                                                        Expr::bitand(
                                                                                            Expr::var(
                                                                                                "apply_rhs",
                                                                                            ),
                                                                                            Expr::u32(
                                                                                                31,
                                                                                            ),
                                                                                        ),
                                                                                    ),
                                                                                    Expr::u32(0),
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
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ));
    // Final result: unary opcodes use the unary path; binary use
    // the apply_result cascade above.
    nodes.push(Node::let_bind(
        "final_result",
        Expr::select(
            Expr::ge(Expr::var("apply_op"), Expr::u32(100)),
            Expr::var("unary_result"),
            Expr::var("apply_result"),
        ),
    ));
    nodes.extend(push_stack("val_stack", "vsp", Expr::var("final_result")));
    nodes
}
