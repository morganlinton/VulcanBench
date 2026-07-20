//! Backend-neutral literal evaluation for the vyre IR.
//!
//! This module owns scalar constant evaluation semantics that must match across
//! optimizer passes and concrete lowerings. Backends may use the recursive
//! folder before emission to avoid target-language constant-evaluation traps,
//! but the rules themselves stay in foundation.

use std::borrow::Cow;

use crate::ir::{BinOp, DataType, Expr, UnOp};

/// Recursively fold a literal-only expression tree.
#[must_use]
pub fn fold_literal_tree(expr: &Expr) -> Option<Cow<'_, Expr>> {
    match expr {
        Expr::BinOp { op, left, right } => {
            let folded_left = fold_literal_tree(left);
            let folded_right = fold_literal_tree(right);
            let left = folded_left.as_deref().unwrap_or(left.as_ref());
            let right = folded_right.as_deref().unwrap_or(right.as_ref());
            fold_binary_literal(op, left, right).map(Cow::Owned)
        }
        Expr::Fma { a, b, c } => {
            let folded_a = fold_literal_tree(a);
            let folded_b = fold_literal_tree(b);
            let folded_c = fold_literal_tree(c);
            let a = folded_a.as_deref().unwrap_or(a.as_ref());
            let b = folded_b.as_deref().unwrap_or(b.as_ref());
            let c = folded_c.as_deref().unwrap_or(c.as_ref());
            fold_fma_literal(a, b, c).map(Cow::Owned)
        }
        Expr::UnOp { op, operand } => {
            let folded_operand = fold_literal_tree(operand);
            let operand = folded_operand.as_deref().unwrap_or(operand.as_ref());
            fold_unary_literal(op, operand).map(Cow::Owned)
        }
        Expr::Cast { target, value } => {
            let folded_value = fold_literal_tree(value);
            let value = folded_value.as_deref().unwrap_or(value.as_ref());
            fold_cast_literal(target, value).map(Cow::Owned)
        }
        _ => None,
    }
}

/// Fold one binary operator applied to literal operands.
#[must_use]
pub fn fold_binary_literal(op: &BinOp, left: &Expr, right: &Expr) -> Option<Expr> {
    match (left, right) {
        (Expr::LitU32(a), Expr::LitU32(b)) => fold_u32_binary(*op, *a, *b),
        (Expr::LitI32(a), Expr::LitI32(b)) => fold_i32_binary(*op, *a, *b),
        (Expr::LitBool(a), Expr::LitBool(b)) => fold_bool_binary(*op, *a, *b),
        (Expr::LitF32(a), Expr::LitF32(b)) => fold_f32_binary(*op, *a, *b),
        _ => None,
    }
}

/// Fold one unary operator applied to a literal operand.
#[must_use]
pub fn fold_unary_literal(op: &UnOp, operand: &Expr) -> Option<Expr> {
    match operand {
        Expr::LitU32(value) => fold_u32_unary(op, *value),
        Expr::LitI32(value) => fold_i32_unary(op, *value),
        Expr::LitBool(value) => fold_bool_unary(op, *value),
        Expr::LitF32(value) => fold_f32_unary(op, *value),
        _ => None,
    }
}

/// Fold a cast applied to a literal operand.
#[must_use]
pub fn fold_cast_literal(target: &DataType, value: &Expr) -> Option<Expr> {
    match (target, value) {
        (DataType::U32, Expr::LitU32(v)) => Some(Expr::LitU32(*v)),
        (DataType::U32, Expr::LitI32(v)) => Some(Expr::LitU32(*v as u32)),
        (DataType::U32, Expr::LitF32(v)) if v.is_finite() => Some(Expr::LitU32(*v as u32)),
        (DataType::U32, Expr::LitBool(v)) => Some(Expr::LitU32(u32::from(*v))),
        (DataType::I32, Expr::LitU32(v)) => Some(Expr::LitI32(*v as i32)),
        (DataType::I32, Expr::LitI32(v)) => Some(Expr::LitI32(*v)),
        (DataType::I32, Expr::LitF32(v)) if v.is_finite() => Some(Expr::LitI32(*v as i32)),
        (DataType::I32, Expr::LitBool(v)) => Some(Expr::LitI32(i32::from(*v))),
        (DataType::F32, Expr::LitU32(v)) => Some(Expr::LitF32(*v as f32)),
        (DataType::F32, Expr::LitI32(v)) => Some(Expr::LitF32(*v as f32)),
        (DataType::F32, Expr::LitF32(v)) => Some(Expr::LitF32(*v)),
        (DataType::F32, Expr::LitBool(v)) => Some(Expr::LitF32(if *v { 1.0 } else { 0.0 })),
        (DataType::Bool, Expr::LitU32(v)) => Some(Expr::LitBool(*v != 0)),
        (DataType::Bool, Expr::LitI32(v)) => Some(Expr::LitBool(*v != 0)),
        (DataType::Bool, Expr::LitF32(v)) => Some(Expr::LitBool(*v != 0.0)),
        (DataType::Bool, Expr::LitBool(v)) => Some(Expr::LitBool(*v)),
        _ => None,
    }
}

