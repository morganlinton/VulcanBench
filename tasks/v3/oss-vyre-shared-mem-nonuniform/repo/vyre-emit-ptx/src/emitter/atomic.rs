use std::fmt::Write as _;

use vyre_foundation::ir::AtomicOp;
use vyre_lower::KernelOp;

use super::BodyCtx;
use crate::reg::{PtxType, Reg};
use crate::EmitError;

impl BodyCtx<'_> {
    pub(super) fn emit_atomic(
        &mut self,
        op: &KernelOp,
        atomic_op: AtomicOp,
    ) -> Result<(), EmitError> {
        // CompareExchange / CompareExchangeWeak take 4 operands and use a
        // distinct PTX mnemonic  -  split out so the common single-value
        // RMW path stays clean.
        if matches!(
            atomic_op,
            AtomicOp::CompareExchange | AtomicOp::CompareExchangeWeak
        ) {
            return self.emit_atomic_cas(op);
        }
        let mnemonic = match atomic_op {
            AtomicOp::Add => "add",
            AtomicOp::And => "and",
            AtomicOp::Or => "or",
            AtomicOp::Xor => "xor",
            AtomicOp::Min => "min",
            AtomicOp::Max | AtomicOp::LruUpdate => "max",
            AtomicOp::Exchange => "exch",
            _ => {
                return Err(EmitError::UnsupportedOp(KernelOp {
                    kind: op.kind.clone(),
                    operands: op.operands.clone(),
                    result: op.result,
                }));
            }
        };
        let binding_slot = *op
            .operands
            .first()
            .ok_or_else(|| EmitError::InvalidDescriptor("Atomic missing slot".into()))?;
        let index_op_id = *op
            .operands
            .get(1)
            .ok_or_else(|| EmitError::InvalidDescriptor("Atomic missing index".into()))?;
        let value_op_id = *op
            .operands
            .get(2)
            .ok_or_else(|| EmitError::InvalidDescriptor("Atomic missing value".into()))?;
        let element_type = self.binding_for_slot(binding_slot)?.element_type.clone();
        let elem_ty = PtxType::from_dtype(&element_type)?;
        let global_ptr =
            *self
                .slot_to_ptr
                .get(&binding_slot)
                .ok_or_else(|| EmitError::InvalidBinding {
                    slot: binding_slot,
                    reason: "global pointer not preloaded".into(),
                })?;
        let index_reg = self.lookup_operand(index_op_id)?;
        let value_reg =
            self.atomic_value_reg(atomic_op, self.lookup_operand(value_op_id)?, elem_ty)?;
        let in_bounds = self
            .full_workgroup_entry
            .then(|| self.emit_index_reg_in_bounds_pred(binding_slot, index_reg));
        let stride = element_type
            .size_bytes()
            .ok_or_else(|| EmitError::UnsupportedDataType(format!("{element_type:?}")))?;
        let addr_reg = self.alloc(PtxType::U64);
        let _ = writeln!(
            self.text,
            "    mul.wide.u32    {addr_reg}, {index_reg}, {stride};"
        );
        let final_addr = self.alloc(PtxType::U64);
        let _ = writeln!(
            self.text,
            "    add.u64    {final_addr}, {global_ptr}, {addr_reg};"
        );
        let type_suffix = atomic_type_suffix(atomic_op, elem_ty)?;
        let result_reg = self.alloc(elem_ty);
        if let Some(in_bounds) = in_bounds {
            let _ = writeln!(
                self.text,
                "    mov.{}    {result_reg}, 0;",
                elem_ty.ptx_type_str()
            );
            let _ = writeln!(
                self.text,
                "    @{in_bounds} atom.global.{mnemonic}.{type_suffix}    {result_reg}, [{final_addr}], {value_reg};"
            );
        } else {
            let _ = writeln!(
                self.text,
                "    atom.global.{mnemonic}.{type_suffix}    {result_reg}, [{final_addr}], {value_reg};"
            );
        }
        self.bind_result(op, result_reg)
    }

    fn atomic_value_reg(
        &mut self,
        atomic_op: AtomicOp,
        value_reg: Reg,
        elem_ty: PtxType,
    ) -> Result<Reg, EmitError> {
        if value_reg.0 == PtxType::Bool
            && matches!(
                atomic_op,
                AtomicOp::Exchange | AtomicOp::And | AtomicOp::Or | AtomicOp::Xor
            )
        {
            return Ok(self.coerce_for_store(value_reg, elem_ty));
        }
        Ok(value_reg)
    }

    /// Lower `Atomic { op: CompareExchange | CompareExchangeWeak }` to PTX
    /// `atom.global.cas.b32`. PTX CAS returns the prior value of the slot;
    /// callers compare it to `cmp` to decide whether the swap committed.
    /// Operands: `[slot, index, cmp_val, new_val]`.
    fn emit_atomic_cas(&mut self, op: &KernelOp) -> Result<(), EmitError> {
        let binding_slot = *op
            .operands
            .first()
            .ok_or_else(|| EmitError::InvalidDescriptor("AtomicCAS missing slot".into()))?;
        let index_op_id = *op
            .operands
            .get(1)
            .ok_or_else(|| EmitError::InvalidDescriptor("AtomicCAS missing index".into()))?;
        let cmp_op_id = *op
            .operands
            .get(2)
            .ok_or_else(|| EmitError::InvalidDescriptor("AtomicCAS missing cmp value".into()))?;
        let new_op_id = *op
            .operands
            .get(3)
            .ok_or_else(|| EmitError::InvalidDescriptor("AtomicCAS missing new value".into()))?;
        let element_type = self.binding_for_slot(binding_slot)?.element_type.clone();
        let elem_ty = PtxType::from_dtype(&element_type)?;
        if !matches!(elem_ty, PtxType::U32 | PtxType::I32) {
            return Err(EmitError::UnsupportedDataType(format!(
                "atom.global.cas requires 32-bit element type; got {:?}",
                element_type
            )));
        }
        let global_ptr =
            *self
                .slot_to_ptr
                .get(&binding_slot)
                .ok_or_else(|| EmitError::InvalidBinding {
                    slot: binding_slot,
                    reason: "global pointer not preloaded".into(),
                })?;
        let index_reg = self.lookup_operand(index_op_id)?;
        let cmp_reg = self.lookup_operand(cmp_op_id)?;
        let new_reg = self.lookup_operand(new_op_id)?;
        let in_bounds = self
            .full_workgroup_entry
            .then(|| self.emit_index_reg_in_bounds_pred(binding_slot, index_reg));
        let stride = element_type
            .size_bytes()
            .ok_or_else(|| EmitError::UnsupportedDataType(format!("{element_type:?}")))?;
        let addr_reg = self.alloc(PtxType::U64);
        let _ = writeln!(
            self.text,
            "    mul.wide.u32    {addr_reg}, {index_reg}, {stride};"
        );
        let final_addr = self.alloc(PtxType::U64);
        let _ = writeln!(
            self.text,
            "    add.u64    {final_addr}, {global_ptr}, {addr_reg};"
        );
        let result_reg = self.alloc(elem_ty);
        if let Some(in_bounds) = in_bounds {
            let _ = writeln!(
                self.text,
                "    mov.{}    {result_reg}, 0;",
                elem_ty.ptx_type_str()
            );
            let _ = writeln!(
                self.text,
                "    @{in_bounds} atom.global.cas.b32    {result_reg}, [{final_addr}], {cmp_reg}, {new_reg};"
            );
        } else {
            let _ = writeln!(
                self.text,
                "    atom.global.cas.b32    {result_reg}, [{final_addr}], {cmp_reg}, {new_reg};"
            );
        }
        self.bind_result(op, result_reg)
    }
}

fn atomic_type_suffix(atomic_op: AtomicOp, elem_ty: PtxType) -> Result<&'static str, EmitError> {
    if matches!(
        atomic_op,
        AtomicOp::Exchange | AtomicOp::And | AtomicOp::Or | AtomicOp::Xor
    ) {
        return match elem_ty {
            PtxType::U32 | PtxType::I32 => Ok("b32"),
            other => Err(EmitError::UnsupportedDataType(format!(
                "atom.global bitwise/exchange requires a 32-bit integer element type; got {other:?}"
            ))),
        };
    }
    Ok(elem_ty.ptx_type_str())
}
