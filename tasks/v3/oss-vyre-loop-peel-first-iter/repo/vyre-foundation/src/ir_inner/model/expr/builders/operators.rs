use super::ops::{binary, unary};
use crate::ir_inner::model::expr::Expr;
use crate::ir_inner::model::types::{BinOp, UnOp};

macro_rules! binary_builders {
    ($($(#[$meta:meta])* $name:ident => $op:expr;)*) => {
        $(
            $(#[$meta])*
            #[must_use]
            #[inline]
            pub fn $name(left: Expr, right: Expr) -> Expr {
                binary($op, left, right)
            }
        )*
    };
}

macro_rules! unary_builders {
    ($($(#[$meta:meta])* $name:ident => $op:expr;)*) => {
        $(
            $(#[$meta])*
            #[must_use]
            #[inline]
            pub fn $name(operand: Expr) -> Expr {
                unary($op, operand)
            }
        )*
    };
}

impl Expr {
    binary_builders! {
        /// `a + b`.
        add => BinOp::Add;
        /// `a - b`.
        sub => BinOp::Sub;
        /// `a * b`.
        mul => BinOp::Mul;
        /// `a / b`; zero divisors evaluate to the total-reference value.
        div => BinOp::Div;
        /// Upper 32 bits of `a * b` for unsigned widening multiply.
        mulhi => BinOp::MulHigh;
        /// `a % b`; zero divisors evaluate to the total-reference value.
        rem => BinOp::Mod;
        /// Unsigned absolute difference.
        abs_diff => BinOp::AbsDiff;
        /// Bitwise XOR.
        bitxor => BinOp::BitXor;
        /// Bitwise AND.
        bitand => BinOp::BitAnd;
        /// Bitwise OR.
        bitor => BinOp::BitOr;
        /// Shift left.
        shl => BinOp::Shl;
        /// Shift right.
        shr => BinOp::Shr;
        /// Rotate left by `right & 31` bits (32-bit barrel rotate).
        rotate_left => BinOp::RotateLeft;
        /// Rotate right by `right & 31` bits (32-bit barrel rotate).
        rotate_right => BinOp::RotateRight;
        /// Equality comparison.
        eq => BinOp::Eq;
        /// Strict less-than comparison.
        lt => BinOp::Lt;
        /// Inequality comparison.
        ne => BinOp::Ne;
        /// Strict greater-than comparison.
        gt => BinOp::Gt;
        /// Less-than-or-equal comparison.
        le => BinOp::Le;
        /// Greater-than-or-equal comparison.
        ge => BinOp::Ge;
        /// Logical AND.
        and => BinOp::And;
        /// Logical OR.
        or => BinOp::Or;
        /// `min(a, b)`.
        min => BinOp::Min;
        /// `max(a, b)`.
        max => BinOp::Max;
    }

    unary_builders! {
        /// Twos-complement negation.
        negate => UnOp::Negate;
        /// Bitwise NOT.
        bitnot => UnOp::BitNot;
        /// Reverse the bit order.
        reverse_bits => UnOp::ReverseBits;
        /// Count one bits.
        popcount => UnOp::Popcount;
        /// Count leading zero bits.
        clz => UnOp::Clz;
        /// Count trailing zero bits.
        ctz => UnOp::Ctz;
        /// Logical NOT.
        not => UnOp::LogicalNot;
        /// Sine.
        sin => UnOp::Sin;
        /// Cosine.
        cos => UnOp::Cos;
        /// Absolute value.
        abs => UnOp::Abs;
        /// Square root.
        sqrt => UnOp::Sqrt;
        /// Inverse square root.
        inverse_sqrt => UnOp::InverseSqrt;
        /// Reciprocal.
        reciprocal => UnOp::Reciprocal;
        /// Floor.
        floor => UnOp::Floor;
        /// Ceiling.
        ceil => UnOp::Ceil;
        /// Round to nearest.
        round => UnOp::Round;
        /// Truncate toward zero.
        trunc => UnOp::Trunc;
        /// Sign extraction.
        sign => UnOp::Sign;
        /// `isNan(a)`.
        is_nan => UnOp::IsNan;
        /// `isInf(a)`.
        is_inf => UnOp::IsInf;
        /// `isFinite(a)`.
        is_finite => UnOp::IsFinite;
    }

    /// `saturating_sub(a, b)` for unsigned operands; clamps to zero when
    /// `b > a` instead of underflowing.
    ///
    /// Emits `BinOp::SaturatingSub` (wire tag `0x17`) directly so that
    /// canonical fingerprints, optimizer identity rules, and the reference
    /// evaluator all see the same opcode regardless of how the expression was
    /// constructed. The WGSL lowering (`a - min(a, b)`) is the backend's
    /// concern, not the IR builder's.
    #[must_use]
    #[inline]
    pub fn saturating_sub(left: Expr, right: Expr) -> Expr {
        binary(BinOp::SaturatingSub, left, right)
    }

    /// `saturating_add(a, b)` for unsigned operands; clamps to `u32::MAX` on
    /// overflow instead of wrapping.
    ///
    /// Emits `BinOp::SaturatingAdd` (wire tag `0x16`) directly so the builder
    /// form and the direct-opcode form share one canonical fingerprint — the
    /// same first-class-opcode contract as [`Expr::saturating_sub`]. The WGSL
    /// lowering (overflow-detect `select`) is the backend's concern.
    #[must_use]
    #[inline]
    pub fn saturating_add(left: Expr, right: Expr) -> Expr {
        binary(BinOp::SaturatingAdd, left, right)
    }

    /// `saturating_mul(a, b)` for unsigned operands; clamps to `u32::MAX` on
    /// overflow instead of wrapping.
    ///
    /// Emits `BinOp::SaturatingMul` (wire tag `0x18`) directly, mirroring
    /// [`Expr::saturating_add`]/[`Expr::saturating_sub`].
    #[must_use]
    #[inline]
    pub fn saturating_mul(left: Expr, right: Expr) -> Expr {
        binary(BinOp::SaturatingMul, left, right)
    }

    /// Construct a wrapping addition node.
    #[must_use]
    #[inline]
    pub fn wrapping_add(self, other: impl Into<Expr>) -> Self {
        binary(BinOp::WrappingAdd, self, other.into())
    }

    /// Construct a wrapping subtraction node.
    #[must_use]
    #[inline]
    pub fn wrapping_sub(self, other: impl Into<Expr>) -> Self {
        binary(BinOp::WrappingSub, self, other.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Node, Program};

    /// `Expr::saturating_sub` must emit `BinOp::SaturatingSub` (wire tag 0x17)
    /// so that the builder form and the direct-opcode form produce identical
    /// canonical fingerprints. Before the fix the builder emitted
    /// `BinOp::Sub(a, BinOp::Min(a, b))` — a two-node tree that serialises to
    /// a completely different byte sequence, causing cache misses and preventing
    /// optimizer identity rules from firing.
    #[test]
    fn saturating_sub_builder_emits_saturating_sub_opcode() {
        let a = Expr::var("a");
        let b = Expr::var("b");

        // Builder form.
        let via_builder = Expr::saturating_sub(a.clone(), b.clone());

        // Direct opcode form.
        let via_opcode = Expr::BinOp {
            op: BinOp::SaturatingSub,
            left: Box::new(a),
            right: Box::new(b),
        };

        // They must be structurally identical so that their canonical fingerprints
        // (computed by Program::fingerprint via canonical wire bytes) agree.
        let make_program = |value: Expr| {
            Program::wrapped(
                vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
                [1, 1, 1],
                vec![Node::Store {
                    buffer: "out".into(),
                    index: Expr::u32(0),
                    value,
                }],
            )
        };

        let fp_builder = make_program(via_builder).fingerprint();
        let fp_opcode = make_program(via_opcode).fingerprint();

        assert_eq!(
            fp_builder, fp_opcode,
            "Expr::saturating_sub must emit BinOp::SaturatingSub so fingerprints agree"
        );
    }

    fn fingerprint_of(value: Expr) -> [u8; 32] {
        Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::Store {
                buffer: "out".into(),
                index: Expr::u32(0),
                value,
            }],
        )
        .fingerprint()
    }

    /// Each newly added first-class binop builder must emit its exact opcode so
    /// the builder form and the direct `Expr::BinOp` form share one canonical
    /// fingerprint — the same contract `saturating_sub` already holds. A builder
    /// that lowered to a multi-node idiom (e.g. rotate as shift-or) would
    /// serialise differently and silently break opcode-keyed optimizer rules.
    #[test]
    fn new_first_class_binop_builders_emit_their_opcode() {
        let cases: [(fn(Expr, Expr) -> Expr, BinOp); 4] = [
            (Expr::rotate_left, BinOp::RotateLeft),
            (Expr::rotate_right, BinOp::RotateRight),
            (Expr::saturating_add, BinOp::SaturatingAdd),
            (Expr::saturating_mul, BinOp::SaturatingMul),
        ];
        for (builder, op) in cases {
            let a = Expr::var("a");
            let b = Expr::var("b");
            let via_builder = builder(a.clone(), b.clone());
            let via_opcode = Expr::BinOp {
                op,
                left: Box::new(a),
                right: Box::new(b),
            };
            assert_eq!(
                fingerprint_of(via_builder),
                fingerprint_of(via_opcode),
                "Expr builder for {op:?} must emit that exact opcode so fingerprints agree"
            );
        }
    }
}