/// Fold an FMA with literal operands.
#[must_use]
pub fn fold_fma_literal(a: &Expr, b: &Expr, c: &Expr) -> Option<Expr> {
    match (a, b, c) {
        (Expr::LitF32(a), Expr::LitF32(b), Expr::LitF32(c))
            if !(a.is_nan() || b.is_nan() || c.is_nan()) =>
        {
            Some(Expr::LitF32(canonical_f32(a.mul_add(*b, *c))))
        }
        _ => None,
    }
}

#[must_use]
pub(crate) fn canonical_f32(value: f32) -> f32 {
    if value.is_nan() {
        f32::from_bits(0x7FC0_0000)
    } else if value.is_subnormal() {
        f32::from_bits(value.to_bits() & 0x8000_0000)
    } else {
        value
    }
}

fn fold_u32_binary(op: BinOp, a: u32, b: u32) -> Option<Expr> {
    Some(match op {
        BinOp::Add | BinOp::WrappingAdd => Expr::LitU32(a.wrapping_add(b)),
        BinOp::Sub | BinOp::WrappingSub => Expr::LitU32(a.wrapping_sub(b)),
        BinOp::Mul => Expr::LitU32(a.wrapping_mul(b)),
        BinOp::Div => Expr::LitU32(if b == 0 { u32::MAX } else { a / b }),
        BinOp::Mod => Expr::LitU32(if b == 0 { 0 } else { a % b }),
        BinOp::BitAnd => Expr::LitU32(a & b),
        BinOp::BitOr => Expr::LitU32(a | b),
        BinOp::BitXor => Expr::LitU32(a ^ b),
        BinOp::Shl => Expr::LitU32(a.wrapping_shl(b % 32)),
        BinOp::Shr => Expr::LitU32(a.wrapping_shr(b % 32)),
        BinOp::Eq => Expr::LitBool(a == b),
        BinOp::Ne => Expr::LitBool(a != b),
        BinOp::Lt => Expr::LitBool(a < b),
        BinOp::Gt => Expr::LitBool(a > b),
        BinOp::Le => Expr::LitBool(a <= b),
        BinOp::Ge => Expr::LitBool(a >= b),
        BinOp::And => Expr::LitBool(a != 0 && b != 0),
        BinOp::Or => Expr::LitBool(a != 0 || b != 0),
        BinOp::Min => Expr::LitU32(a.min(b)),
        BinOp::Max => Expr::LitU32(a.max(b)),
        BinOp::AbsDiff => Expr::LitU32(a.abs_diff(b)),
        BinOp::SaturatingAdd => Expr::LitU32(a.saturating_add(b)),
        BinOp::SaturatingSub => Expr::LitU32(a.saturating_sub(b)),
        BinOp::SaturatingMul => Expr::LitU32(a.saturating_mul(b)),
        BinOp::RotateLeft => Expr::LitU32(a.rotate_left(b % 32)),
        BinOp::RotateRight => Expr::LitU32(a.rotate_right(b % 32)),
        BinOp::MulHigh => Expr::LitU32(((a as u64).wrapping_mul(b as u64) >> 32) as u32),
        _ => return None,
    })
}

