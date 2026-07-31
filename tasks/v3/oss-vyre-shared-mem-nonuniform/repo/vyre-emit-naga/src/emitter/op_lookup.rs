//! Pure lookup tables: vyre IR ops → naga BinaryOperator / UnaryOperator
//! / MathFunction handles, plus literal and barrier-flag conversion.
//! No state  -  the contents are direct enum mappings.

use crate::EmitError;
use naga::{BinaryOperator, Literal, ScalarKind, UnaryOperator};
use vyre_foundation::ir::{BinOp, DataType, UnOp};
use vyre_foundation::memory_model::MemoryOrdering;
use vyre_lower::LiteralValue;

pub(super) fn naga_literal(literal: &LiteralValue) -> Result<Literal, EmitError> {
    match literal {
        LiteralValue::U32(value) => Ok(Literal::U32(*value)),
        LiteralValue::I32(value) => Ok(Literal::I32(*value)),
        LiteralValue::F32(value) if value.is_finite() => Ok(Literal::F32(*value)),
        LiteralValue::F32(value) => Err(EmitError::InvalidDescriptor(format!(
            "f32 literal {value:?} is not finite; Naga literals cannot represent NaN/Inf"
        ))),
        LiteralValue::Bool(value) => Ok(Literal::Bool(*value)),
    }
}

pub(super) fn binary_operator(op: BinOp) -> Result<BinaryOperator, EmitError> {
    Ok(match op {
        BinOp::Add | BinOp::WrappingAdd => BinaryOperator::Add,
        BinOp::Sub | BinOp::WrappingSub => BinaryOperator::Subtract,
        BinOp::Mul => BinaryOperator::Multiply,
        BinOp::Div => BinaryOperator::Divide,
        BinOp::Mod => BinaryOperator::Modulo,
        BinOp::Eq => BinaryOperator::Equal,
        BinOp::Ne => BinaryOperator::NotEqual,
        BinOp::Lt => BinaryOperator::Less,
        BinOp::Le => BinaryOperator::LessEqual,
        BinOp::Gt => BinaryOperator::Greater,
        BinOp::Ge => BinaryOperator::GreaterEqual,
        // BinOp::And / Or always emit bitwise. WGSL bitwise And/Or on
        // bool operands returns bool with the LogicalAnd/Or truth
        // table (no short-circuit, but vyre IR doesn't model
        // short-circuit anyway). This is the only mapping that's
        // accepted by naga across the bool+bool, u32+u32, and mixed
        // bool+u32 (post-widen) operand shapes the BinOpKind arm
        // produces. LogicalAnd/Or would reject u32 operands outright.
        BinOp::And => BinaryOperator::And,
        BinOp::Or => BinaryOperator::InclusiveOr,
        BinOp::BitAnd => BinaryOperator::And,
        BinOp::BitOr => BinaryOperator::InclusiveOr,
        BinOp::BitXor => BinaryOperator::ExclusiveOr,
        BinOp::Shl => BinaryOperator::ShiftLeft,
        BinOp::Shr => BinaryOperator::ShiftRight,
        other => {
            return Err(EmitError::NagaConstructionFailed(format!(
                "binary op `{other:?}` has no direct Naga operator"
            )))
        }
    })
}

pub(super) fn unary_operator(op: &UnOp) -> Result<UnaryOperator, EmitError> {
    Ok(match op {
        UnOp::Negate => UnaryOperator::Negate,
        UnOp::LogicalNot => UnaryOperator::LogicalNot,
        UnOp::BitNot => UnaryOperator::BitwiseNot,
        other => {
            return Err(EmitError::NagaConstructionFailed(format!(
                "unary op `{other:?}` has no direct Naga unary operator"
            )))
        }
    })
}

/// Map BinOps that compile to `Expression::Math` (WGSL builtin
/// functions) instead of the basic `BinaryOperator` enum. Returns
/// `None` for ops that already have a direct binary-operator form.
pub(super) fn binary_math_function(op: BinOp) -> Option<naga::MathFunction> {
    Some(match op {
        BinOp::Min => naga::MathFunction::Min,
        BinOp::Max => naga::MathFunction::Max,
        // Saturating arithmetic + AbsDiff are emitted via the same
        // builtin path; Naga lowers them to wgsl `min(max(...))` etc.
        BinOp::SaturatingAdd | BinOp::SaturatingSub | BinOp::SaturatingMul | BinOp::AbsDiff => {
            return None;
        }
        _ => return None,
    })
}

