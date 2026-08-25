// Algebraic identity simplifications for binary operators.
//
// Each rule encodes an algebraic identity that eliminates redundant GPU
// instructions. Rules fire when one operand is a literal identity/annihilator
// element, or when both operands are syntactically identical.
//
// Contributors: add new rules to the appropriate section below. Each section
// is delimited by a Unicode box-drawing comment header.

use crate::ir::Expr;
use crate::optimizer::algebraic_rules::{
    binop_identity_replacement, IdentityReplacement, ScalarLiteral,
};

/// Check if an expression is known to produce a float type.
/// Used by FMA synthesis to avoid integer→float promotion.
pub(crate) fn is_float_expr(expr: &Expr) -> bool {
    match expr {
        Expr::LitF32(_) | Expr::Fma { .. } => true,
        // BinOp with float evidence produces float.
        Expr::BinOp { left, right, .. } => is_float_expr(left) || is_float_expr(right),
        // UnOp preserves type
        Expr::UnOp { operand, .. } => is_float_expr(operand),
        // Cast to float
        Expr::Cast { target, .. } => {
            matches!(target, crate::ir::DataType::F32 | crate::ir::DataType::F64)
        }
        // Without a type system we cannot prove float-ness of
        // variables/loads. Be conservative: do not synthesize FMA
        // unless an operand provides real float evidence.
        _ => false,
    }
}

pub(super) fn mul_operands(expr: &Expr) -> Option<(&Expr, &Expr)> {
    match expr {
        Expr::BinOp {
            op: crate::ir::BinOp::Mul,
            left,
            right,
        } => Some((left, right)),
        _ => None,
    }
}

/// True iff `expr` is an integer literal (u32 or i32). Used by the
/// distributive expansion rule (ROADMAP A33) to gate the rewrite so
/// it only fires when one of the new sub-multiplications will fold
/// in the next const-fold pass.
fn lit_int(expr: &Expr) -> Option<()> {
    match expr {
        Expr::LitU32(_) | Expr::LitI32(_) => Some(()),
        _ => None,
    }
}

/// True for expressions that are pure and side-effect-free.
/// Used to guard reflexive equality folding so we never elide a Load
/// whose ordering matters.
pub(super) fn is_simple_pure(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::LitU32(_)
            | Expr::LitI32(_)
            | Expr::LitF32(_)
            | Expr::LitBool(_)
            | Expr::Var(_)
            | Expr::InvocationId { .. }
            | Expr::WorkgroupId { .. }
            | Expr::LocalId { .. }
            | Expr::SubgroupLocalId
            | Expr::SubgroupSize
    )
}