fn fold_i32_binary(op: BinOp, a: i32, b: i32) -> Option<Expr> {
    Some(match op {
        BinOp::Add | BinOp::WrappingAdd => Expr::LitI32(a.wrapping_add(b)),
        BinOp::Sub | BinOp::WrappingSub => Expr::LitI32(a.wrapping_sub(b)),
        BinOp::Mul => Expr::LitI32(a.wrapping_mul(b)),
        // I32 Div / Mod: deterministic target-safe folding that matches the
        // dynamic integer division contract used by target emitters.
        // divisor == 0 → 0 (both Div and Mod). i32::MIN / -1 → i32::MIN
        // (Rust's wrapping_div), i32::MIN % -1 → 0 (Rust's wrapping_rem).
        BinOp::Div => {
            if b == 0 {
                Expr::LitI32(0)
            } else {
                Expr::LitI32(a.wrapping_div(b))
            }
        }
        BinOp::Mod => {
            if b == 0 {
                Expr::LitI32(0)
            } else {
                Expr::LitI32(a.wrapping_rem(b))
            }
        }
        BinOp::BitAnd => Expr::LitI32(a & b),
        BinOp::BitOr => Expr::LitI32(a | b),
        BinOp::BitXor => Expr::LitI32(a ^ b),
        BinOp::Shl => {
            if b < 0 {
                return None;
            }
            Expr::LitI32(a.wrapping_shl((b as u32) % 32))
        }
        BinOp::Shr => {
            if b < 0 {
                return None;
            }
            Expr::LitI32(a.wrapping_shr((b as u32) % 32))
        }
        BinOp::Eq => Expr::LitBool(a == b),
        BinOp::Ne => Expr::LitBool(a != b),
        BinOp::Lt => Expr::LitBool(a < b),
        BinOp::Gt => Expr::LitBool(a > b),
        BinOp::Le => Expr::LitBool(a <= b),
        BinOp::Ge => Expr::LitBool(a >= b),
        BinOp::And => Expr::LitBool(a != 0 && b != 0),
        BinOp::Or => Expr::LitBool(a != 0 || b != 0),
        BinOp::Min => Expr::LitI32(a.min(b)),
        BinOp::Max => Expr::LitI32(a.max(b)),
        BinOp::AbsDiff => Expr::LitU32(a.abs_diff(b)),
        BinOp::SaturatingAdd => Expr::LitI32(a.saturating_add(b)),
        BinOp::SaturatingSub => Expr::LitI32(a.saturating_sub(b)),
        BinOp::SaturatingMul => Expr::LitI32(a.saturating_mul(b)),
        BinOp::RotateLeft => Expr::LitI32(a.rotate_left((b as u32) % 32)),
        BinOp::RotateRight => Expr::LitI32(a.rotate_right((b as u32) % 32)),
        _ => return None,
    })
}

fn fold_bool_binary(op: BinOp, a: bool, b: bool) -> Option<Expr> {
    Some(match op {
        BinOp::And => Expr::LitBool(a && b),
        BinOp::Or => Expr::LitBool(a || b),
        BinOp::BitXor => Expr::LitBool(a ^ b),
        BinOp::Eq => Expr::LitBool(a == b),
        BinOp::Ne => Expr::LitBool(a != b),
        _ => return None,
    })
}

fn fold_f32_binary(op: BinOp, a: f32, b: f32) -> Option<Expr> {
    let a = canonical_f32(a);
    let b = canonical_f32(b);
    if a.is_nan() || b.is_nan() {
        return None;
    }
    Some(match op {
        BinOp::Add => Expr::LitF32(canonical_f32(a + b)),
        BinOp::Sub => Expr::LitF32(canonical_f32(a - b)),
        BinOp::Mul => Expr::LitF32(canonical_f32(a * b)),
        BinOp::Div => {
            if b == 0.0 {
                return None;
            }
            Expr::LitF32(canonical_f32(a / b))
        }
        BinOp::Mod => {
            if b == 0.0 {
                return None;
            }
            Expr::LitF32(canonical_f32(a % b))
        }
        BinOp::Eq => Expr::LitBool(a == b),
        BinOp::Ne => Expr::LitBool(a != b),
        BinOp::Lt => Expr::LitBool(a < b),
        BinOp::Gt => Expr::LitBool(a > b),
        BinOp::Le => Expr::LitBool(a <= b),
        BinOp::Ge => Expr::LitBool(a >= b),
        BinOp::Min => Expr::LitF32(canonical_f32(a.min(b))),
        BinOp::Max => Expr::LitF32(canonical_f32(a.max(b))),
        _ => return None,
    })
}

