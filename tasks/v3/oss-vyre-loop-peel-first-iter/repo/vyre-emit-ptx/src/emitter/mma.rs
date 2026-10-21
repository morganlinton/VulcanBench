use std::fmt::Write as _;

use vyre_lower::{KernelOp, MatrixMmaElement, MatrixMmaLayout, MatrixMmaShape};

use super::BodyCtx;
use crate::reg::{PtxType, Reg};
use crate::EmitError;

impl BodyCtx<'_> {
    pub(super) fn emit_matrix_mma(
        &mut self,
        op: &KernelOp,
        shape: MatrixMmaShape,
        a_layout: MatrixMmaLayout,
        b_layout: MatrixMmaLayout,
        a_type: MatrixMmaElement,
        b_type: MatrixMmaElement,
        accum_type: MatrixMmaElement,
    ) -> Result<[Reg; 4], EmitError> {
        if op.operands.len() < 10 {
            return Err(EmitError::InvalidDescriptor(format!(
                "MatrixMma requires 10 operands but got {}. Fix: pass four A fragment regs, two B fragment regs, and four accumulator regs.",
                op.operands.len()
            )));
        }
        if shape != MatrixMmaShape::M16N8K16
            || a_layout != MatrixMmaLayout::RowMajor
            || b_layout != MatrixMmaLayout::ColMajor
            || a_type != MatrixMmaElement::F16
            || b_type != MatrixMmaElement::F16
            || accum_type != MatrixMmaElement::F32
        {
            return Err(EmitError::UnsupportedOp(KernelOp {
                kind: op.kind.clone(),
                operands: op.operands.clone(),
                result: op.result,
            }));
        }
        if !self.options.target.supports_wmma_f16() {
            return Err(EmitError::UnsupportedOp(KernelOp {
                kind: op.kind.clone(),
                operands: op.operands.clone(),
                result: op.result,
            }));
        }

        let a = [
            self.lookup_operand(op.operands[0])?,
            self.lookup_operand(op.operands[1])?,
            self.lookup_operand(op.operands[2])?,
            self.lookup_operand(op.operands[3])?,
        ];
        let b = [
            self.lookup_operand(op.operands[4])?,
            self.lookup_operand(op.operands[5])?,
        ];
        let c = [
            self.lookup_operand(op.operands[6])?,
            self.lookup_operand(op.operands[7])?,
            self.lookup_operand(op.operands[8])?,
            self.lookup_operand(op.operands[9])?,
        ];
        for reg in a.iter().chain(b.iter()) {
            if reg.0 != PtxType::U32 {
                return Err(EmitError::InvalidDescriptor(format!(
                    "MatrixMma f16 fragments must be packed u32 registers; got {reg}. Fix: pack two f16 lanes per u32 fragment operand."
                )));
            }
        }
        for reg in &c {
            if reg.0 != PtxType::F32 {
                return Err(EmitError::InvalidDescriptor(format!(
                    "MatrixMma f32 accumulators must be f32 registers; got {reg}. Fix: pass f32 accumulator operands."
                )));
            }
        }

        let d = [
            self.alloc(PtxType::F32),
            self.alloc(PtxType::F32),
            self.alloc(PtxType::F32),
            self.alloc(PtxType::F32),
        ];
        let _ = writeln!(
            self.text,
            "    mma.sync.aligned.m16n8k16.row.col.f32.f16.f16.f32    {{{}, {}, {}, {}}}, {{{}, {}, {}, {}}}, {{{}, {}}}, {{{}, {}, {}, {}}};",
            d[0],
            d[1],
            d[2],
            d[3],
            a[0],
            a[1],
            a[2],
            a[3],
            b[0],
            b[1],
            c[0],
            c[1],
            c[2],
            c[3],
        );
        Ok(d)
    }
}
