//! Frozen unary-operation discriminants for primitive operation metadata.
// TAG RESERVATIONS: Negate=0x01, BitNot=0x02, LogicalNot=0x03,
// Popcount=0x04, Clz=0x05, Ctz=0x06, ReverseBits=0x07, Cos=0x08,
// Sin=0x09, Abs=0x0A, Sqrt=0x0B, Floor=0x0C, Ceil=0x0D, Round=0x0E,
// Trunc=0x0F, Sign=0x10, IsNan=0x11, IsInf=0x12, IsFinite=0x13,
// Exp=0x14, Log=0x15, Log2=0x16, Exp2=0x17, Tan=0x18, Acos=0x19,
// Asin=0x1A, Atan=0x1B, Tanh=0x1C, Sinh=0x1D, Cosh=0x1E,
// InverseSqrt=0x1F, Unpack4Low=0x20, Unpack4High=0x21,
// Unpack8Low=0x22, Unpack8High=0x23, Reciprocal=0x24,
// 0x25..=0x7F reserved, Opaque=0x80.
//
// Rotate ops are *binary* (operand + count), so they live on `BinOp`
// alongside `Shl`/`Shr`. See `vyre-spec::BinOp::RotateRight/RotateLeft`.

use crate::extension::ExtensionUnOpId;

/// Unary operation kind in the frozen data contract.
///
/// Example: `UnOp::ReverseBits` identifies a bit-reversal primitive without
/// binding the catalog to a backend intrinsic spelling.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub enum UnOp {
    /// Arithmetic negation.
    Negate,
    /// Bitwise NOT.
    BitNot,
    /// Logical NOT.
    LogicalNot,
    /// Count set bits.
    Popcount,
    /// Count leading zeros.
    Clz,
    /// Count trailing zeros.
    Ctz,
    /// Reverse all bits.
    ReverseBits,
    /// Cosine (f32).
    Cos,
    /// Sine (f32).
    Sin,
    /// Absolute value (f32).
    Abs,
    /// Square root (f32).
    Sqrt,
    /// Floor (f32).
    Floor,
    /// Ceil (f32).
    Ceil,
    /// Round (f32).
    Round,
    /// Trunc (f32).
    Trunc,
    /// Sign (f32).
    Sign,
    /// Is NaN (f32).
    IsNan,
    /// Is Inf (f32).
    IsInf,
    /// Is Finite (f32).
    IsFinite,
    /// Natural exponential (f32).
    Exp,
    /// Natural logarithm (f32).
    Log,
    /// Base-2 logarithm (f32).
    Log2,
    /// Base-2 exponential (f32).
    Exp2,
    /// Tangent (f32).
    Tan,
    /// Arc cosine (f32).
    Acos,
    /// Arc sine (f32).
    Asin,
    /// Arc tangent (f32).
    Atan,
    /// Hyperbolic tangent (f32).
    Tanh,
    /// Hyperbolic sine (f32).
    Sinh,
    /// Hyperbolic cosine (f32).
    Cosh,
    /// Reciprocal square root (f32).
    InverseSqrt,
    /// Unpack lower 4-bits of a u8 into a u32/f32.
    Unpack4Low,
    /// Unpack upper 4-bits of a u8 into a u32/f32.
    Unpack4High,
    /// Unpack lower 8-bits (byte 0) of a u32 into a u32/f32.
    Unpack8Low,
    /// Unpack upper 8-bits (byte 3) of a u32 into a u32/f32.
    Unpack8High,
    /// Reciprocal (f32).
    Reciprocal,
    /// Extension-declared unary operator.
    ///
    /// The `ExtensionUnOpId` resolves via the vyre-core extension
    /// registry to a `&'static dyn ExtensionUnOp` with per-backend
    /// lowerings. Wire encoding is `0x80 ++ u32 extension_id`.
    Opaque(ExtensionUnOpId),
}

impl_builtin_wire_tag!(UnOp, Opaque, {
    Negate => 0x01,
    BitNot => 0x02,
    LogicalNot => 0x03,
    Popcount => 0x04,
    Clz => 0x05,
    Ctz => 0x06,
    ReverseBits => 0x07,
    Cos => 0x08,
    Sin => 0x09,
    Abs => 0x0A,
    Sqrt => 0x0B,
    Floor => 0x0C,
    Ceil => 0x0D,
    Round => 0x0E,
    Trunc => 0x0F,
    Sign => 0x10,
    IsNan => 0x11,
    IsInf => 0x12,
    IsFinite => 0x13,
    Exp => 0x14,
    Log => 0x15,
    Log2 => 0x16,
    Exp2 => 0x17,
    Tan => 0x18,
    Acos => 0x19,
    Asin => 0x1A,
    Atan => 0x1B,
    Tanh => 0x1C,
    Sinh => 0x1D,
    Cosh => 0x1E,
    InverseSqrt => 0x1F,
    Unpack4Low => 0x20,
    Unpack4High => 0x21,
    Unpack8Low => 0x22,
    Unpack8High => 0x23,
    Reciprocal => 0x24,
});