fn fold_u32_unary(op: &UnOp, v: u32) -> Option<Expr> {
    Some(match op {
        UnOp::Negate => Expr::LitU32(v.wrapping_neg()),
        UnOp::BitNot => Expr::LitU32(!v),
        UnOp::LogicalNot => Expr::LitBool(v == 0),
        UnOp::Popcount => Expr::LitU32(v.count_ones()),
        UnOp::Clz => Expr::LitU32(v.leading_zeros()),
        UnOp::Ctz => Expr::LitU32(v.trailing_zeros()),
        UnOp::ReverseBits => Expr::LitU32(v.reverse_bits()),
        UnOp::Abs => Expr::LitU32(v),
        UnOp::Sign => Expr::LitF32(if v == 0 { 0.0 } else { 1.0 }),
        UnOp::Sqrt => Expr::LitF32(libm::sqrtf(v as f32)),
        UnOp::InverseSqrt => Expr::LitF32(1.0 / libm::sqrtf(v as f32)),
        UnOp::Reciprocal => Expr::LitF32(1.0 / v as f32),
        UnOp::Exp => Expr::LitF32(libm::expf(v as f32)),
        UnOp::Exp2 => Expr::LitF32(libm::exp2f(v as f32)),
        UnOp::Log => Expr::LitF32(libm::logf(v as f32)),
        UnOp::Log2 => Expr::LitF32(libm::log2f(v as f32)),
        UnOp::Sin => Expr::LitF32(libm::sinf(v as f32)),
        UnOp::Cos => Expr::LitF32(libm::cosf(v as f32)),
        UnOp::Tan => Expr::LitF32(libm::tanf(v as f32)),
        UnOp::Asin => Expr::LitF32(libm::asinf(v as f32)),
        UnOp::Acos => Expr::LitF32(libm::acosf(v as f32)),
        UnOp::Atan => Expr::LitF32(libm::atanf(v as f32)),
        UnOp::Sinh => Expr::LitF32(libm::sinhf(v as f32)),
        UnOp::Cosh => Expr::LitF32(libm::coshf(v as f32)),
        UnOp::Tanh => Expr::LitF32(libm::tanhf(v as f32)),
        UnOp::Floor | UnOp::Ceil | UnOp::Round | UnOp::Trunc => Expr::LitF32(v as f32),
        UnOp::IsNan => Expr::LitBool(false),
        UnOp::IsInf => Expr::LitBool(false),
        UnOp::IsFinite => Expr::LitBool(true),
        UnOp::Unpack4Low => Expr::LitU32(v & 0x0F),
        UnOp::Unpack4High => Expr::LitU32((v >> 4) & 0x0F),
        UnOp::Unpack8Low => Expr::LitU32(v & 0xFF),
        UnOp::Unpack8High => Expr::LitU32((v >> 24) & 0xFF),
        _ => return None,
    })
}

fn fold_i32_unary(op: &UnOp, v: i32) -> Option<Expr> {
    Some(match op {
        UnOp::Negate => Expr::LitI32(v.wrapping_neg()),
        UnOp::BitNot => Expr::LitI32(!v),
        UnOp::LogicalNot => Expr::LitBool(v == 0),
        UnOp::Popcount => Expr::LitI32(v.count_ones() as i32),
        UnOp::Clz => Expr::LitI32(v.leading_zeros() as i32),
        UnOp::Ctz => Expr::LitI32(v.trailing_zeros() as i32),
        UnOp::ReverseBits => Expr::LitI32(v.reverse_bits()),
        UnOp::Abs => Expr::LitI32(v.wrapping_abs()),
        UnOp::Sign => Expr::LitF32(if v == 0 { 0.0 } else { v.signum() as f32 }),
        UnOp::Sqrt => Expr::LitF32(libm::sqrtf(v as f32)),
        UnOp::InverseSqrt => Expr::LitF32(1.0 / libm::sqrtf(v as f32)),
        UnOp::Reciprocal => Expr::LitF32(1.0 / v as f32),
        UnOp::Exp => Expr::LitF32(libm::expf(v as f32)),
        UnOp::Exp2 => Expr::LitF32(libm::exp2f(v as f32)),
        UnOp::Log => Expr::LitF32(libm::logf(v as f32)),
        UnOp::Log2 => Expr::LitF32(libm::log2f(v as f32)),
        UnOp::Sin => Expr::LitF32(libm::sinf(v as f32)),
        UnOp::Cos => Expr::LitF32(libm::cosf(v as f32)),
        UnOp::Tan => Expr::LitF32(libm::tanf(v as f32)),
        UnOp::Asin => Expr::LitF32(libm::asinf(v as f32)),
        UnOp::Acos => Expr::LitF32(libm::acosf(v as f32)),
        UnOp::Atan => Expr::LitF32(libm::atanf(v as f32)),
        UnOp::Sinh => Expr::LitF32(libm::sinhf(v as f32)),
        UnOp::Cosh => Expr::LitF32(libm::coshf(v as f32)),
        UnOp::Tanh => Expr::LitF32(libm::tanhf(v as f32)),
        UnOp::Floor | UnOp::Ceil | UnOp::Round | UnOp::Trunc => Expr::LitF32(v as f32),
        UnOp::IsNan => Expr::LitBool(false),
        UnOp::IsInf => Expr::LitBool(false),
        UnOp::IsFinite => Expr::LitBool(true),
        _ => return None,
    })
}

