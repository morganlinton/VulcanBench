//! Surface tests for `Expr` convenience builders.
//!
//! Every expression builder must produce the expected IR shape so that
//! frontends and tests build correct programs.

use vyre::ir::{BinOp, Expr};

// ------------------------------------------------------------------
// Arithmetic
// ------------------------------------------------------------------

#[test]
fn add_produces_binop_add() {
    let e = Expr::add(Expr::u32(1), Expr::u32(2));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Add, .. }));
}

#[test]
fn sub_produces_binop_sub() {
    let e = Expr::sub(Expr::u32(5), Expr::u32(3));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Sub, .. }));
}

#[test]
fn mul_produces_binop_mul() {
    let e = Expr::mul(Expr::u32(2), Expr::u32(3));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Mul, .. }));
}

#[test]
fn div_produces_binop_div() {
    let e = Expr::div(Expr::u32(6), Expr::u32(2));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Div, .. }));
}

#[test]
fn rem_produces_binop_mod() {
    let e = Expr::rem(Expr::u32(7), Expr::u32(3));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Mod, .. }));
}

#[test]
fn negate_produces_unop_negate() {
    let e = Expr::negate(Expr::u32(5));
    assert!(matches!(
        e,
        Expr::UnOp {
            op: vyre::ir::UnOp::Negate,
            ..
        }
    ));
}

#[test]
fn saturating_sub_emits_saturating_sub_opcode() {
    // saturating_sub emits the dedicated BinOp::SaturatingSub opcode (wire tag
    // 0x17), not a `left - max(right, 0)` lowering to BinOp::Sub — so the wire
    // fingerprint and the saturating evaluation semantics stay exact.
    let e = Expr::saturating_sub(Expr::u32(5), Expr::u32(10));
    assert!(matches!(e, Expr::BinOp { op: BinOp::SaturatingSub, .. }));
}

#[test]
fn abs_diff_produces_binop_abs_diff() {
    let e = Expr::abs_diff(Expr::u32(3), Expr::u32(7));
    assert!(matches!(
        e,
        Expr::BinOp {
            op: BinOp::AbsDiff,
            ..
        }
    ));
}

// ------------------------------------------------------------------
// Comparison
// ------------------------------------------------------------------

#[test]
fn eq_produces_binop_eq() {
    let e = Expr::eq(Expr::u32(1), Expr::u32(1));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Eq, .. }));
}

#[test]
fn ne_produces_binop_ne() {
    let e = Expr::ne(Expr::u32(1), Expr::u32(2));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Ne, .. }));
}

#[test]
fn lt_produces_binop_lt() {
    let e = Expr::lt(Expr::u32(1), Expr::u32(2));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Lt, .. }));
}

#[test]
fn le_produces_binop_le() {
    let e = Expr::le(Expr::u32(1), Expr::u32(2));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Le, .. }));
}

#[test]
fn gt_produces_binop_gt() {
    let e = Expr::gt(Expr::u32(2), Expr::u32(1));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Gt, .. }));
}

#[test]
fn ge_produces_binop_ge() {
    let e = Expr::ge(Expr::u32(2), Expr::u32(1));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Ge, .. }));
}

// ------------------------------------------------------------------
// Bitwise
// ------------------------------------------------------------------

#[test]
fn bitand_produces_binop_bitand() {
    let e = Expr::bitand(Expr::u32(0xFF), Expr::u32(0x0F));
    assert!(matches!(
        e,
        Expr::BinOp {
            op: BinOp::BitAnd,
            ..
        }
    ));
}

#[test]
fn bitor_produces_binop_bitor() {
    let e = Expr::bitor(Expr::u32(0xF0), Expr::u32(0x0F));
    assert!(matches!(
        e,
        Expr::BinOp {
            op: BinOp::BitOr,
            ..
        }
    ));
}

#[test]
fn bitxor_produces_binop_bitxor() {
    let e = Expr::bitxor(Expr::u32(0xFF), Expr::u32(0x0F));
    assert!(matches!(
        e,
        Expr::BinOp {
            op: BinOp::BitXor,
            ..
        }
    ));
}

#[test]
fn shl_produces_binop_shl() {
    let e = Expr::shl(Expr::u32(1), Expr::u32(4));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Shl, .. }));
}

