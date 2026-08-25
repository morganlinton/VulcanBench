//! IEEE-754 `f32` operation semantics for the reference interpreter.

use crate::ieee754;
use vyre::ir::{BinOp, UnOp};
use vyre::Error;

use crate::value::Value;

pub(super) fn binop_f32(op: BinOp, left: f32, right: f32) -> Result<Value, vyre::Error> {
    let left = canonical_f32(left);
    let right = canonical_f32(right);
    let wrap = |v: f32| Value::Float(f64::from(canonical_f32(v)));
    match op {
        BinOp::Add => Ok(wrap(left + right)),
        BinOp::Sub => Ok(wrap(left - right)),
        BinOp::Mul => Ok(wrap(left * right)),
        BinOp::Div => Ok(wrap(left / right)),
        BinOp::Min => Ok(wrap(f32::min(left, right))),
        BinOp::Max => Ok(wrap(f32::max(left, right))),
        BinOp::Eq => Ok(Value::Bool(left == right)),
        BinOp::Ne => Ok(Value::Bool(left != right)),
        BinOp::Lt => Ok(Value::Bool(left < right)),
        BinOp::Gt => Ok(Value::Bool(left > right)),
        BinOp::Le => Ok(Value::Bool(left <= right)),
        BinOp::Ge => Ok(Value::Bool(left >= right)),
        _ => Err(Error::interp(format!(
            "binary op `{op:?}` is not defined for f32 operands. Fix: use arithmetic or comparison ops only for float primitives."
        ))),
    }
}

pub(super) fn unop_f32(op: &UnOp, value: f32) -> Result<Value, vyre::Error> {
    let value = canonical_f32(value);
    let wrap = |v: f32| Value::Float(f64::from(canonical_f32(v)));
    match op {
        UnOp::Negate => Ok(wrap(-value)),
        UnOp::Abs => Ok(wrap(value.abs())),
        UnOp::Sqrt => Ok(wrap(ieee754::canonical_sqrt(value))),
        UnOp::InverseSqrt => Ok(wrap(ieee754::canonical_inverse_sqrt(value))),
        UnOp::Reciprocal => Ok(wrap(ieee754::canonical_reciprocal(value))),
        UnOp::Sin => Ok(wrap(ieee754::canonical_sin(value))),
        UnOp::Cos => Ok(wrap(ieee754::canonical_cos(value))),
        UnOp::Floor => Ok(wrap(value.floor())),
        UnOp::Ceil => Ok(wrap(value.ceil())),
        UnOp::Round => Ok(wrap(value.round())),
        UnOp::Trunc => Ok(wrap(value.trunc())),
        UnOp::Sign => Ok(wrap(sign(value))),
        UnOp::IsNan => Ok(Value::Bool(value.is_nan())),
        UnOp::IsInf => Ok(Value::Bool(value.is_infinite())),
        UnOp::IsFinite => Ok(Value::Bool(value.is_finite())),
        // V7-CORR-005: softmax + attention emit Expr::UnOp { op: Exp, .. }
        // and need a reference eval path so CPU ref executes cleanly.
        UnOp::Exp => Ok(wrap(ieee754::canonical_exp(value))),
        UnOp::Log => Ok(wrap(ieee754::canonical_log(value))),
        UnOp::Log2 => Ok(wrap(ieee754::canonical_log2(value))),
        UnOp::Exp2 => Ok(wrap(ieee754::canonical_exp2(value))),
        UnOp::Tan => Ok(wrap(ieee754::canonical_tan(value))),
        UnOp::Acos => Ok(wrap(ieee754::canonical_acos(value))),
        UnOp::Asin => Ok(wrap(ieee754::canonical_asin(value))),
        UnOp::Atan => Ok(wrap(ieee754::canonical_atan(value))),
        UnOp::Tanh => Ok(wrap(ieee754::canonical_tanh(value))),
        UnOp::Sinh => Ok(wrap(ieee754::canonical_sinh(value))),
        UnOp::Cosh => Ok(wrap(ieee754::canonical_cosh(value))),
        _ => Err(Error::interp(format!(
            "unary op `{op:?}` is not defined for f32 operands. Fix: use numeric or IEEE-754 classification ops only for float primitives."
        ))),
    }
}

fn sign(value: f32) -> f32 {
    if value.is_nan() {
        f32::NAN
    } else if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}

pub(crate) fn canonical_f32(value: f32) -> f32 {
    if value.is_nan() {
        f32::from_bits(0x7FC0_0000)
    } else if value.is_subnormal() {
        f32::from_bits(value.to_bits() & 0x8000_0000)
    } else {
        value
    }
}
