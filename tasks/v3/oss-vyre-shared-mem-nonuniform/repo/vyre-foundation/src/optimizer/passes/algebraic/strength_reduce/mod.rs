use crate::ir::{BinOp, Expr, Program, UnOp};
use crate::optimizer::rewrite::rewrite_program;
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};

mod arithmetic;

use arithmetic::{
    granlund_montgomery_div, horner_polynomial_int, power_of_two_shift, reciprocal_constant_fold,
    shift_add_decompose, synthesize_fma_add, synthesize_fma_sub,
};

/// Replace multiplication by powers of two with shifts.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "strength_reduce",
    requires = ["const_fold"],
    invalidates = ["const_fold", "reaching_def_propagate", "value_numbering"],
    phase = "scalar_algebra",
    boundary_class = "abi_preserving",
    cost_model_family = "scalar"
)]
pub struct StrengthReduce;

impl StrengthReduce {
    /// O(1) gate: strength reduction only fires on expressions, which only
    /// live inside Let / Assign / Store / If-cond / Loop bound / Trap nodes.
    /// A program that is just Return / Barrier / `IndirectDispatch` / `AsyncWait`
    /// has no expression tree to rewrite. The bitset mask covers every
    /// expression-bearing node kind so SKIP fires on truly expression-free
    /// programs without false negatives.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if !program
            .stats()
            .has_any_node_kind(crate::ir::stats::NODE_KIND_EXPRESSION_BEARING_MASK)
        {
            return PassAnalysis::SKIP;
        }
        PassAnalysis::RUN
    }

    /// Rewrite multiply-by-power-of-two expressions into left shifts.
    ///
    /// AUDIT_2026-04-24 F-SR-01 (closed): `rewrite_program` already
    /// preserves `non_composable_with_self` via `with_rewritten_entry`
    /// (see builder.rs line ~134). No explicit call needed here.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let (program, changed) = rewrite_program(program, reduce_expr);
        PassResult { program, changed }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "strength-reduction table keeps algebraic rewrite precedence auditable"
)]
fn reduce_expr(expr: &Expr) -> Option<Expr> {
    if let Some(reduced) = horner_polynomial_int(expr) {
        return Some(reduced);
    }

    let Expr::BinOp { op, left, right } = expr else {
        return None;
    };
    match op {
        // Integer Mul-by-2^k → Shl by k.
        BinOp::Mul => {
            if matches!(right.as_ref(), Expr::LitU32(0)) {
                return Some(Expr::u32(0));
            }
            if matches!(left.as_ref(), Expr::LitU32(0)) {
                return Some(Expr::u32(0));
            }
            if matches!(right.as_ref(), Expr::LitU32(1)) {
                return Some(left.as_ref().clone());
            }
            if matches!(left.as_ref(), Expr::LitU32(1)) {
                return Some(right.as_ref().clone());
            }
            if let Some(shift) = power_of_two_shift(right) {
                return Some(Expr::shl(left.as_ref().clone(), Expr::u32(shift)));
            }
            if let Some(shift) = power_of_two_shift(left) {
                return Some(Expr::shl(right.as_ref().clone(), Expr::u32(shift)));
            }
            // ── Shift-add decomposition for non-power-of-two constants ──
            // GPU imul is 4-8 cycles; shift+add/sub is 2 cycles.
            // x * C  →  (x << hi) ± (x << lo)  when C = 2^hi ± 2^lo.
            // This fires for the most common index/stride multipliers
            // found in real GPU kernels (3, 5, 6, 7, 9, 10, 12, 15).
            if let Some(decomposed) = shift_add_decompose(left.as_ref(), right.as_ref()) {
                return Some(decomposed);
            }
            if let Some(decomposed) = shift_add_decompose(right.as_ref(), left.as_ref()) {
                return Some(decomposed);
            }
            // Signed multiply by a negative constant: x * (-C) → Negate(x * C).
            // The positive product is strength-reduced to shifts on the next
            // fixpoint iteration. Two's-complement i32 only; -1 is owned by
            // const-fold and i32::MIN cannot be negated without overflow.
            if let Expr::LitI32(c) = right.as_ref() {
                if *c < -1 && *c != i32::MIN {
                    return Some(Expr::negate(Expr::mul(
                        left.as_ref().clone(),
                        Expr::i32(-*c),
                    )));
                }
            }
            if let Expr::LitI32(c) = left.as_ref() {
                if *c < -1 && *c != i32::MIN {
                    return Some(Expr::negate(Expr::mul(
                        right.as_ref().clone(),
                        Expr::i32(-*c),
                    )));
                }
            }
            // Float: x * 2.0 → x + x (saves a mul, uses cheaper add).
            if matches!(right.as_ref(), Expr::LitF32(v) if lit_f32_eq(*v, 2.0)) {
                return Some(Expr::add(left.as_ref().clone(), left.as_ref().clone()));
            }
            if matches!(left.as_ref(), Expr::LitF32(v) if lit_f32_eq(*v, 2.0)) {
                return Some(Expr::add(right.as_ref().clone(), right.as_ref().clone()));
            }
            // Float: x * 1.0 → x (multiplicative identity).
            if matches!(right.as_ref(), Expr::LitF32(v) if lit_f32_eq(*v, 1.0)) {
                return Some(left.as_ref().clone());
            }
            if matches!(left.as_ref(), Expr::LitF32(v) if lit_f32_eq(*v, 1.0)) {
                return Some(right.as_ref().clone());
            }
            None
        }
        // Unsigned Div-by-2^k → Shr by k. Only fires when rhs is a
        // LitU32 power of two  -  LitI32 paths avoid signed semantics
        // mismatch (negative dividend + rounding direction).
        BinOp::Div => {
            // ROADMAP G2: 1.0 / constant → compile-time reciprocal literal.
            if let Some(folded) = reciprocal_constant_fold(left.as_ref(), right.as_ref()) {
                return Some(folded);
            }
            // ROADMAP G2: 1.0 / x → Reciprocal(x). Keeping reciprocal as
            // a first-class IR op lets strict backends emit precise rcp and
            // ULP-budgeted backends emit approximate rcp without re-discovering
            // the expression shape in every driver.
            if matches!(left.as_ref(), Expr::LitF32(v) if lit_f32_eq(*v, 1.0))
                && !matches!(right.as_ref(), Expr::LitF32(_))
            {
                return Some(Expr::reciprocal(right.as_ref().clone()));
            }
            match right.as_ref() {
                Expr::LitU32(1) => Some(left.as_ref().clone()),
                Expr::LitU32(value) if value.is_power_of_two() => Some(Expr::shr(
                    left.as_ref().clone(),
                    Expr::u32(value.trailing_zeros()),
                )),
                // Granlund-Montgomery: any non-zero, non-power-of-two u32
                // constant → mulhi(n, magic) >> shift.
                // Saves 40-90 GPU cycles per division.
                Expr::LitU32(d) if *d > 1 && !d.is_power_of_two() => {
                    granlund_montgomery_div(left.as_ref(), *d)
                }
                // Float: x / 2.0 → x * 0.5 (mul is cheaper than div).
                Expr::LitF32(v) if lit_f32_eq(*v, 2.0) => {
                    Some(Expr::mul(left.as_ref().clone(), Expr::f32(0.5)))
                }
                // Float: x / 1.0 → x (identity).
                Expr::LitF32(v) if lit_f32_eq(*v, 1.0) => Some(left.as_ref().clone()),
                // Float: x / C → x * (1/C) for any non-zero finite constant.
                // GPU fdiv is 4-8× slower than fmul; on training workloads
                // with per-element normalization (LayerNorm, RMSNorm) this
                // turns a ~32-cycle instruction into a ~4-cycle one.
                Expr::LitF32(v) if v.is_finite() && f32_nonzero(*v) => {
                    Some(Expr::mul(left.as_ref().clone(), Expr::f32(1.0 / v)))
                }
                _ => None,
            }
        }
        // Unsigned Mod-by-constant.
        BinOp::Mod => {
            let Expr::LitU32(value) = right.as_ref() else {
                return None;
            };
            // x % 1 → 0 for every unsigned x.
            if *value == 1 {
                return Some(Expr::u32(0));
            }
            // x % 2^k → x & (2^k - 1).
            if value.is_power_of_two() {
                return Some(Expr::bitand(left.as_ref().clone(), Expr::u32(value - 1)));
            }
            // x % d (non-power-of-two constant) → x - (x / d) * d, reusing the
            // Granlund-Montgomery exact u32 division. The (x / d) * d multiply
            // strength-reduces to shift/add on the next fixpoint pass, so a
            // ~40-90 cycle integer umod becomes mulhi + shift + mul + sub.
            // d == 0 falls through here (granlund_montgomery_div guards d <= 1),
            // leaving modulo-by-zero intact for the backend to trap.
            granlund_montgomery_div(left.as_ref(), *value).map(|quotient| {
                Expr::sub(
                    left.as_ref().clone(),
                    Expr::mul(quotient, Expr::u32(*value)),
                )
            })
        }
        // Float: x + 0.0 → x (additive identity).
        BinOp::Add => {
            if let Some(fma) = synthesize_fma_add(left, right) {
                return Some(fma);
            }
            if matches!(right.as_ref(), Expr::LitF32(v) if *v == 0.0) {
                return Some(left.as_ref().clone());
            }
            if matches!(left.as_ref(), Expr::LitF32(v) if *v == 0.0) {
                return Some(right.as_ref().clone());
            }
            // Integer: x + 0 → x.
            if matches!(right.as_ref(), Expr::LitU32(0)) {
                return Some(left.as_ref().clone());
            }
            if matches!(left.as_ref(), Expr::LitU32(0)) {
                return Some(right.as_ref().clone());
            }
            // ── Negation fusion ──────────────────────────────────
            // x + (-y) → x - y  (eliminates 1 negate instruction)
            if let Expr::UnOp {
                op: UnOp::Negate,
                operand: y,
            } = right.as_ref()
            {
                return Some(Expr::sub(left.as_ref().clone(), y.as_ref().clone()));
            }
            // (-x) + y → y - x
            if let Expr::UnOp {
                op: UnOp::Negate,
                operand: x,
            } = left.as_ref()
            {
                return Some(Expr::sub(right.as_ref().clone(), x.as_ref().clone()));
            }
            None
        }
        // Float: x - 0.0 → x (subtractive identity).
        BinOp::Sub => {
            if let Some(fma) = synthesize_fma_sub(left, right) {
                return Some(fma);
            }
            if matches!(right.as_ref(), Expr::LitF32(v) if *v == 0.0) {
                return Some(left.as_ref().clone());
            }
            if matches!(right.as_ref(), Expr::LitU32(0)) {
                return Some(left.as_ref().clone());
            }
            // x - (-y) → x + y  (eliminates 1 negate instruction)
            if let Expr::UnOp {
                op: UnOp::Negate,
                operand: y,
            } = right.as_ref()
            {
                return Some(Expr::add(left.as_ref().clone(), y.as_ref().clone()));
            }
            None
        }
        // ── Shift fusion + shift-by-zero elimination ────────────
        // (x << a) << b → x << (a + b) when a,b are literal.
        // x << 0 → x,  x >> 0 → x.
        BinOp::Shl | BinOp::Shr => {
            // Zero shifted by any amount is still zero.
            if matches!(left.as_ref(), Expr::LitU32(0) | Expr::LitI32(0)) {
                return Some(left.as_ref().clone());
            }
            // Shift by zero → identity.
            if matches!(right.as_ref(), Expr::LitU32(0)) {
                return Some(left.as_ref().clone());
            }
            // Chained shift fusion: (x <<|>> a) <<|>> b → x <<|>> (a+b)
            // Only fuse when both shifts are the same direction.
            if let Expr::BinOp {
                op: inner_op,
                left: x,
                right: inner_shift,
            } = left.as_ref()
            {
                if inner_op == op {
                    if let (Expr::LitU32(a), Expr::LitU32(b)) =
                        (inner_shift.as_ref(), right.as_ref())
                    {
                        let total = a.saturating_add(*b);
                        // A u32 shift by >= 32 bits produces 0 for every
                        // non-zero value of x; clamping to 31 would produce
                        // x << 31 instead of 0 — a miscompile.
                        if total > 31 {
                            return Some(Expr::u32(0));
                        }
                        return Some(Expr::BinOp {
                            op: *op,
                            left: x.clone(),
                            right: Box::new(Expr::u32(total)),
                        });
                    }
                }
            }
            None
        }

        // ── BitAnd mask fusion ──────────────────────────────────
        // (x >> k) & mask  where mask = (1 << n) - 1
        // → extract n bits starting at position k.
        // This is a recognition pass; the combined operation is
        // already optimal but canonicalizing it aids CSE.

        // ── BitAnd complement annihilator ───────────────────────
        // x & ~x → 0,  ~x & x → 0   (complementary mask cancellation)
        BinOp::BitAnd => {
            if let Expr::UnOp {
                op: UnOp::BitNot,
                operand: inner,
            } = right.as_ref()
            {
                if inner.as_ref() == left.as_ref() {
                    return Some(Expr::u32(0));
                }
            }
            if let Expr::UnOp {
                op: UnOp::BitNot,
                operand: inner,
            } = left.as_ref()
            {
                if inner.as_ref() == right.as_ref() {
                    return Some(Expr::u32(0));
                }
            }
            None
        }

        // ── BitOr complement → all-ones ─────────────────────────
        // x | ~x → 0xFFFFFFFF,  ~x | x → 0xFFFFFFFF
        BinOp::BitOr | BinOp::BitXor => {
            if let Expr::UnOp {
                op: UnOp::BitNot,
                operand: inner,
            } = right.as_ref()
            {
                if inner.as_ref() == left.as_ref() {
                    return Some(Expr::u32(u32::MAX));
                }
            }
            if let Expr::UnOp {
                op: UnOp::BitNot,
                operand: inner,
            } = left.as_ref()
            {
                if inner.as_ref() == right.as_ref() {
                    return Some(Expr::u32(u32::MAX));
                }
            }
            None
        }

        // ── Rotate-by-zero → identity ───────────────────────────
        BinOp::RotateLeft | BinOp::RotateRight if matches!(right.as_ref(), Expr::LitU32(0)) => {
            Some(left.as_ref().clone())
        }
        // Rotate-by-32 (full width) → identity for u32
        BinOp::RotateLeft | BinOp::RotateRight if matches!(right.as_ref(), Expr::LitU32(32)) => {
            Some(left.as_ref().clone())
        }

        // ── AbsDiff self-identity ───────────────────────────────
        // absdiff(x, x) → 0
        BinOp::AbsDiff if left.as_ref() == right.as_ref() => Some(Expr::u32(0)),

        // ── Min/Max with literal extremes ───────────────────────
        // min(x, 0) → 0  for unsigned (u32 cannot be negative)
        BinOp::Min if matches!(right.as_ref(), Expr::LitU32(0)) => Some(Expr::u32(0)),
        BinOp::Min if matches!(left.as_ref(), Expr::LitU32(0)) => Some(Expr::u32(0)),
        // max(x, 0) → x  for unsigned
        BinOp::Max if matches!(right.as_ref(), Expr::LitU32(0)) => Some(left.as_ref().clone()),
        BinOp::Max if matches!(left.as_ref(), Expr::LitU32(0)) => Some(right.as_ref().clone()),
        // min(x, MAX) → x,  max(x, MAX) → MAX
        BinOp::Min if matches!(right.as_ref(), Expr::LitU32(u32::MAX)) => {
            Some(left.as_ref().clone())
        }
        BinOp::Min if matches!(left.as_ref(), Expr::LitU32(u32::MAX)) => {
            Some(right.as_ref().clone())
        }
        BinOp::Max if matches!(right.as_ref(), Expr::LitU32(u32::MAX)) => Some(Expr::u32(u32::MAX)),
        BinOp::Max if matches!(left.as_ref(), Expr::LitU32(u32::MAX)) => Some(Expr::u32(u32::MAX)),

        // ── Comparison strength reduction ───────────────────────
        // x < 0 → false  for unsigned (u32 can never be negative)
        BinOp::Lt if matches!(right.as_ref(), Expr::LitU32(0)) => Some(Expr::bool(false)),
        // x >= 0 → true  for unsigned
        BinOp::Ge if matches!(right.as_ref(), Expr::LitU32(0)) => Some(Expr::bool(true)),
        // 0 > x → false  for unsigned
        BinOp::Gt if matches!(left.as_ref(), Expr::LitU32(0)) => {
            // 0 > x is false for all u32 x
            Some(Expr::bool(false))
        }
        // 0 <= x → true  for unsigned
        BinOp::Le if matches!(left.as_ref(), Expr::LitU32(0)) => Some(Expr::bool(true)),

        _ => None,
    }
}

#[inline]
fn lit_f32_eq(value: f32, expected: f32) -> bool {
    value.to_bits() == expected.to_bits()
}

#[inline]
fn f32_nonzero(value: f32) -> bool {
    value.to_bits() & 0x7FFF_FFFF != 0
}

#[cfg(test)]
mod tests;