#[test]
fn shr_produces_binop_shr() {
    let e = Expr::shr(Expr::u32(16), Expr::u32(2));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Shr, .. }));
}

#[test]
fn not_produces_unop_logical_not() {
    let e = Expr::not(Expr::bool(true));
    assert!(matches!(
        e,
        Expr::UnOp {
            op: vyre::ir::UnOp::LogicalNot,
            ..
        }
    ));
}

// ------------------------------------------------------------------
// Logical
// ------------------------------------------------------------------

#[test]
fn and_produces_binop_and() {
    let e = Expr::and(Expr::bool(true), Expr::bool(false));
    assert!(matches!(e, Expr::BinOp { op: BinOp::And, .. }));
}

#[test]
fn or_produces_binop_or() {
    let e = Expr::or(Expr::bool(true), Expr::bool(false));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Or, .. }));
}

// ------------------------------------------------------------------
// Min / Max
// ------------------------------------------------------------------

#[test]
fn min_produces_binop_min() {
    let e = Expr::min(Expr::u32(3), Expr::u32(7));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Min, .. }));
}

#[test]
fn max_produces_binop_max() {
    let e = Expr::max(Expr::u32(3), Expr::u32(7));
    assert!(matches!(e, Expr::BinOp { op: BinOp::Max, .. }));
}

// ------------------------------------------------------------------
// Float helpers
// ------------------------------------------------------------------

#[test]
fn floor_produces_unop_floor() {
    let e = Expr::floor(Expr::f32(1.5));
    assert!(matches!(
        e,
        Expr::UnOp {
            op: vyre::ir::UnOp::Floor,
            ..
        }
    ));
}

#[test]
fn ceil_produces_unop_ceil() {
    let e = Expr::ceil(Expr::f32(1.5));
    assert!(matches!(
        e,
        Expr::UnOp {
            op: vyre::ir::UnOp::Ceil,
            ..
        }
    ));
}

#[test]
fn round_produces_unop_round() {
    let e = Expr::round(Expr::f32(1.5));
    assert!(matches!(
        e,
        Expr::UnOp {
            op: vyre::ir::UnOp::Round,
            ..
        }
    ));
}

#[test]
fn trunc_produces_unop_trunc() {
    let e = Expr::trunc(Expr::f32(1.5));
    assert!(matches!(
        e,
        Expr::UnOp {
            op: vyre::ir::UnOp::Trunc,
            ..
        }
    ));
}

#[test]
fn sqrt_produces_unop_sqrt() {
    let e = Expr::sqrt(Expr::f32(4.0));
    assert!(matches!(
        e,
        Expr::UnOp {
            op: vyre::ir::UnOp::Sqrt,
            ..
        }
    ));
}

#[test]
fn abs_produces_unop_abs() {
    let e = Expr::abs(Expr::f32(-3.0));
    assert!(matches!(
        e,
        Expr::UnOp {
            op: vyre::ir::UnOp::Abs,
            ..
        }
    ));
}

// ------------------------------------------------------------------
// Literals
// ------------------------------------------------------------------

#[test]
fn u32_literal_shape() {
    assert!(matches!(Expr::u32(42), Expr::LitU32(42)));
}

#[test]
fn i32_literal_shape() {
    assert!(matches!(Expr::i32(-7), Expr::LitI32(-7)));
}

#[test]
fn f32_literal_shape() {
    assert!(matches!(Expr::f32(3.14), Expr::LitF32(v) if v == 3.14));
}

#[test]
fn bool_literal_shape() {
    assert!(matches!(Expr::bool(true), Expr::LitBool(true)));
}

// ------------------------------------------------------------------
// Wrapping helpers on Expr
// ------------------------------------------------------------------

#[test]
fn wrapping_add_method_produces_binop() {
    let e = Expr::u32(1).wrapping_add(Expr::u32(2));
    assert!(matches!(
        e,
        Expr::BinOp {
            op: BinOp::WrappingAdd,
            ..
        }
    ));
}

#[test]
fn wrapping_sub_method_produces_binop() {
    let e = Expr::u32(5).wrapping_sub(Expr::u32(3));
    assert!(matches!(
        e,
        Expr::BinOp {
            op: BinOp::WrappingSub,
            ..
        }
    ));
}
