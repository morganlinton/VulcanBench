use std::fmt::Write as _;

use crate::reg::{PtxType, Reg};
use vyre_foundation::ir::{BinOp, DataType};

pub(super) fn is_ptx_vectorizable_dtype(dt: &DataType) -> bool {
    matches!(
        dt,
        DataType::U32 | DataType::I32 | DataType::F32 | DataType::Bool
    )
}

pub(super) fn write_reg_tuple(out: &mut String, regs: &[Reg]) {
    out.push('{');
    for (idx, reg) in regs.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        let _ = write!(out, "{reg}");
    }
    out.push('}');
}

pub(super) fn ptx_binop_suffix(op: BinOp, ty: PtxType) -> &'static str {
    match op {
        // Logical And/Or on predicate operands MUST use `.pred`. PTX
        // rejects `and.b32 %p, %p, %p` because the operand class does not
        // match the type suffix. Bitwise variants on b32 operands keep
        // `.b32`.
        BinOp::And | BinOp::Or if matches!(ty, PtxType::Bool) => "pred",
        BinOp::BitAnd
        | BinOp::BitOr
        | BinOp::BitXor
        | BinOp::Shl
        | BinOp::RotateLeft
        | BinOp::RotateRight
        | BinOp::And
        | BinOp::Or => "b32",
        BinOp::Shr => match ty {
            PtxType::I32 => "s32",
            _ => "u32",
        },
        BinOp::AbsDiff => "u32",
        _ => match ty {
            PtxType::F32 => "f32",
            PtxType::I32 => "s32",
            PtxType::Bool | PtxType::B16 | PtxType::U32 => "u32",
            PtxType::U64 => "u64",
        },
    }
}