/// Algebraic identity simplifications for binary operators.
/// These rewrites are always valid and don't require literal operands  -
/// they fire when one operand is a literal identity/annihilator element.
///
/// Critical for GPU kernels where loop unrolling and index arithmetic
/// generate patterns like `base + 0`, `stride * 1`, or `mask & 0`.
#[expect(
    clippy::too_many_lines,
    clippy::match_same_arms,
    reason = "binary identity table intentionally groups algebraic laws by operator family"
)]
pub(super) fn simplify_binop(op: crate::ir::BinOp, left: &Expr, right: &Expr) -> Option<Expr> {
    use crate::ir::BinOp;

    // Helper: is this a u32/i32/f32 zero?
    let is_zero = |e: &Expr| match e {
        Expr::LitU32(0) | Expr::LitI32(0) => true,
        Expr::LitF32(v) => lit_f32_eq(*v, 0.0),
        _ => false,
    };
    // Helper: is this a u32/i32/f32 one?
    let is_one = |e: &Expr| match e {
        Expr::LitU32(1) | Expr::LitI32(1) => true,
        Expr::LitF32(v) => lit_f32_eq(*v, 1.0),
        _ => false,
    };

    if let Some(replacement) = binop_identity_replacement(
        op,
        left == right && is_simple_pure(left),
        expr_scalar_literal(left),
        expr_scalar_literal(right),
    ) {
        return Some(match replacement {
            IdentityReplacement::Left => left.clone(),
            IdentityReplacement::Right => right.clone(),
        });
    }

    match op {
        // ─── FMA synthesis ───────────────────────────────────
        // (a * b) + c  →  Fma(a, b, c)
        // c + (a * b)  →  Fma(a, b, c)
        // (a * b) - c  →  Fma(a, b, -c)
        // c - (a * b)  →  Fma(-a, b, c)
        // Maps to a single GPU FMA instruction: 1 cycle vs 2 for mul+add.
        BinOp::Add => {
            if let Some((a, b)) = mul_operands(left) {
                if is_float_expr(right) {
                    return Some(Expr::fma(a.clone(), b.clone(), right.clone()));
                }
            }
            if let Some((a, b)) = mul_operands(right) {
                if is_float_expr(left) {
                    return Some(Expr::fma(a.clone(), b.clone(), left.clone()));
                }
            }
            // x + 0 → x,  0 + x → x
            if is_zero(right) {
                return Some(left.clone());
            }
            if is_zero(left) {
                return Some(right.clone());
            }

            // ─── Algebraic Reassociation ─────────────────────────
            // (a + K1) + K2 → a + (K1 + K2)
            if let Expr::BinOp {
                op: BinOp::Add,
                left: inner_a,
                right: inner_k1,
            } = left
            {
                match (inner_k1.as_ref(), right) {
                    (Expr::LitU32(k1), Expr::LitU32(k2)) => {
                        if let Some(sum) = k1.checked_add(*k2) {
                            return Some(Expr::add(inner_a.as_ref().clone(), Expr::u32(sum)));
                        }
                    }
                    (Expr::LitI32(k1), Expr::LitI32(k2)) => {
                        if let Some(sum) = k1.checked_add(*k2) {
                            return Some(Expr::add(inner_a.as_ref().clone(), Expr::i32(sum)));
                        }
                    }
                    _ => {}
                }
            }
            // (K1 + a) + K2 → a + (K1 + K2)
            if let Expr::BinOp {
                op: BinOp::Add,
                left: inner_k1,
                right: inner_a,
            } = left
            {
                match (inner_k1.as_ref(), right) {
                    (Expr::LitU32(k1), Expr::LitU32(k2)) => {
                        if let Some(sum) = k1.checked_add(*k2) {
                            return Some(Expr::add(inner_a.as_ref().clone(), Expr::u32(sum)));
                        }
                    }
                    (Expr::LitI32(k1), Expr::LitI32(k2)) => {
                        if let Some(sum) = k1.checked_add(*k2) {
                            return Some(Expr::add(inner_a.as_ref().clone(), Expr::i32(sum)));
                        }
                    }
                    _ => {}
                }
            }

            // ─── Distributive Law ──────────────────────────────────────────
            // (x * K1) + (x * K2) → x * (K1 + K2)
            if let (
                Expr::BinOp {
                    op: BinOp::Mul,
                    left: l_inner_a,
                    right: l_inner_k,
                },
                Expr::BinOp {
                    op: BinOp::Mul,
                    left: r_inner_a,
                    right: r_inner_k,
                },
            ) = (left, right)
            {
                if l_inner_a == r_inner_a {
                    match (l_inner_k.as_ref(), r_inner_k.as_ref()) {
                        (Expr::LitU32(k1), Expr::LitU32(k2)) => {
                            if let Some(sum) = k1.checked_add(*k2) {
                                return Some(Expr::mul(l_inner_a.as_ref().clone(), Expr::u32(sum)));
                            }
                        }
                        (Expr::LitI32(k1), Expr::LitI32(k2)) => {
                            if let Some(sum) = k1.checked_add(*k2) {
                                return Some(Expr::mul(l_inner_a.as_ref().clone(), Expr::i32(sum)));
                            }
                        }
                        _ => {}
                    }
                }
            }

            None
        }

        BinOp::Sub => {
            if left == right {
                return Some(Expr::u32(0));
            }
            if let Some((a, b)) = mul_operands(left) {
                if is_float_expr(right) {
                    return Some(Expr::fma(a.clone(), b.clone(), Expr::negate(right.clone())));
                }
            }
            if let Some((a, b)) = mul_operands(right) {
                if is_float_expr(left) {
                    return Some(Expr::fma(Expr::negate(a.clone()), b.clone(), left.clone()));
                }
            }
            if is_zero(right) {
                return Some(left.clone());
            }

            // ─── Distributive Law ──────────────────────────────────────────
            // (x * K1) - (x * K2) → x * (K1 - K2)
            if let (
                Expr::BinOp {
                    op: BinOp::Mul,
                    left: l_inner_a,
                    right: l_inner_k,
                },
                Expr::BinOp {
                    op: BinOp::Mul,
                    left: r_inner_a,
                    right: r_inner_k,
                },
            ) = (left, right)
            {
                if l_inner_a == r_inner_a {
                    match (l_inner_k.as_ref(), r_inner_k.as_ref()) {
                        (Expr::LitU32(k1), Expr::LitU32(k2)) => {
                            if let Some(diff) = k1.checked_sub(*k2) {
                                return Some(Expr::mul(
                                    l_inner_a.as_ref().clone(),
                                    Expr::u32(diff),
                                ));
                            }
                        }
                        (Expr::LitI32(k1), Expr::LitI32(k2)) => {
                            if let Some(diff) = k1.checked_sub(*k2) {
                                return Some(Expr::mul(
                                    l_inner_a.as_ref().clone(),
                                    Expr::i32(diff),
                                ));
                            }
                        }
                        _ => {}
                    }
                }
            }

            None
        }

        BinOp::Mul => {
            // ─── Algebraic Reassociation ─────────────────────────
            // (a * K1) * K2 → a * (K1 * K2)
            if let Expr::BinOp {
                op: BinOp::Mul,
                left: inner_a,
                right: inner_k1,
            } = left
            {
                match (inner_k1.as_ref(), right) {
                    (Expr::LitU32(k1), Expr::LitU32(k2)) => {
                        if let Some(prod) = k1.checked_mul(*k2) {
                            return Some(Expr::mul(inner_a.as_ref().clone(), Expr::u32(prod)));
                        }
                    }
                    (Expr::LitI32(k1), Expr::LitI32(k2)) => {
                        if let Some(prod) = k1.checked_mul(*k2) {
                            return Some(Expr::mul(inner_a.as_ref().clone(), Expr::i32(prod)));
                        }
                    }
                    _ => {}
                }
            }
            // (K1 * a) * K2 → a * (K1 * K2)
            if let Expr::BinOp {
                op: BinOp::Mul,
                left: inner_k1,
                right: inner_a,
            } = left
            {
                match (inner_k1.as_ref(), right) {
                    (Expr::LitU32(k1), Expr::LitU32(k2)) => {
                        if let Some(prod) = k1.checked_mul(*k2) {
                            return Some(Expr::mul(inner_a.as_ref().clone(), Expr::u32(prod)));
                        }
                    }
                    (Expr::LitI32(k1), Expr::LitI32(k2)) => {
                        if let Some(prod) = k1.checked_mul(*k2) {
                            return Some(Expr::mul(inner_a.as_ref().clone(), Expr::i32(prod)));
                        }
                    }
                    _ => {}
                }
            }

            // ─── Distributive expansion (ROADMAP A33) ────────────
            // Mul(c, Add(a, b)) → Add(Mul(c, a), Mul(c, b)) when `c` is
            // a literal and at least one of `a`/`b` is also a literal.
            // The "at least one literal sibling" guard guarantees at
            // least one of the new Muls folds in the next pass, so the
            // rewrite is monotone-down on instruction count for the
            // post-fold IR. Wrapping integer arithmetic preserves
            // equality (a + b)·c == a·c + b·c under u32/i32 overflow,
            // so the rewrite is `Exact` for integer types only;
            // floating point is rejected because the rounding of
            // separate multiplications differs from one fused
            // multiply.
            if let (
                Some(()),
                Expr::BinOp {
                    op: BinOp::Add,
                    left: a,
                    right: b,
                },
            ) = (lit_int(left), right)
            {
                if lit_int(a).is_some() || lit_int(b).is_some() {
                    return Some(Expr::add(
                        Expr::mul(left.clone(), a.as_ref().clone()),
                        Expr::mul(left.clone(), b.as_ref().clone()),
                    ));
                }
            }
            if let (
                Expr::BinOp {
                    op: BinOp::Add,
                    left: a,
                    right: b,
                },
                Some(()),
            ) = (left, lit_int(right))
            {
                if lit_int(a).is_some() || lit_int(b).is_some() {
                    return Some(Expr::add(
                        Expr::mul(a.as_ref().clone(), right.clone()),
                        Expr::mul(b.as_ref().clone(), right.clone()),
                    ));
                }
            }

            // ─── Sign-preserving distributive expansion for subtraction (ROADMAP A33) ───
            if let (
                Some(()),
                Expr::BinOp {
                    op: BinOp::Sub,
                    left: a,
                    right: b,
                },
            ) = (lit_int(left), right)
            {
                if lit_int(a).is_some() || lit_int(b).is_some() {
                    return Some(Expr::BinOp {
                        op: BinOp::Sub,
                        left: Box::new(Expr::mul(left.clone(), a.as_ref().clone())),
                        right: Box::new(Expr::mul(left.clone(), b.as_ref().clone())),
                    });
                }
            }
            if let (
                Expr::BinOp {
                    op: BinOp::Sub,
                    left: a,
                    right: b,
                },
                Some(()),
            ) = (left, lit_int(right))
            {
                if lit_int(a).is_some() || lit_int(b).is_some() {
                    return Some(Expr::BinOp {
                        op: BinOp::Sub,
                        left: Box::new(Expr::mul(a.as_ref().clone(), right.clone())),
                        right: Box::new(Expr::mul(b.as_ref().clone(), right.clone())),
                    });
                }
            }

            // x * 1 → x,  1 * x → x
            if is_one(right) {
                return Some(left.clone());
            }
            if is_one(left) {
                return Some(right.clone());
            }

            // x * (-1) → Negate(x),  (-1) * x → Negate(x)
            // GPU neg is 1 cycle; imul is 4-8 cycles.
            if matches!(right, Expr::LitI32(-1)) {
                return Some(Expr::negate(left.clone()));
            }
            if matches!(left, Expr::LitI32(-1)) {
                return Some(Expr::negate(right.clone()));
            }
            if matches!(right, Expr::LitF32(v) if lit_f32_eq(*v, -1.0)) {
                return Some(Expr::negate(left.clone()));
            }
            if matches!(left, Expr::LitF32(v) if lit_f32_eq(*v, -1.0)) {
                return Some(Expr::negate(right.clone()));
            }

            // x * 0 → 0,  0 * x → 0  (for integer types only)
            if matches!(right, Expr::LitU32(0) | Expr::LitI32(0)) {
                return Some(right.clone());
            }
            if matches!(left, Expr::LitU32(0) | Expr::LitI32(0)) {
                return Some(left.clone());
            }

            None
        }

        // x / 1 → x
        BinOp::Div if is_one(right) => Some(left.clone()),
        // Literal i32 / -1 is folded by the shared literal evaluator when
        // defined. Non-literal i32 can be MIN at runtime, where target-text division
        // overflows, so do not rewrite it to negate.
        BinOp::Div if matches!(right, Expr::LitI32(-1)) => None,

        // ─── Reciprocal sqrt fusion ──────────────────────────
        // 1.0 / sqrt(x) → rsqrt(x)
        // Maps to a single GPU SFU instruction (4 cycles) instead of
        // sqrt + fdiv (8+32 = 40 cycles). Critical for normalization
        // kernels (LayerNorm, RMSNorm).
        BinOp::Div if matches!(left, Expr::LitF32(v) if lit_f32_eq(*v, 1.0)) => {
            if let Expr::UnOp {
                op: crate::ir::UnOp::Sqrt,
                operand,
            } = right
            {
                return Some(Expr::UnOp {
                    op: crate::ir::UnOp::InverseSqrt,
                    operand: operand.clone(),
                });
            }
            None
        }

        // ─── Trig / identity peepholes ────────────────────────
        BinOp::Div => {
            // sin(x) / cos(x) → tan(x)  (saves 1 transcendental)
            if let (
                Expr::UnOp {
                    op: crate::ir::UnOp::Sin,
                    operand: sin_arg,
                },
                Expr::UnOp {
                    op: crate::ir::UnOp::Cos,
                    operand: cos_arg,
                },
            ) = (left, right)
            {
                if sin_arg == cos_arg {
                    return Some(Expr::UnOp {
                        op: crate::ir::UnOp::Tan,
                        operand: sin_arg.clone(),
                    });
                }
            }
            // x / x → 1  (integer only; float needs NaN/zero guard)
            if left == right && !is_float_expr(left) {
                return Some(Expr::u32(1));
            }
            None
        }

        // x & 0 → 0 (integer bitwise annihilator)
        BinOp::BitAnd if matches!(right, Expr::LitU32(0) | Expr::LitI32(0)) => Some(right.clone()),
        BinOp::BitAnd if matches!(left, Expr::LitU32(0) | Expr::LitI32(0)) => Some(left.clone()),

        // x | 0 → x,  0 | x → x (integer bitwise identity)
        BinOp::BitOr if matches!(right, Expr::LitU32(0) | Expr::LitI32(0)) => Some(left.clone()),
        BinOp::BitOr if matches!(left, Expr::LitU32(0) | Expr::LitI32(0)) => Some(right.clone()),

        // x ^ 0 → x,  0 ^ x → x (xor identity)
        BinOp::BitXor if matches!(right, Expr::LitU32(0) | Expr::LitI32(0)) => Some(left.clone()),
        BinOp::BitXor if matches!(left, Expr::LitU32(0) | Expr::LitI32(0)) => Some(right.clone()),

        // ── Shift fusion + shift-by-zero elimination ────────────
        // (x << a) << b → x << (a + b) when a,b are literal.
        // x << 0 → x,  x >> 0 → x.
        BinOp::Shl | BinOp::Shr => {
            if matches!(right, Expr::LitU32(0)) {
                return Some(left.clone());
            }
            if let Expr::BinOp {
                op: inner_op,
                left: x,
                right: inner_shift,
            } = left
            {
                if *inner_op == op {
                    if let (Expr::LitU32(a), Expr::LitU32(b)) = (inner_shift.as_ref(), right) {
                        let fused = a.saturating_add(*b).min(31);
                        return Some(Expr::BinOp {
                            op,
                            left: x.clone(),
                            right: Box::new(Expr::u32(fused)),
                        });
                    }
                }
            }
            None
        }

        // ─── Self-operand identities ─────────────────────────
        // These fire on loop-unrolling artifacts and duplicated
        // index expressions that produce syntactically identical
        // operands.  Each rule removes an entire instruction from
        // the GPU shader.

        // x ^ x → 0  (xor self-inverse)
        BinOp::BitXor if left == right => Some(Expr::u32(0)),
        // x & x → x  (bitwise idempotent)
        BinOp::BitAnd if left == right => Some(left.clone()),
        // x | x → x  (bitwise idempotent)
        BinOp::BitOr if left == right => Some(left.clone()),

        // ─── Comparison self-identities ──────────────────────
        // x == x → true,  x != x → false   -  only when `x` is purely
        // value-deterministic. Two `Load` reads of the same buffer can
        // observe distinct values under relaxed memory ordering, so
        // folding `Eq(Load, Load) → true` would be unsound. The
        // `is_simple_pure` guard keeps us inside the safe envelope
        // (Var/Lit/builtin) where repeated evaluation is observably
        // free.
        BinOp::Eq if left == right && is_simple_pure(left) => Some(Expr::bool(true)),
        BinOp::Ne if left == right && is_simple_pure(left) => Some(Expr::bool(false)),
        // x < x → false,  x > x → false   -  same purity caveat applies.
        BinOp::Lt if left == right && is_simple_pure(left) => Some(Expr::bool(false)),
        BinOp::Gt if left == right && is_simple_pure(left) => Some(Expr::bool(false)),
        // x <= x → true,  x >= x → true
        BinOp::Le if left == right && is_simple_pure(left) => Some(Expr::bool(true)),
        BinOp::Ge if left == right && is_simple_pure(left) => Some(Expr::bool(true)),

        // ─── Modulo identities ───────────────────────────────
        // x % 1 → 0  (any integer mod 1 is zero)
        BinOp::Mod if is_one(right) => Some(Expr::u32(0)),
        // x % x → 0  (self-modulo is zero for non-zero x)
        BinOp::Mod if left == right => Some(Expr::u32(0)),

        // ─── Min/Max absorption ──────────────────────────────
        // min(x, x) → x,  max(x, x) → x
        BinOp::Min if left == right => Some(left.clone()),
        BinOp::Max if left == right => Some(left.clone()),

        // ─── ROADMAP A35: range-based fold identities ────────
        // For unsigned u32, every value lies in [0, u32::MAX], so:
        //   min(x, u32::MAX) → x  /  min(u32::MAX, x) → x
        //   max(x, 0)        → x  /  max(0, x) → x
        //   min(x, 0)        → 0  /  min(0, x) → 0
        //   max(x, u32::MAX) → u32::MAX
        // These need only the static type bounds, not full range
        // analysis; full range-fact-driven folds will land beside the
        // downstream range pass (A16 / A35 stronger variant) when that
        // substrate is wired into the optimizer.
        BinOp::Min if matches!(right, Expr::LitU32(u32::MAX)) => Some(left.clone()),
        BinOp::Min if matches!(left, Expr::LitU32(u32::MAX)) => Some(right.clone()),
        BinOp::Max if matches!(right, Expr::LitU32(0)) => Some(left.clone()),
        BinOp::Max if matches!(left, Expr::LitU32(0)) => Some(right.clone()),
        BinOp::Min if matches!(right, Expr::LitU32(0)) => Some(Expr::u32(0)),
        BinOp::Min if matches!(left, Expr::LitU32(0)) => Some(Expr::u32(0)),
        BinOp::Max if matches!(right, Expr::LitU32(u32::MAX)) => Some(Expr::u32(u32::MAX)),
        BinOp::Max if matches!(left, Expr::LitU32(u32::MAX)) => Some(Expr::u32(u32::MAX)),

        // Comparison with extremes: x < 0 → false, 0 <= x → true,
        // x <= u32::MAX → true, u32::MAX < x → false. These come up
        // after const-fold normalises a runtime check that always
        // succeeds (e.g. `if (lane >= 0)`  -  true for u32 by type).
        BinOp::Lt if matches!(right, Expr::LitU32(0)) => Some(Expr::bool(false)),
        BinOp::Ge if matches!(right, Expr::LitU32(0)) => Some(Expr::bool(true)),
        BinOp::Le if matches!(right, Expr::LitU32(u32::MAX)) => Some(Expr::bool(true)),
        BinOp::Gt if matches!(right, Expr::LitU32(u32::MAX)) => Some(Expr::bool(false)),
        // Symmetric forms (literal on the left):
        BinOp::Gt if matches!(left, Expr::LitU32(0)) => Some(Expr::bool(false)),
        BinOp::Le if matches!(left, Expr::LitU32(0)) => Some(Expr::bool(true)),
        BinOp::Ge if matches!(left, Expr::LitU32(u32::MAX)) => Some(Expr::bool(true)),
        BinOp::Lt if matches!(left, Expr::LitU32(u32::MAX)) => Some(Expr::bool(false)),

        // u32 modulo by `u32::MAX + 1` (which is just `0` literal in
        // u32) is undefined; mod by 1 already handled above; mod by
        // 2^32 is unrepresentable as a literal  -  no extra rule needed.

        // ─── Wrapping arithmetic identities ──────────────────
        // wrapping_add(x, 0) → x,  wrapping_sub(x, 0) → x
        BinOp::WrappingAdd if is_zero(right) => Some(left.clone()),
        BinOp::WrappingAdd if is_zero(left) => Some(right.clone()),
        BinOp::WrappingSub if is_zero(right) => Some(left.clone()),
        // wrapping_sub(x, x) → 0
        BinOp::WrappingSub if left == right => Some(Expr::u32(0)),

        // ─── Saturating arithmetic identities ────────────────
        BinOp::SaturatingAdd if is_zero(right) => Some(left.clone()),
        BinOp::SaturatingAdd if is_zero(left) => Some(right.clone()),
        BinOp::SaturatingSub if is_zero(right) => Some(left.clone()),
        BinOp::SaturatingSub if left == right => Some(Expr::u32(0)),
        BinOp::SaturatingMul if is_one(right) => Some(left.clone()),
        BinOp::SaturatingMul if is_one(left) => Some(right.clone()),
        BinOp::SaturatingMul if is_zero(right) => Some(Expr::u32(0)),
        BinOp::SaturatingMul if is_zero(left) => Some(Expr::u32(0)),

        // ─── Logical identities ──────────────────────────────
        // true && x → x,  x && true → x
        BinOp::And if matches!(left, Expr::LitBool(true)) => Some(right.clone()),
        BinOp::And if matches!(right, Expr::LitBool(true)) => Some(left.clone()),
        // false && x → false,  x && false → false
        BinOp::And if matches!(left, Expr::LitBool(false)) => Some(Expr::bool(false)),
        BinOp::And if matches!(right, Expr::LitBool(false)) => Some(Expr::bool(false)),
        // true || x → true,  x || true → true
        BinOp::Or if matches!(left, Expr::LitBool(true)) => Some(Expr::bool(true)),
        BinOp::Or if matches!(right, Expr::LitBool(true)) => Some(Expr::bool(true)),
        // false || x → x,  x || false → x
        BinOp::Or if matches!(left, Expr::LitBool(false)) => Some(right.clone()),
        BinOp::Or if matches!(right, Expr::LitBool(false)) => Some(left.clone()),
        // x && x → x,  x || x → x  (idempotent)
        BinOp::And if left == right => Some(left.clone()),
        BinOp::Or if left == right => Some(left.clone()),

        // ─── ROADMAP A25: chained-predicate boolean simplification ──
        // x && !x → false  (contradiction)
        BinOp::And if matches!(right, Expr::UnOp { op: crate::ir::UnOp::LogicalNot, operand } if operand.as_ref() == left) => {
            Some(Expr::bool(false))
        }
        BinOp::And if matches!(left, Expr::UnOp { op: crate::ir::UnOp::LogicalNot, operand } if operand.as_ref() == right) => {
            Some(Expr::bool(false))
        }
        // x || !x → true  (tautology)
        BinOp::Or if matches!(right, Expr::UnOp { op: crate::ir::UnOp::LogicalNot, operand } if operand.as_ref() == left) => {
            Some(Expr::bool(true))
        }
        BinOp::Or if matches!(left, Expr::UnOp { op: crate::ir::UnOp::LogicalNot, operand } if operand.as_ref() == right) => {
            Some(Expr::bool(true))
        }
        // Absorption: x && (x || y) → x ; (x || y) && x → x
        BinOp::And if matches!(right, Expr::BinOp { op: BinOp::Or, left: l, right: r } if l.as_ref() == left || r.as_ref() == left) => {
            Some(left.clone())
        }
        BinOp::And if matches!(left, Expr::BinOp { op: BinOp::Or, left: l, right: r } if l.as_ref() == right || r.as_ref() == right) => {
            Some(right.clone())
        }
        // Absorption: x || (x && y) → x ; (x && y) || x → x
        BinOp::Or if matches!(right, Expr::BinOp { op: BinOp::And, left: l, right: r } if l.as_ref() == left || r.as_ref() == left) => {
            Some(left.clone())
        }
        BinOp::Or if matches!(left, Expr::BinOp { op: BinOp::And, left: l, right: r } if l.as_ref() == right || r.as_ref() == right) => {
            Some(right.clone())
        }
        // (Reflexive Eq/Ne handled in the comparison-self-identities
        // section above  -  guarded by `is_simple_pure` for soundness
        // under relaxed memory ordering.)

        // ─── BitAnd with all-ones mask → identity ────────────
        BinOp::BitAnd if matches!(right, Expr::LitU32(u32::MAX)) => Some(left.clone()),
        BinOp::BitAnd if matches!(left, Expr::LitU32(u32::MAX)) => Some(right.clone()),
        // BitOr with all-ones → all-ones
        BinOp::BitOr if matches!(right, Expr::LitU32(u32::MAX)) => Some(Expr::u32(u32::MAX)),
        BinOp::BitOr if matches!(left, Expr::LitU32(u32::MAX)) => Some(Expr::u32(u32::MAX)),

        // ─── BitXor with all-ones → BitNot ───────────────────
        // x ^ 0xFFFFFFFF → ~x  (common mask pattern)
        BinOp::BitXor if matches!(right, Expr::LitU32(u32::MAX)) => Some(Expr::UnOp {
            op: crate::ir::UnOp::BitNot,
            operand: Box::new(left.clone()),
        }),
        BinOp::BitXor if matches!(left, Expr::LitU32(u32::MAX)) => Some(Expr::UnOp {
            op: crate::ir::UnOp::BitNot,
            operand: Box::new(right.clone()),
        }),

        _ => None,
    }
}