/// Map UnOps that compile to `Expression::Math` (WGSL builtin
/// functions) instead of the basic `UnaryOperator` enum.
pub(super) fn unary_math_function(op: &UnOp) -> Option<naga::MathFunction> {
    Some(match op {
        UnOp::Sqrt => naga::MathFunction::Sqrt,
        UnOp::InverseSqrt => naga::MathFunction::InverseSqrt,
        UnOp::Abs => naga::MathFunction::Abs,
        UnOp::Sin => naga::MathFunction::Sin,
        UnOp::Cos => naga::MathFunction::Cos,
        UnOp::Tan => naga::MathFunction::Tan,
        UnOp::Asin => naga::MathFunction::Asin,
        UnOp::Acos => naga::MathFunction::Acos,
        UnOp::Atan => naga::MathFunction::Atan,
        UnOp::Sinh => naga::MathFunction::Sinh,
        UnOp::Cosh => naga::MathFunction::Cosh,
        UnOp::Tanh => naga::MathFunction::Tanh,
        UnOp::Exp => naga::MathFunction::Exp,
        UnOp::Exp2 => naga::MathFunction::Exp2,
        UnOp::Log => naga::MathFunction::Log,
        UnOp::Log2 => naga::MathFunction::Log2,
        UnOp::Floor => naga::MathFunction::Floor,
        UnOp::Ceil => naga::MathFunction::Ceil,
        UnOp::Round => naga::MathFunction::Round,
        UnOp::Trunc => naga::MathFunction::Trunc,
        UnOp::Sign => naga::MathFunction::Sign,
        UnOp::Popcount => naga::MathFunction::CountOneBits,
        UnOp::Clz => naga::MathFunction::CountLeadingZeros,
        UnOp::Ctz => naga::MathFunction::CountTrailingZeros,
        UnOp::ReverseBits => naga::MathFunction::ReverseBits,
        _ => return None,
    })
}

/// The `(right_shift, and_mask)` lowering for the nibble/byte unpack UnOps.
/// WGSL/Naga has no pack/unpack intrinsic, so these compile to an explicit
/// `(value >> shift) & mask` on u32. The semantics mirror `ir_eval` exactly:
/// `Unpack4Low = v & 0x0F`, `Unpack4High = (v >> 4) & 0x0F`,
/// `Unpack8Low = v & 0xFF`, `Unpack8High = (v >> 24) & 0xFF`.
pub(super) fn unpack_shift_mask(op: &UnOp) -> Option<(u32, u32)> {
    Some(match op {
        UnOp::Unpack4Low => (0, 0x0F),
        UnOp::Unpack4High => (4, 0x0F),
        UnOp::Unpack8Low => (0, 0xFF),
        UnOp::Unpack8High => (24, 0xFF),
        _ => return None,
    })
}

pub(super) fn scalar_cast_target(target: &DataType) -> Result<(ScalarKind, u8), EmitError> {
    match target {
        DataType::Bool => Ok((ScalarKind::Bool, 1)),
        DataType::U8 | DataType::U16 | DataType::U32 => Ok((ScalarKind::Uint, 4)),
        DataType::I8 | DataType::I16 | DataType::I32 => Ok((ScalarKind::Sint, 4)),
        DataType::F32 => Ok((ScalarKind::Float, 4)),
        // `Bytes` is a packed-byte buffer-element marker, NOT a castable scalar.
        // The foundation cast table (`validate::cast::cast_is_valid`) rejects
        // every cast to/from `Bytes`, but the Program-compatibility `emit_module`
        // path does NOT run full validation, so a `Cast { target: Bytes }` can
        // reach here. Mapping it to a 32-bit uint would SILENTLY reinterpret a
        // byte target as a word (Law 10). Fail closed and name the fix.
        DataType::Bytes => Err(EmitError::NagaConstructionFailed(
            "cast target `Bytes` is not a scalar: `Bytes` is a packed-byte buffer \
             element and must be unpacked via a pack-to-u32 pre-pass before \
             emission, never reinterpreted as a u32 word"
                .to_owned(),
        )),
        other => Err(EmitError::NagaConstructionFailed(format!(
            "cast target `{other:?}` is not supported by the scalar Naga emitter"
        ))),
    }
}

pub(super) fn barrier_flags(ordering: MemoryOrdering) -> Result<naga::Barrier, EmitError> {
    match ordering {
        MemoryOrdering::Acquire
        | MemoryOrdering::Release
        | MemoryOrdering::AcqRel
        | MemoryOrdering::SeqCst => Ok(naga::Barrier::STORAGE | naga::Barrier::WORK_GROUP),
        MemoryOrdering::Relaxed => Err(EmitError::InvalidDescriptor(
            "relaxed barrier has no synchronization semantics".to_owned(),
        )),
        MemoryOrdering::GridSync => Err(EmitError::NagaConstructionFailed(
            "grid synchronization requires dispatch splitting before Naga emission".to_owned(),
        )),
        _ => Err(EmitError::NagaConstructionFailed(
            "future memory ordering is not mapped by the Naga emitter".to_owned(),
        )),
    }
}
