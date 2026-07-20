use std::fmt::Write as _;

use vyre_foundation::ir::{BinOp, DataType, SubgroupReduceOp, UnOp};

use super::format::ptx_binop_suffix;
use super::names::unop_name;
use super::BodyCtx;
use crate::reg::{PtxType, Reg};
use crate::EmitError;

impl BodyCtx<'_> {
    pub(super) fn canonicalize_f32(&mut self, value: Reg) -> Reg {
        if value.0 != PtxType::F32 {
            return value;
        }
        let bits = self.alloc(PtxType::U32);
        let abs_bits = self.alloc(PtxType::U32);
        let sign_bits = self.alloc(PtxType::U32);
        let subnormal_or_zero = self.alloc(PtxType::Bool);
        let nan = self.alloc(PtxType::Bool);
        let no_subnormal_bits = self.alloc(PtxType::U32);
        let canonical_bits = self.alloc(PtxType::U32);
        let out = self.alloc(PtxType::F32);
        let _ = writeln!(self.text, "    mov.b32    {bits}, {value};");
        let _ = writeln!(self.text, "    and.b32    {abs_bits}, {bits}, 0x7fffffff;");
        let _ = writeln!(self.text, "    and.b32    {sign_bits}, {bits}, 0x80000000;");
        let _ = writeln!(
            self.text,
            "    setp.lt.u32    {subnormal_or_zero}, {abs_bits}, 0x00800000;"
        );
        let _ = writeln!(
            self.text,
            "    setp.gt.u32    {nan}, {abs_bits}, 0x7f800000;"
        );
        let _ = writeln!(
            self.text,
            "    selp.u32    {no_subnormal_bits}, {sign_bits}, {bits}, {subnormal_or_zero};"
        );
        let _ = writeln!(
            self.text,
            "    selp.u32    {canonical_bits}, 0x7fc00000, {no_subnormal_bits}, {nan};"
        );
        let _ = writeln!(self.text, "    mov.b32    {out}, {canonical_bits};");
        out
    }

    pub(super) fn coerce_for_store(&mut self, value: Reg, elem_ty: PtxType) -> Reg {
        if value.0 != PtxType::Bool || elem_ty == PtxType::Bool {
            return value;
        }
        let out = self.alloc(PtxType::U32);
        let _ = writeln!(self.text, "    selp.u32    {out}, 1, 0, {value};");
        out
    }

    pub(super) fn pred_from_boolish(&mut self, value: Reg) -> Reg {
        if value.0 == PtxType::Bool {
            return value;
        }
        let pred = self.alloc(PtxType::Bool);
        let _ = writeln!(self.text, "    setp.ne.u32    {pred}, {value}, 0;");
        pred
    }

    pub(super) fn emit_binop(
        &mut self,
        op: BinOp,
        left: Reg,
        right: Reg,
    ) -> Result<(Reg, PtxType), EmitError> {
        let ty = left.0;
        if ty == PtxType::Bool && matches!(op, BinOp::Eq | BinOp::Ne) {
            let out = self.alloc(PtxType::Bool);
            let xor = self.alloc(PtxType::Bool);
            let _ = writeln!(self.text, "    xor.pred    {xor}, {left}, {right};");
            if matches!(op, BinOp::Eq) {
                let _ = writeln!(self.text, "    not.pred    {out}, {xor};");
            } else {
                let _ = writeln!(self.text, "    mov.pred    {out}, {xor};");
            }
            return Ok((out, PtxType::Bool));
        }
        if matches!(
            op,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
        ) {
            let out = self.alloc(PtxType::Bool);
            let cmp = match op {
                BinOp::Eq => "eq",
                BinOp::Ne if ty == PtxType::F32 => "neu",
                BinOp::Ne => "ne",
                BinOp::Lt => "lt",
                BinOp::Le => "le",
                BinOp::Gt => "gt",
                BinOp::Ge => "ge",
                other => {
                    return Err(EmitError::InvalidDescriptor(format!(
                        "comparison lowering received non-comparison operator {other:?}. \
                         Fix: route arithmetic operators through the arithmetic PTX lowering path."
                    )));
                }
            };
            let suffix = if ty == PtxType::F32 {
                "f32"
            } else if ty == PtxType::I32 {
                "s32"
            } else {
                "u32"
            };
            let _ = writeln!(
                self.text,
                "    setp.{cmp}.{suffix}    {out}, {left}, {right};"
            );
            return Ok((out, PtxType::Bool));
        }
        match op {
            BinOp::AbsDiff if ty == PtxType::U32 || ty == PtxType::Bool => {
                let left_ge_right = self.alloc(PtxType::Bool);
                let hi = self.alloc(PtxType::U32);
                let lo = self.alloc(PtxType::U32);
                let out = self.alloc(PtxType::U32);
                let _ = writeln!(
                    self.text,
                    "    setp.ge.u32    {left_ge_right}, {left}, {right};"
                );
                let _ = writeln!(
                    self.text,
                    "    selp.u32    {hi}, {left}, {right}, {left_ge_right};"
                );
                let _ = writeln!(
                    self.text,
                    "    selp.u32    {lo}, {right}, {left}, {left_ge_right};"
                );
                let _ = writeln!(self.text, "    sub.u32    {out}, {hi}, {lo};");
                Ok((out, PtxType::U32))
            }
            BinOp::RotateLeft | BinOp::RotateRight
                if ty == PtxType::U32 || ty == PtxType::I32 || ty == PtxType::Bool =>
            {
                let out = self.alloc(ty);
                let direction = if matches!(op, BinOp::RotateLeft) {
                    "l"
                } else {
                    "r"
                };
                let _ = writeln!(
                    self.text,
                    "    shf.{direction}.wrap.b32    {out}, {left}, {left}, {right};"
                );
                Ok((out, ty))
            }
            BinOp::Shl | BinOp::Shr if ty == PtxType::U32 || ty == PtxType::I32 => {
                let out = self.alloc(ty);
                let masked_shift = self.alloc(PtxType::U32);
                let mnemonic = if matches!(op, BinOp::Shl) {
                    "shl"
                } else {
                    "shr"
                };
                let suffix = ptx_binop_suffix(op, ty);
                let _ = writeln!(self.text, "    and.b32    {masked_shift}, {right}, 31;");
                let _ = writeln!(
                    self.text,
                    "    {mnemonic}.{suffix}    {out}, {left}, {masked_shift};"
                );
                Ok((out, ty))
            }
            BinOp::Div if ty == PtxType::U32 || ty == PtxType::Bool => {
                let out = self.emit_total_u32_div(left, right);
                Ok((out, PtxType::U32))
            }
            BinOp::Mod if ty == PtxType::U32 || ty == PtxType::Bool => {
                let out = self.emit_total_u32_mod(left, right);
                Ok((out, PtxType::U32))
            }
            BinOp::Div if ty == PtxType::I32 => {
                let out = self.emit_total_i32_div(left, right);
                Ok((out, PtxType::I32))
            }
            BinOp::Div if ty == PtxType::F32 => {
                let out = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    div.rn.f32    {out}, {left}, {right};");
                Ok((self.canonicalize_f32(out), PtxType::F32))
            }
            BinOp::Mod if ty == PtxType::I32 => {
                let out = self.emit_total_i32_mod(left, right);
                Ok((out, PtxType::I32))
            }
            BinOp::SaturatingAdd if ty == PtxType::U32 || ty == PtxType::Bool => {
                let sum = self.alloc(PtxType::U32);
                let overflow = self.alloc(PtxType::Bool);
                let out = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    add.u32    {sum}, {left}, {right};");
                let _ = writeln!(self.text, "    setp.lt.u32    {overflow}, {sum}, {left};");
                let _ = writeln!(
                    self.text,
                    "    selp.u32    {out}, 0xffffffff, {sum}, {overflow};"
                );
                Ok((out, PtxType::U32))
            }
            BinOp::SaturatingSub if ty == PtxType::U32 || ty == PtxType::Bool => {
                let underflow = self.alloc(PtxType::Bool);
                let diff = self.alloc(PtxType::U32);
                let out = self.alloc(PtxType::U32);
                let _ = writeln!(
                    self.text,
                    "    setp.lt.u32    {underflow}, {left}, {right};"
                );
                let _ = writeln!(self.text, "    sub.u32    {diff}, {left}, {right};");
                let _ = writeln!(self.text, "    selp.u32    {out}, 0, {diff}, {underflow};");
                Ok((out, PtxType::U32))
            }
            BinOp::SaturatingMul if ty == PtxType::U32 || ty == PtxType::Bool => {
                // Full 64-bit product via the native widening multiply, clamped to
                // u32::MAX when it overflows 32 bits — i.e. when the product's high
                // word is non-zero. Byte-for-byte `u32::saturating_mul` and the
                // oracle's `select(b != 0 && a > MAX/b, MAX, a*b)` contract, without
                // an emulated division.
                let prod = self.alloc(PtxType::U64);
                let prod_hi64 = self.alloc(PtxType::U64);
                let lo = self.alloc(PtxType::U32);
                let hi = self.alloc(PtxType::U32);
                let overflow = self.alloc(PtxType::Bool);
                let out = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    mul.wide.u32    {prod}, {left}, {right};");
                let _ = writeln!(self.text, "    cvt.u32.u64    {lo}, {prod};");
                let _ = writeln!(self.text, "    shr.u64    {prod_hi64}, {prod}, 32;");
                let _ = writeln!(self.text, "    cvt.u32.u64    {hi}, {prod_hi64};");
                let _ = writeln!(self.text, "    setp.ne.u32    {overflow}, {hi}, 0;");
                let _ = writeln!(
                    self.text,
                    "    selp.u32    {out}, 0xffffffff, {lo}, {overflow};"
                );
                Ok((out, PtxType::U32))
            }
            _ => {
                let out_ty = if matches!(op, BinOp::And | BinOp::Or) && ty == PtxType::Bool {
                    PtxType::Bool
                } else {
                    ty
                };
                let out = self.alloc(out_ty);
                let mnemonic = match op {
                    BinOp::Add | BinOp::WrappingAdd => "add",
                    BinOp::Sub | BinOp::WrappingSub => "sub",
                    BinOp::Mul => {
                        if ty == PtxType::F32 {
                            "mul"
                        } else {
                            "mul.lo"
                        }
                    }
                    BinOp::MulHigh => "mul.hi",
                    BinOp::BitAnd | BinOp::And => "and",
                    BinOp::BitOr | BinOp::Or => "or",
                    BinOp::BitXor => "xor",
                    BinOp::Shl => "shl",
                    BinOp::Shr => "shr",
                    BinOp::Min => "min",
                    BinOp::Max => "max",
                    other => {
                        return Err(EmitError::PtxConstructionFailed(format!(
                            "BinOp `{other:?}` has no PTX lowering. Fix: add descriptor PTX emission before enabling this op on CUDA."
                        )));
                    }
                };
                let suffix = ptx_binop_suffix(op, ty);
                let _ = writeln!(
                    self.text,
                    "    {mnemonic}.{suffix}    {out}, {left}, {right};"
                );
                Ok((self.canonicalize_f32(out), out_ty))
            }
        }
    }

    pub(super) fn emit_small_u32_const_mul(&mut self, value: Reg, constant: u32) -> Option<Reg> {
        if !matches!(value.0, PtxType::U32 | PtxType::Bool) {
            return None;
        }
        if constant == 0 {
            let out = self.alloc(PtxType::U32);
            let _ = writeln!(self.text, "    mov.u32    {out}, 0;");
            return Some(out);
        }
        if constant == 1 {
            return Some(value);
        }
        if constant.is_power_of_two() {
            let out = self.alloc(PtxType::U32);
            let shift = constant.trailing_zeros();
            let _ = writeln!(self.text, "    shl.b32    {out}, {value}, {shift};");
            return Some(out);
        }
        if constant.count_ones() > 4 {
            return None;
        }
        let mut acc = None;
        for shift in 0..u32::BITS {
            if (constant & (1u32 << shift)) == 0 {
                continue;
            }
            let term = if shift == 0 {
                value
            } else {
                let shifted = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    shl.b32    {shifted}, {value}, {shift};");
                shifted
            };
            acc = Some(match acc {
                Some(prev) => {
                    let out = self.alloc(PtxType::U32);
                    let _ = writeln!(self.text, "    add.u32    {out}, {prev}, {term};");
                    out
                }
                None => term,
            });
        }
        acc
    }

    pub(super) fn emit_fast_u32_const_div(&mut self, value: Reg, divisor: u32) -> Option<Reg> {
        if !matches!(value.0, PtxType::U32 | PtxType::Bool) || divisor == 0 {
            return None;
        }
        if divisor == 1 {
            return Some(value);
        }
        if divisor.is_power_of_two() {
            let out = self.alloc(PtxType::U32);
            let shift = divisor.trailing_zeros();
            let _ = writeln!(self.text, "    shr.u32    {out}, {value}, {shift};");
            return Some(out);
        }
        if divisor == 3 {
            let magic = self.alloc(PtxType::U32);
            let high = self.alloc(PtxType::U32);
            let out = self.alloc(PtxType::U32);
            let _ = writeln!(self.text, "    mov.u32    {magic}, 0xaaaaaaab;");
            let _ = writeln!(self.text, "    mul.hi.u32    {high}, {value}, {magic};");
            let _ = writeln!(self.text, "    shr.u32    {out}, {high}, 1;");
            return Some(out);
        }
        None
    }

    pub(super) fn emit_fast_u32_const_mod(&mut self, value: Reg, divisor: u32) -> Option<Reg> {
        if !matches!(value.0, PtxType::U32 | PtxType::Bool) || divisor == 0 {
            return None;
        }
        let out = self.alloc(PtxType::U32);
        if divisor == 1 {
            let _ = writeln!(self.text, "    mov.u32    {out}, 0;");
            return Some(out);
        }
        if divisor.is_power_of_two() {
            let mask = divisor - 1;
            let _ = writeln!(self.text, "    and.b32    {out}, {value}, {mask};");
            return Some(out);
        }
        None
    }

    fn emit_total_u32_div(&mut self, left: Reg, right: Reg) -> Reg {
        let out = self.alloc(PtxType::U32);
        let pred = self.alloc(PtxType::Bool);
        let done = self.alloc_label("u32_div_done");
        let _ = writeln!(self.text, "    mov.u32    {out}, 0xffffffff;");
        let _ = writeln!(self.text, "    setp.eq.u32    {pred}, {right}, 0;");
        let _ = writeln!(self.text, "    @{pred} bra {done};");
        let _ = writeln!(self.text, "    div.u32    {out}, {left}, {right};");
        let _ = writeln!(self.text, "{done}:");
        out
    }

    fn emit_total_u32_mod(&mut self, left: Reg, right: Reg) -> Reg {
        let out = self.alloc(PtxType::U32);
        let pred = self.alloc(PtxType::Bool);
        let done = self.alloc_label("u32_mod_done");
        let _ = writeln!(self.text, "    mov.u32    {out}, 0;");
        let _ = writeln!(self.text, "    setp.eq.u32    {pred}, {right}, 0;");
        let _ = writeln!(self.text, "    @{pred} bra {done};");
        let _ = writeln!(self.text, "    rem.u32    {out}, {left}, {right};");
        let _ = writeln!(self.text, "{done}:");
        out
    }

    fn emit_total_i32_div(&mut self, left: Reg, right: Reg) -> Reg {
        let out = self.alloc(PtxType::I32);
        let zero = self.alloc(PtxType::Bool);
        let min = self.alloc(PtxType::Bool);
        let neg_one = self.alloc(PtxType::Bool);
        let overflow = self.alloc(PtxType::Bool);
        let overflow_label = self.alloc_label("i32_div_min_overflow");
        let done = self.alloc_label("i32_div_done");
        let _ = writeln!(self.text, "    mov.s32    {out}, 0;");
        let _ = writeln!(self.text, "    setp.eq.s32    {zero}, {right}, 0;");
        let _ = writeln!(self.text, "    @{zero} bra {done};");
        let _ = writeln!(self.text, "    setp.eq.u32    {min}, {left}, 0x80000000;");
        let _ = writeln!(
            self.text,
            "    setp.eq.u32    {neg_one}, {right}, 0xffffffff;"
        );
        let _ = writeln!(self.text, "    and.pred    {overflow}, {min}, {neg_one};");
        let _ = writeln!(self.text, "    @{overflow} bra {overflow_label};");
        let _ = writeln!(self.text, "    div.s32    {out}, {left}, {right};");
        let _ = writeln!(self.text, "    bra {done};");
        let _ = writeln!(self.text, "{overflow_label}:");
        let _ = writeln!(self.text, "    mov.u32    {out}, 0x80000000;");
        let _ = writeln!(self.text, "{done}:");
        out
    }

    fn emit_total_i32_mod(&mut self, left: Reg, right: Reg) -> Reg {
        let out = self.alloc(PtxType::I32);
        let zero = self.alloc(PtxType::Bool);
        let min = self.alloc(PtxType::Bool);
        let neg_one = self.alloc(PtxType::Bool);
        let overflow = self.alloc(PtxType::Bool);
        let done = self.alloc_label("i32_mod_done");
        let _ = writeln!(self.text, "    mov.s32    {out}, 0;");
        let _ = writeln!(self.text, "    setp.eq.s32    {zero}, {right}, 0;");
        let _ = writeln!(self.text, "    @{zero} bra {done};");
        let _ = writeln!(self.text, "    setp.eq.u32    {min}, {left}, 0x80000000;");
        let _ = writeln!(
            self.text,
            "    setp.eq.u32    {neg_one}, {right}, 0xffffffff;"
        );
        let _ = writeln!(self.text, "    and.pred    {overflow}, {min}, {neg_one};");
        let _ = writeln!(self.text, "    @{overflow} bra {done};");
        let _ = writeln!(self.text, "    rem.s32    {out}, {left}, {right};");
        let _ = writeln!(self.text, "{done}:");
        out
    }

    pub(super) fn emit_unop(&mut self, op: &UnOp, operand: Reg) -> Result<Reg, EmitError> {
        let out = match (op, operand.0) {
            (UnOp::Negate, PtxType::F32) => {
                let out = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    neg.f32    {out}, {operand};");
                out
            }
            (UnOp::Negate, _) => {
                let out = self.alloc(PtxType::I32);
                let _ = writeln!(self.text, "    neg.s32    {out}, {operand};");
                out
            }
            (UnOp::BitNot, _) => {
                let out = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    not.b32    {out}, {operand};");
                out
            }
            (UnOp::LogicalNot, _) => {
                let out = self.alloc(PtxType::Bool);
                if operand.0 == PtxType::Bool {
                    let _ = writeln!(self.text, "    not.pred    {out}, {operand};");
                } else {
                    let _ = writeln!(self.text, "    setp.eq.u32    {out}, {operand}, 0;");
                }
                out
            }
            (UnOp::Abs, PtxType::F32) => {
                let out = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    abs.f32    {out}, {operand};");
                out
            }
            (UnOp::Abs, _) => {
                let out = self.alloc(PtxType::I32);
                let _ = writeln!(self.text, "    abs.s32    {out}, {operand};");
                out
            }
            (UnOp::Sqrt, PtxType::F32) => {
                let out = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    sqrt.rn.f32    {out}, {operand};");
                out
            }
            (UnOp::InverseSqrt, PtxType::F32) => {
                let out = self.alloc(PtxType::F32);
                if self.options.ulp_budget.is_some_and(|budget| budget > 0) {
                    let _ = writeln!(self.text, "    rsqrt.approx.f32    {out}, {operand};");
                } else {
                    let _ = writeln!(self.text, "    sqrt.rn.f32    {out}, {operand};");
                    let _ = writeln!(self.text, "    rcp.rn.f32    {out}, {out};");
                }
                out
            }
            (UnOp::Reciprocal, PtxType::F32) => {
                let out = self.alloc(PtxType::F32);
                if self.options.ulp_budget.is_some_and(|budget| budget > 0) {
                    let _ = writeln!(self.text, "    rcp.approx.f32    {out}, {operand};");
                } else {
                    let _ = writeln!(self.text, "    rcp.rn.f32    {out}, {operand};");
                }
                out
            }
            (UnOp::Tanh, PtxType::F32) => {
                if !self.options.ulp_budget.is_some_and(|budget| budget > 0) {
                    return Err(EmitError::PtxConstructionFailed(
                        "CUDA PTX `tanh` lowering requires approximate transcendental instructions, but ulp_budget is not positive. Fix: set an explicit ULP budget for this dispatch or route to strict lowering.".into(),
                    ));
                }
                let out = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    tanh.approx.f32    {out}, {operand};");
                out
            }
            (UnOp::Exp, PtxType::F32)
            | (UnOp::Log, PtxType::F32)
            | (UnOp::Exp2, PtxType::F32)
            | (UnOp::Log2, PtxType::F32)
            | (UnOp::Sin, PtxType::F32)
            | (UnOp::Cos, PtxType::F32) => {
                if !self.options.ulp_budget.is_some_and(|budget| budget > 0) {
                    return Err(EmitError::PtxConstructionFailed(format!(
                        "CUDA PTX `{op:?}` lowering requires approximate transcendental instructions, but ulp_budget is not positive. Fix: set an explicit ULP budget for this dispatch or route to strict lowering."
                    )));
                }
                let out = self.alloc(PtxType::F32);
                match op {
                    UnOp::Exp => {
                        let tmp = self.alloc(PtxType::F32);
                        let _ = writeln!(self.text, "    mul.f32    {tmp}, {operand}, 0f3FB8AA3B;");
                        let _ = writeln!(self.text, "    ex2.approx.f32    {out}, {tmp};");
                    }
                    UnOp::Log => {
                        let tmp = self.alloc(PtxType::F32);
                        let _ = writeln!(self.text, "    lg2.approx.f32    {tmp}, {operand};");
                        let _ = writeln!(self.text, "    mul.f32    {out}, {tmp}, 0f3F317218;");
                    }
                    UnOp::Exp2 => {
                        let _ = writeln!(self.text, "    ex2.approx.f32    {out}, {operand};");
                    }
                    UnOp::Log2 => {
                        let _ = writeln!(self.text, "    lg2.approx.f32    {out}, {operand};");
                    }
                    UnOp::Sin => {
                        let _ = writeln!(self.text, "    sin.approx.f32    {out}, {operand};");
                    }
                    UnOp::Cos => {
                        let _ = writeln!(self.text, "    cos.approx.f32    {out}, {operand};");
                    }
                    _ => {}
                }
                out
            }
            (UnOp::Floor, PtxType::F32) => {
                let out = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    cvt.rmi.f32.f32    {out}, {operand};");
                out
            }
            (UnOp::Ceil, PtxType::F32) => {
                let out = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    cvt.rpi.f32.f32    {out}, {operand};");
                out
            }
            (UnOp::Round, PtxType::F32) => {
                let out = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    cvt.rni.f32.f32    {out}, {operand};");
                out
            }
            (UnOp::Trunc, PtxType::F32) => {
                let out = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    cvt.rzi.f32.f32    {out}, {operand};");
                out
            }
            (UnOp::Popcount, _) => {
                let out = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    popc.b32    {out}, {operand};");
                out
            }
            (UnOp::Clz, _) => {
                let out = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    clz.b32    {out}, {operand};");
                out
            }
            (UnOp::Ctz, _) => {
                let reversed = self.alloc(PtxType::U32);
                let out = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    brev.b32    {reversed}, {operand};");
                let _ = writeln!(self.text, "    clz.b32    {out}, {reversed};");
                out
            }
            (UnOp::ReverseBits, _) => {
                let out = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    brev.b32    {out}, {operand};");
                out
            }
            (UnOp::IsNan, PtxType::F32) => {
                let out = self.alloc(PtxType::Bool);
                let _ = writeln!(
                    self.text,
                    "    setp.nan.f32    {out}, {operand}, {operand};"
                );
                out
            }
            (UnOp::IsInf, PtxType::F32) => {
                let bits = self.alloc(PtxType::U32);
                let abs = self.alloc(PtxType::U32);
                let out = self.alloc(PtxType::Bool);
                let _ = writeln!(self.text, "    mov.b32    {bits}, {operand};");
                let _ = writeln!(self.text, "    and.b32    {abs}, {bits}, 0x7fffffff;");
                let _ = writeln!(self.text, "    setp.eq.u32    {out}, {abs}, 0x7f800000;");
                out
            }
            (UnOp::IsFinite, PtxType::F32) => {
                let bits = self.alloc(PtxType::U32);
                let abs = self.alloc(PtxType::U32);
                let out = self.alloc(PtxType::Bool);
                let _ = writeln!(self.text, "    mov.b32    {bits}, {operand};");
                let _ = writeln!(self.text, "    and.b32    {abs}, {bits}, 0x7fffffff;");
                let _ = writeln!(self.text, "    setp.lt.u32    {out}, {abs}, 0x7f800000;");
                out
            }
            other => {
                return Err(EmitError::PtxConstructionFailed(format!(
                    "UnOp `{}` on {:?} has no PTX lowering. Fix: add descriptor PTX emission before enabling this op on CUDA.",
                    unop_name(other.0),
                    other.1
                )));
            }
        };
        Ok(self.canonicalize_f32(out))
    }

    pub(super) fn emit_cast(&mut self, src: Reg, target: &DataType) -> Result<Reg, EmitError> {
        let dst_ty = PtxType::from_dtype(target)?;
        // A float source has no defined narrowing integer conversion: it converts
        // only to u32/i32 (saturating `cvt.rzi`, below), bool (truthy), or f32.
        // `from_dtype` collapses U8/U16->U32 and I8/I16->I32, so WITHOUT this guard
        // an f32->u8 would silently emit a non-narrowing `cvt.rzi.u32.f32` (a full
        // u32-range value claimed as a u8). The foundation cast table
        // (`validate::cast::cast_is_valid`) rejects these casts, but the
        // no-validation emit path can reach here. Fail closed (Law 10), matching
        // the naga emitter and the `Bytes` arm in `from_dtype`. (f32->u64/i64 maps
        // to PtxType::U64 != F32, so it already fails closed via the unmatched `_`
        // arm below; this closes the narrow-int holes that `from_dtype` masks.)
        if src.0 == PtxType::F32
            && matches!(
                target,
                DataType::U8 | DataType::U16 | DataType::I8 | DataType::I16
            )
        {
            return Err(EmitError::PtxConstructionFailed(format!(
                "cast from f32 to `{target:?}` has no defined conversion: a float \
                 source converts only to u32/i32 (saturating), bool (truthy), or f32. \
                 Fix: cast the f32 to u32 or i32 first, then narrow the integer."
            )));
        }
        // Narrowing to a sub-word integer (u8/u16/i8/i16). `from_dtype` collapses
        // these to U32/I32, so the `src.0 == dst_ty` identity check below would
        // treat `u32 -> u8` as a no-op and KEEP the high bits — a silent
        // non-narrowing that diverges from Rust `as`, the V035 contract, and the
        // reference oracle. PTX has no sub-word register, but it has the canonical
        // narrowing converts: `cvt.u32.u8` zero-extends the low byte (== `& 0xFF`)
        // and `cvt.s32.s8` sign-extends it (`200 as i8 == -56`). Reduce the source
        // to its low 32-bit word first, then emit the narrowing convert.
        let narrow = match target {
            DataType::U8 => Some(("u32", "u8")),
            DataType::U16 => Some(("u32", "u16")),
            DataType::I8 => Some(("s32", "s8")),
            DataType::I16 => Some(("s32", "s16")),
            _ => None,
        };
        if let Some((dst_t, src_t)) = narrow {
            let base = match src.0 {
                PtxType::U32 | PtxType::I32 => src,
                PtxType::Bool => {
                    let word = self.alloc(PtxType::U32);
                    let _ = writeln!(self.text, "    selp.u32    {word}, 1, 0, {src};");
                    word
                }
                PtxType::U64 => {
                    let word = self.alloc(PtxType::U32);
                    let _ = writeln!(self.text, "    cvt.u32.u64    {word}, {src};");
                    word
                }
                PtxType::F32 | PtxType::B16 => {
                    // F32 is already rejected by the guard above; B16 (packed
                    // f16/bf16) is not an integer-like narrowing source. Fail
                    // closed rather than reinterpret float bits as an integer.
                    return Err(EmitError::PtxConstructionFailed(format!(
                        "cast from {:?} to `{target:?}` has no defined integer \
                         narrowing: only an integer-like scalar source narrows. \
                         Fix: cast to u32 or i32 first, then narrow.",
                        src.0
                    )));
                }
            };
            let dst = self.alloc(dst_ty);
            let _ = writeln!(self.text, "    cvt.{dst_t}.{src_t}    {dst}, {base};");
            return Ok(dst);
        }
        if src.0 == dst_ty {
            return Ok(src);
        }
        let dst = self.alloc(dst_ty);
        match (src.0, dst_ty) {
            (PtxType::U32, PtxType::F32) => {
                let _ = writeln!(self.text, "    cvt.rn.f32.u32    {dst}, {src};");
            }
            (PtxType::I32, PtxType::F32) => {
                let _ = writeln!(self.text, "    cvt.rn.f32.s32    {dst}, {src};");
            }
            (PtxType::Bool, PtxType::F32) => {
                let word = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    selp.u32    {word}, 1, 0, {src};");
                let _ = writeln!(self.text, "    cvt.rn.f32.u32    {dst}, {word};");
            }
            (PtxType::F32, PtxType::U32) => {
                let _ = writeln!(self.text, "    cvt.rzi.u32.f32    {dst}, {src};");
            }
            (PtxType::F32, PtxType::I32) => {
                let _ = writeln!(self.text, "    cvt.rzi.s32.f32    {dst}, {src};");
            }
            (PtxType::Bool, PtxType::U32) => {
                let _ = writeln!(self.text, "    selp.u32    {dst}, 1, 0, {src};");
            }
            (PtxType::Bool, PtxType::I32) => {
                let _ = writeln!(self.text, "    selp.u32    {dst}, 1, 0, {src};");
            }
            (PtxType::U32 | PtxType::I32, PtxType::Bool) => {
                let _ = writeln!(self.text, "    setp.ne.u32    {dst}, {src}, 0;");
            }
            (PtxType::F32, PtxType::Bool) => {
                let _ = writeln!(self.text, "    setp.neu.f32    {dst}, {src}, 0f00000000;");
            }
            (PtxType::U32, PtxType::I32) | (PtxType::I32, PtxType::U32) => {
                let _ = writeln!(self.text, "    mov.b32    {dst}, {src};");
            }
            (PtxType::U32, PtxType::U64) => {
                // Zero-extend 32→64.
                let _ = writeln!(self.text, "    cvt.u64.u32    {dst}, {src};");
            }
            (PtxType::I32, PtxType::U64) => {
                // Sign-extend 32→64; the 64-bit two's-complement bit pattern is
                // written into the .u64 (`%rd`) register.
                let _ = writeln!(self.text, "    cvt.s64.s32    {dst}, {src};");
            }
            (PtxType::U64, PtxType::U32) => {
                // Explicit narrowing: keep the low 32 bits.
                let _ = writeln!(self.text, "    cvt.u32.u64    {dst}, {src};");
            }
            (PtxType::U64, PtxType::I32) => {
                // Narrowing to a signed 32-bit value: keep the low 32 bits; the
                // bit pattern of the low word IS the i32 (matches naga's low-word
                // bitcast and the reference's low-word narrowing). NEVER fail
                // closed here — wgpu/naga supports u64->i32, so CUDA must too.
                let _ = writeln!(self.text, "    cvt.u32.u64    {dst}, {src};");
            }
            (PtxType::U64, PtxType::Bool) => {
                // Truthiness over the FULL 64 bits (not just the low word):
                // matches the reference `value != 0` and naga's (low | high) != 0.
                let _ = writeln!(self.text, "    setp.ne.u64    {dst}, {src}, 0;");
            }
            (PtxType::U64, PtxType::F32) => {
                // Full 64-bit → F32 with round-to-nearest. NEVER narrow to u32
                // first: that silently discards the high 32 bits.
                let _ = writeln!(self.text, "    cvt.rn.f32.u64    {dst}, {src};");
            }
            _ => {
                return Err(EmitError::PtxConstructionFailed(format!(
                    "unsupported PTX cast from {:?} to {:?}. Fix: validate casts before CUDA emission.",
                    src.0, dst_ty
                )));
            }
        }
        Ok(dst)
    }

    pub(super) fn subgroup_lane_mask(&self) -> u32 {
        self.options.subgroup_size.saturating_sub(1)
    }

    pub(super) fn emit_subgroup_reduce(
        &mut self,
        op: SubgroupReduceOp,
        value: Reg,
    ) -> Result<Reg, EmitError> {
        if value.0 == PtxType::F32 {
            return self.emit_f32_subgroup_reduce(op, value);
        }
        // redux.sync handles add/min/max with the signed/unsigned type and
        // and/or/xor as bitwise `.b32`. It has no integer product reduction.
        let (op_str, type_str) = match op {
            SubgroupReduceOp::Add => ("add", value.0.ptx_type_str()),
            SubgroupReduceOp::Min => ("min", value.0.ptx_type_str()),
            SubgroupReduceOp::Max => ("max", value.0.ptx_type_str()),
            SubgroupReduceOp::And => ("and", "b32"),
            SubgroupReduceOp::Or => ("or", "b32"),
            SubgroupReduceOp::Xor => ("xor", "b32"),
            SubgroupReduceOp::Mul => {
                // No `redux.sync` integer product; reduce with the same
                // shfl.idx XOR butterfly the f32 path uses. Integers shuffle
                // directly (no f32<->b32 bitcast). `mul.lo` keeps the low 32
                // bits (matching the reference oracle's `wrapping_mul`).
                let combine = match value.0 {
                    PtxType::U32 => "mul.lo.u32",
                    PtxType::I32 => "mul.lo.s32",
                    other_ty => {
                        return Err(EmitError::InvalidDescriptor(format!(
                            "PTX subgroup product reduction needs a 32-bit integer operand, got {other_ty:?}. Fix: use u32/i32 (or an f32 operand)."
                        )))
                    }
                };
                return Ok(self.emit_subgroup_xor_butterfly(value, value.0, combine));
            }
            other => {
                return Err(EmitError::InvalidDescriptor(format!(
                    "PTX subgroup reduction operator {other:?} is not supported. Fix: extend emit_subgroup_reduce."
                )))
            }
        };
        let result = self.alloc(value.0);
        let mask = self.alloc(PtxType::U32);
        let _ = writeln!(self.text, "    activemask.b32    {mask};");
        let _ = writeln!(
            self.text,
            "    redux.sync.{op_str}.{type_str}    {result}, {value}, {mask};"
        );
        Ok(result)
    }

    fn emit_f32_subgroup_reduce(
        &mut self,
        op: SubgroupReduceOp,
        value: Reg,
    ) -> Result<Reg, EmitError> {
        // f32 has no redux.sync; reduce with a shfl XOR butterfly whose combine
        // instruction is the reduction operator. Bitwise ops are undefined on
        // floats and fail closed.
        let combine = match op {
            SubgroupReduceOp::Add => "add.f32",
            SubgroupReduceOp::Mul => "mul.f32",
            SubgroupReduceOp::Min => "min.f32",
            SubgroupReduceOp::Max => "max.f32",
            SubgroupReduceOp::And | SubgroupReduceOp::Or | SubgroupReduceOp::Xor => {
                return Err(EmitError::InvalidDescriptor(
                    "subgroup bitwise reduction (and/or/xor) is undefined on f32. Fix: use an integer operand.".into(),
                ))
            }
            other => {
                return Err(EmitError::InvalidDescriptor(format!(
                    "PTX f32 subgroup reduction operator {other:?} is not supported. Fix: extend emit_f32_subgroup_reduce."
                )))
            }
        };
        Ok(self.emit_subgroup_xor_butterfly(value, PtxType::F32, combine))
    }

    /// XOR (butterfly) all-reduce shared by the f32 and integer-product paths.
    ///
    /// Every lane exchanges with `lane ^ offset` and applies `combine`, so after
    /// log2(width) steps EVERY lane holds the full reduction — matching the
    /// all-lane-broadcast contract of WGSL `subgroupAdd` / `redux.sync` / the
    /// reference oracle. (A `shfl.down` tree feeds only lane 0, which violates
    /// that contract for lanes 1..; this is verified all-lane on a live sm_120
    /// GPU by `vyre-driver-cuda`'s `subgroup_reduce_gpu_parity`.)
    ///
    /// The exchange uses `.idx` mode with an explicit `laneid ^ offset` source
    /// (the same instruction the verified `subgroup_shuffle` path uses), with
    /// `c = subgroup_size - 1` (the standard full-warp clamp, segmask=0). The
    /// XOR partner is always in range for a power-of-two width, so no
    /// predication is needed. `combine` is the full typed PTX op
    /// (`add.f32` / `max.f32` / `mul.lo.u32` / ...).
    fn emit_subgroup_xor_butterfly(&mut self, value: Reg, acc_ty: PtxType, combine: &str) -> Reg {
        let is_f32 = acc_ty == PtxType::F32;
        let acc = self.alloc(acc_ty);
        let mask = self.alloc(PtxType::U32);
        let lane = self.alloc(PtxType::U32);
        let idx_clamp = self.subgroup_lane_mask();
        // `mov.b32` is a width-preserving bit copy valid for f32 and integer
        // 32-bit registers alike.
        let _ = writeln!(self.text, "    mov.b32    {acc}, {value};");
        let _ = writeln!(self.text, "    activemask.b32    {mask};");
        let _ = writeln!(self.text, "    mov.u32    {lane}, %laneid;");

        let mut offset = self.options.subgroup_size / 2;
        while offset > 0 {
            let src_lane = self.alloc(PtxType::U32);
            let _ = writeln!(self.text, "    xor.b32    {src_lane}, {lane}, {offset};");
            if is_f32 {
                // Cross-lane shfl operates on `.b32`; bitcast f32<->b32 around it.
                let bits = self.alloc(PtxType::U32);
                let shuffled_bits = self.alloc(PtxType::U32);
                let shuffled = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    mov.b32    {bits}, {acc};");
                let _ = writeln!(
                    self.text,
                    "    shfl.sync.idx.b32    {shuffled_bits}, {bits}, {src_lane}, 0x{idx_clamp:x}, {mask};"
                );
                let _ = writeln!(self.text, "    mov.b32    {shuffled}, {shuffled_bits};");
                let _ = writeln!(self.text, "    {combine}    {acc}, {acc}, {shuffled};");
            } else {
                // Integer registers shuffle directly.
                let shuffled = self.alloc(acc_ty);
                let _ = writeln!(
                    self.text,
                    "    shfl.sync.idx.b32    {shuffled}, {acc}, {src_lane}, 0x{idx_clamp:x}, {mask};"
                );
                let _ = writeln!(self.text, "    {combine}    {acc}, {acc}, {shuffled};");
            }
            offset /= 2;
        }
        acc
    }
}