fn fold_bool_unary(op: &UnOp, v: bool) -> Option<Expr> {
    Some(match op {
        UnOp::LogicalNot | UnOp::BitNot => Expr::LitBool(!v),
        UnOp::IsNan | UnOp::IsInf => Expr::LitBool(false),
        UnOp::IsFinite => Expr::LitBool(true),
        _ => return None,
    })
}

fn fold_f32_unary(op: &UnOp, v: f32) -> Option<Expr> {
    let v = canonical_f32(v);
    if v.is_nan() && !matches!(op, UnOp::IsNan | UnOp::IsInf | UnOp::IsFinite) {
        return None;
    }
    Some(match op {
        UnOp::Negate => Expr::LitF32(canonical_f32(-v)),
        UnOp::Sqrt => Expr::LitF32(canonical_f32(libm::sqrtf(v))),
        UnOp::InverseSqrt => Expr::LitF32(canonical_f32(1.0 / libm::sqrtf(v))),
        UnOp::Reciprocal => Expr::LitF32(canonical_f32(1.0 / v)),
        UnOp::Exp => Expr::LitF32(canonical_f32(libm::expf(v))),
        UnOp::Exp2 => Expr::LitF32(canonical_f32(libm::exp2f(v))),
        UnOp::Log => Expr::LitF32(canonical_f32(libm::logf(v))),
        UnOp::Log2 => Expr::LitF32(canonical_f32(libm::log2f(v))),
        UnOp::Sin => Expr::LitF32(canonical_f32(libm::sinf(v))),
        UnOp::Cos => Expr::LitF32(canonical_f32(libm::cosf(v))),
        UnOp::Tan => Expr::LitF32(canonical_f32(libm::tanf(v))),
        UnOp::Asin => Expr::LitF32(canonical_f32(libm::asinf(v))),
        UnOp::Acos => Expr::LitF32(canonical_f32(libm::acosf(v))),
        UnOp::Atan => Expr::LitF32(canonical_f32(libm::atanf(v))),
        UnOp::Sinh => Expr::LitF32(canonical_f32(libm::sinhf(v))),
        UnOp::Cosh => Expr::LitF32(canonical_f32(libm::coshf(v))),
        UnOp::Tanh => Expr::LitF32(canonical_f32(libm::tanhf(v))),
        UnOp::Ceil => Expr::LitF32(canonical_f32(v.ceil())),
        UnOp::Floor => Expr::LitF32(canonical_f32(v.floor())),
        UnOp::Round => Expr::LitF32(canonical_f32(v.round())),
        UnOp::Trunc => Expr::LitF32(canonical_f32(v.trunc())),
        UnOp::Abs => Expr::LitF32(canonical_f32(v.abs())),
        UnOp::Sign => Expr::LitF32(canonical_f32(if v == 0.0 { 0.0 } else { v.signum() })),
        UnOp::IsNan => Expr::LitBool(v.is_nan()),
        UnOp::IsInf => Expr::LitBool(v.is_infinite()),
        UnOp::IsFinite => Expr::LitBool(v.is_finite()),
        UnOp::LogicalNot => Expr::LitBool(v == 0.0),
        _ => return None,
    })
}
