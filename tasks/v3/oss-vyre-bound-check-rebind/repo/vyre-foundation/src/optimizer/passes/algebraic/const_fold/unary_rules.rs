// Algebraic simplifications for unary operators.
//
// Each function in this module encodes an algebraic identity that eliminates
// redundant GPU instructions. Contributors add new rules here without touching
// the main const_fold dispatch.

use crate::ir::Expr;

/// Algebraic simplifications of unary operators that don't require
/// the operand to be a literal  -  these are always valid rewrites.
#[expect(
    clippy::too_many_lines,
    clippy::match_same_arms,
    reason = "unary constant-fold table keeps exact mathematical identities auditable"
)]
pub(super) fn simplify_unop(op: &crate::ir::UnOp, operand: &Expr) -> Option<Expr> {
    use crate::ir::{BinOp, UnOp};
    match (op, operand) {
        // ─── Involutions (applying twice = identity) ──────────
        // Not(Not(x)) → x  (boolean involution)
        (
            UnOp::LogicalNot,
            Expr::UnOp {
                op: UnOp::LogicalNot,
                operand: inner,
            },
        ) => Some(inner.as_ref().clone()),
        // Neg(Neg(x)) → x  (arithmetic involution)
        (
            UnOp::Negate,
            Expr::UnOp {
                op: UnOp::Negate,
                operand: inner,
            },
        ) => Some(inner.as_ref().clone()),
        // BitNot(BitNot(x)) → x  (bitwise involution)
        // RevBits(RevBits(x)) → x  (bit-reversal involution)
        (
            UnOp::ReverseBits,
            Expr::UnOp {
                op: UnOp::ReverseBits,
                operand: inner,
            },
        ) => Some(inner.as_ref().clone()),
        (
            UnOp::BitNot,
            Expr::UnOp {
                op: UnOp::BitNot,
                operand: inner,
            },
        ) => Some(inner.as_ref().clone()),

        // ─── Negation fusion ─────────────────────────────────
        // Neg(Sub(a, b)) → Sub(b, a)  (flipped subtraction)
        (
            UnOp::Negate,
            Expr::BinOp {
                op: BinOp::Sub,
                left,
                right,
            },
        ) => Some(Expr::sub(right.as_ref().clone(), left.as_ref().clone())),

        // ─── Abs identities ──────────────────────────────────
        // Abs(Neg(x)) → Abs(x)  (|−x| = |x|)
        (
            UnOp::Abs,
            Expr::UnOp {
                op: UnOp::Negate,
                operand: inner,
            },
        ) => Some(Expr::UnOp {
            op: UnOp::Abs,
            operand: inner.clone(),
        }),
        // Abs(Abs(x)) → Abs(x)  (idempotent)
        (UnOp::Abs, Expr::UnOp { op: UnOp::Abs, .. }) => Some(operand.clone()),

        // ─── Idempotent float operations ─────────────────────
        // Applying these twice is the same as once  -  each
        // elimination removes a GPU transcendental instruction.
        (
            UnOp::Floor,
            Expr::UnOp {
                op: UnOp::Floor, ..
            },
        ) => Some(operand.clone()),
        (UnOp::Ceil, Expr::UnOp { op: UnOp::Ceil, .. }) => Some(operand.clone()),
        (
            UnOp::Round,
            Expr::UnOp {
                op: UnOp::Round, ..
            },
        ) => Some(operand.clone()),
        (
            UnOp::Trunc,
            Expr::UnOp {
                op: UnOp::Trunc, ..
            },
        ) => Some(operand.clone()),
        (UnOp::Sign, Expr::UnOp { op: UnOp::Sign, .. }) => Some(operand.clone()),

        // ─── Floor/Ceil/Trunc subsumption ────────────────────
        // Floor(Trunc(x)) → Trunc(x)  (trunc already removes fractional part)
        (
            UnOp::Floor,
            Expr::UnOp {
                op: UnOp::Trunc, ..
            },
        ) => Some(operand.clone()),
        // Ceil(Trunc(x)) → Trunc(x)
        (
            UnOp::Ceil,
            Expr::UnOp {
                op: UnOp::Trunc, ..
            },
        ) => Some(operand.clone()),
        // Round(Trunc(x)) → Trunc(x)
        (
            UnOp::Round,
            Expr::UnOp {
                op: UnOp::Trunc, ..
            },
        ) => Some(operand.clone()),

        // ─── Sqrt/InverseSqrt cancellation ───────────────────
        // InverseSqrt(InverseSqrt(x)) is not identity, but
        // Sqrt(Sqrt(x)) is x^(1/4)  -  no simplification.
        // However: InverseSqrt of a literal 1.0 → 1.0
        (UnOp::InverseSqrt, Expr::LitF32(v)) if lit_f32_eq(*v, 1.0) => Some(Expr::f32(1.0)),
        (UnOp::Reciprocal, Expr::LitF32(v)) if lit_f32_eq(*v, 1.0) => Some(Expr::f32(1.0)),
        (UnOp::Sqrt, Expr::LitF32(v)) if lit_f32_eq(*v, 1.0) => Some(Expr::f32(1.0)),
        (UnOp::Sqrt, Expr::LitF32(v)) if lit_f32_eq(*v, 0.0) => Some(Expr::f32(0.0)),

        // ─── Trig constants ──────────────────────────────────
        (UnOp::Sin, Expr::LitF32(v)) if lit_f32_eq(*v, 0.0) => Some(Expr::f32(0.0)),
        (UnOp::Cos, Expr::LitF32(v)) if lit_f32_eq(*v, 0.0) => Some(Expr::f32(1.0)),
        (UnOp::Tan, Expr::LitF32(v)) if lit_f32_eq(*v, 0.0) => Some(Expr::f32(0.0)),
        (UnOp::Exp, Expr::LitF32(v)) if lit_f32_eq(*v, 0.0) => Some(Expr::f32(1.0)),
        (UnOp::Exp2, Expr::LitF32(v)) if lit_f32_eq(*v, 0.0) => Some(Expr::f32(1.0)),
        (UnOp::Log, Expr::LitF32(v)) if lit_f32_eq(*v, 1.0) => Some(Expr::f32(0.0)),
        (UnOp::Log2, Expr::LitF32(v)) if lit_f32_eq(*v, 1.0) => Some(Expr::f32(0.0)),
        // Inverse / hyperbolic trig at exact-result arguments. PI and
        // PI/2 cases are skipped because the IR has no canonical PI
        // literal; the caller would need to write the constant out
        // explicitly and the next const-fold pass can pick it up.
        (UnOp::Asin, Expr::LitF32(v)) if lit_f32_eq(*v, 0.0) => Some(Expr::f32(0.0)),
        (UnOp::Acos, Expr::LitF32(v)) if lit_f32_eq(*v, 1.0) => Some(Expr::f32(0.0)),
        (UnOp::Atan, Expr::LitF32(v)) if lit_f32_eq(*v, 0.0) => Some(Expr::f32(0.0)),
        (UnOp::Tanh, Expr::LitF32(v)) if lit_f32_eq(*v, 0.0) => Some(Expr::f32(0.0)),
        (UnOp::Sinh, Expr::LitF32(v)) if lit_f32_eq(*v, 0.0) => Some(Expr::f32(0.0)),
        (UnOp::Cosh, Expr::LitF32(v)) if lit_f32_eq(*v, 0.0) => Some(Expr::f32(1.0)),

        // ─── Popcount/Clz/Ctz of zero literal ────────────────
        (UnOp::Popcount, Expr::LitU32(0)) => Some(Expr::u32(0)),
        (UnOp::Clz, Expr::LitU32(0)) => Some(Expr::u32(32)),
        (UnOp::Ctz, Expr::LitU32(0)) => Some(Expr::u32(32)),
        (UnOp::ReverseBits, Expr::LitU32(0)) => Some(Expr::u32(0)),

        // Constant folding for bit-counting unary ops eliminates the
        // runtime intrinsic call when the operand is compile-time constant.
        (UnOp::Popcount, Expr::LitU32(value)) => Some(Expr::u32(value.count_ones())),
        (UnOp::Clz, Expr::LitU32(value)) => Some(Expr::u32(value.leading_zeros())),
        (UnOp::Ctz, Expr::LitU32(value)) => Some(Expr::u32(value.trailing_zeros())),
        (UnOp::ReverseBits, Expr::LitU32(value)) => Some(Expr::u32(value.reverse_bits())),

        // BitNot of a literal is just the bitwise complement.
        (UnOp::BitNot, Expr::LitU32(value)) => Some(Expr::u32(!value)),

        // Negate of a signed literal is the wrapping-negation of the
        // value; matches CPU + GPU two's-complement semantics.
        (UnOp::Negate, Expr::LitI32(value)) => Some(Expr::i32(value.wrapping_neg())),

        // u32 absolute value is identity (no sign bit). i32 abs uses
        // wrapping semantics so abs(i32::MIN) stays defined behavior
        // (returns i32::MIN per the existing wrapping convention).
        (UnOp::Abs, Expr::LitU32(value)) => Some(Expr::u32(*value)),
        (UnOp::Abs, Expr::LitI32(value)) => Some(Expr::i32(value.wrapping_abs())),

        _ => None,
    }
}

#[inline]
fn lit_f32_eq(value: f32, expected: f32) -> bool {
    value.to_bits() == expected.to_bits()
}