fn expr_scalar_literal(expr: &Expr) -> Option<ScalarLiteral> {
    match expr {
        Expr::LitU32(value) => Some(ScalarLiteral::U32(*value)),
        Expr::LitI32(value) => Some(ScalarLiteral::I32(*value)),
        Expr::LitF32(value) => Some(ScalarLiteral::F32(*value)),
        Expr::LitBool(value) => Some(ScalarLiteral::Bool(*value)),
        _ => None,
    }
}

// ─── ROADMAP A35: stronger range fold ─────────────────────────────
// Mod(x, N) where x.max < N -> x
// We use a tiny single-block lookbehind to find if `x` was defined as `LitU32(c)` where `c < N`.
pub(super) fn fold_mod_lookbehind(
    nodes: &[crate::ir::Node],
    changed: &mut bool,
) -> Vec<crate::ir::Node> {
    use crate::ir::{Expr, Node};
    use rustc_hash::FxHashMap;
    use std::sync::Arc;

    let mut out = Vec::with_capacity(nodes.len());
    let mut local_lits: FxHashMap<crate::ir::Ident, u32> = FxHashMap::default();

    for node in nodes {
        match node {
            Node::Let { name, value } => {
                let new_value = rewrite_expr_for_mod(value, &local_lits, changed);
                if let Expr::LitU32(c) = &new_value {
                    local_lits.insert(name.clone(), *c);
                } else if let Expr::LitI32(c) = &new_value {
                    if *c >= 0 {
                        if let Ok(value) = u32::try_from(*c) {
                            local_lits.insert(name.clone(), value);
                        }
                    }
                }
                out.push(Node::Let {
                    name: name.clone(),
                    value: new_value,
                });
            }
            Node::Assign { name, value } => {
                local_lits.remove(name);
                out.push(Node::Assign {
                    name: name.clone(),
                    value: rewrite_expr_for_mod(value, &local_lits, changed),
                });
            }
            Node::Store {
                buffer,
                index,
                value,
            } => {
                out.push(Node::Store {
                    buffer: buffer.clone(),
                    index: rewrite_expr_for_mod(index, &local_lits, changed),
                    value: rewrite_expr_for_mod(value, &local_lits, changed),
                });
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                out.push(Node::If {
                    cond: rewrite_expr_for_mod(cond, &local_lits, changed),
                    then: fold_mod_lookbehind(then, changed),
                    otherwise: fold_mod_lookbehind(otherwise, changed),
                });
            }
            Node::Loop {
                var,
                from,
                to,
                body,
            } => {
                out.push(Node::Loop {
                    var: var.clone(),
                    from: rewrite_expr_for_mod(from, &local_lits, changed),
                    to: rewrite_expr_for_mod(to, &local_lits, changed),
                    body: fold_mod_lookbehind(body, changed),
                });
            }
            Node::Block(body) => {
                out.push(Node::Block(fold_mod_lookbehind(body, changed)));
            }
            Node::Region {
                generator,
                source_region,
                body,
            } => {
                out.push(Node::Region {
                    generator: generator.clone(),
                    source_region: source_region.clone(),
                    body: Arc::new(fold_mod_lookbehind(body, changed)),
                });
            }
            Node::Trap { address, tag } => {
                out.push(Node::Trap {
                    address: Box::new(rewrite_expr_for_mod(address, &local_lits, changed)),
                    tag: tag.clone(),
                });
            }
            _ => out.push(node.clone()),
        }
    }
    out
}

fn rewrite_expr_for_mod(
    expr: &crate::ir::Expr,
    local_lits: &rustc_hash::FxHashMap<crate::ir::Ident, u32>,
    changed: &mut bool,
) -> crate::ir::Expr {
    use crate::ir::{BinOp, Expr};
    let mut transformer = |e: &Expr| -> Option<Expr> {
        if let Expr::BinOp {
            op: BinOp::Mod,
            left,
            right,
        } = e
        {
            if let (Expr::Var(x), Expr::LitU32(n)) = (left.as_ref(), right.as_ref()) {
                if let Some(&c) = local_lits.get(x) {
                    if c < *n {
                        *changed = true;
                        return Some(Expr::Var(x.clone()));
                    }
                }
            }
        }
        None
    };
    let cow = crate::optimizer::rewrite::rewrite_expr(expr, &mut transformer);
    cow.into_owned()
}

#[inline]
fn lit_f32_eq(value: f32, expected: f32) -> bool {
    value.to_bits() == expected.to_bits()
}
