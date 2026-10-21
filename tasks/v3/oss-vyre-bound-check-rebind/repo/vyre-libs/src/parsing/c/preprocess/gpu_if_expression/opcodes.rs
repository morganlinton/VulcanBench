// Operator opcodes. Higher = higher precedence; `precedence_of` is
// the source of truth and the codes are arbitrary as long as the
// precedence helper agrees.
pub(super) const OP_LPAREN: u32 = 1; // sentinel; never popped except by `)`.
pub(super) const OP_TERNARY_Q: u32 = 2; // sentinel for the `?` of `?:`.
pub(super) const OP_LOR: u32 = 3;
pub(super) const OP_LAND: u32 = 4;
pub(super) const OP_BOR: u32 = 5;
pub(super) const OP_BXOR: u32 = 6;
pub(super) const OP_BAND: u32 = 7;
pub(super) const OP_EQ: u32 = 8;
pub(super) const OP_NE: u32 = 9;
pub(super) const OP_LT: u32 = 10;
pub(super) const OP_LE: u32 = 11;
pub(super) const OP_GT: u32 = 12;
pub(super) const OP_GE: u32 = 13;
pub(super) const OP_SHL: u32 = 14;
pub(super) const OP_SHR: u32 = 15;
pub(super) const OP_ADD: u32 = 16;
pub(super) const OP_SUB: u32 = 17;
pub(super) const OP_MUL: u32 = 18;
pub(super) const OP_DIV: u32 = 19;
pub(super) const OP_MOD: u32 = 20;
// Unary operators. Higher precedence than any binary so they always
// apply before binary work. Encoded as opcodes >= 100 so the apply
// helper can distinguish unary (pop 1) from binary (pop 2).
pub(super) const OP_UN_NOT: u32 = 101;
pub(super) const OP_UN_BNOT: u32 = 102;
pub(super) const OP_UN_NEG: u32 = 103;
pub(super) const OP_UN_PLUS: u32 = 104;
pub(super) const INVALID_EXPR_VALUE: u32 =
    crate::parsing::c::preprocess::gpu_if_expression_abi::INVALID_EXPR_VALUE;

use super::*;

pub(super) fn precedence_of(op: Expr) -> Expr {
    // Match each opcode to its precedence. LPAREN/TERNARY_Q have
    // precedence 0 so binary operators never pop them. Unary ops
    // are 14 so they apply before any binary work but still get
    // popped by `)`/EOF drain.
    Expr::select(
        Expr::ge(op.clone(), Expr::u32(100)),
        Expr::u32(14),
        Expr::select(
            Expr::eq(op.clone(), Expr::u32(OP_MUL)),
            Expr::u32(13),
            Expr::select(
                Expr::eq(op.clone(), Expr::u32(OP_DIV)),
                Expr::u32(13),
                Expr::select(
                    Expr::eq(op.clone(), Expr::u32(OP_MOD)),
                    Expr::u32(13),
                    Expr::select(
                        Expr::eq(op.clone(), Expr::u32(OP_ADD)),
                        Expr::u32(12),
                        Expr::select(
                            Expr::eq(op.clone(), Expr::u32(OP_SUB)),
                            Expr::u32(12),
                            Expr::select(
                                Expr::eq(op.clone(), Expr::u32(OP_SHL)),
                                Expr::u32(11),
                                Expr::select(
                                    Expr::eq(op.clone(), Expr::u32(OP_SHR)),
                                    Expr::u32(11),
                                    Expr::select(
                                        Expr::eq(op.clone(), Expr::u32(OP_LT)),
                                        Expr::u32(10),
                                        Expr::select(
                                            Expr::eq(op.clone(), Expr::u32(OP_LE)),
                                            Expr::u32(10),
                                            Expr::select(
                                                Expr::eq(op.clone(), Expr::u32(OP_GT)),
                                                Expr::u32(10),
                                                Expr::select(
                                                    Expr::eq(op.clone(), Expr::u32(OP_GE)),
                                                    Expr::u32(10),
                                                    Expr::select(
                                                        Expr::eq(op.clone(), Expr::u32(OP_EQ)),
                                                        Expr::u32(9),
                                                        Expr::select(
                                                            Expr::eq(op.clone(), Expr::u32(OP_NE)),
                                                            Expr::u32(9),
                                                            Expr::select(
                                                                Expr::eq(
                                                                    op.clone(),
                                                                    Expr::u32(OP_BAND),
                                                                ),
                                                                Expr::u32(8),
                                                                Expr::select(
                                                                    Expr::eq(
                                                                        op.clone(),
                                                                        Expr::u32(OP_BXOR),
                                                                    ),
                                                                    Expr::u32(7),
                                                                    Expr::select(
                                                                        Expr::eq(
                                                                            op.clone(),
                                                                            Expr::u32(OP_BOR),
                                                                        ),
                                                                        Expr::u32(6),
                                                                        Expr::select(
                                                                            Expr::eq(
                                                                                op.clone(),
                                                                                Expr::u32(OP_LAND),
                                                                            ),
                                                                            Expr::u32(5),
                                                                            Expr::select(
                                                                                Expr::eq(
                                                                                    op,
                                                                                    Expr::u32(
                                                                                        OP_LOR,
                                                                                    ),
                                                                                ),
                                                                                Expr::u32(4),
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
    )
}
