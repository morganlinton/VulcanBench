use std::fmt::Write as _;

use vyre_lower::{KernelBody, KernelOp, KernelOpKind};

use super::schedule::is_schedulable_pure_op;
use super::BodyCtx;
use crate::reg::PtxType;
use crate::EmitError;

const MAX_PREDICATED_BODY_OPS: usize = 4;

impl BodyCtx<'_> {
    pub(super) fn emit_region(
        &mut self,
        body: &KernelBody,
        op: &KernelOp,
        generator: &str,
    ) -> Result<(), EmitError> {
        let _ = writeln!(self.text, "    // region: {generator}");
        if let Some(child_id) = op.operands.first() {
            if let Some(child) = body.child_bodies.get(*child_id as usize) {
                self.emit_body(child)?;
            }
        }
        Ok(())
    }

    pub(super) fn emit_structured_block(
        &mut self,
        body: &KernelBody,
        op: &KernelOp,
    ) -> Result<(), EmitError> {
        if let Some(child_id) = op.operands.first() {
            if let Some(child) = body.child_bodies.get(*child_id as usize) {
                self.emit_body(child)?;
            }
        }
        Ok(())
    }

    pub(super) fn emit_structured_if_then(
        &mut self,
        body: &KernelBody,
        op: &KernelOp,
    ) -> Result<(), EmitError> {
        let cond_id = *op
            .operands
            .first()
            .ok_or_else(|| EmitError::InvalidDescriptor("StructuredIfThen missing cond".into()))?;
        let body_id = *op.operands.get(1).ok_or_else(|| {
            EmitError::InvalidDescriptor("StructuredIfThen missing body index".into())
        })?;
        let cond_reg = self.lookup_operand(cond_id)?;
        if let Some(child) = body.child_bodies.get(body_id as usize) {
            if child.ops.len() <= MAX_PREDICATED_BODY_OPS {
                let pred = self.pred_from_boolish(cond_reg);
                if self.emit_predicated_store_body(child, pred, false)? {
                    return Ok(());
                }
            }
        }
        let branch_pred = self.pred_from_boolish(cond_reg);
        let end_label = self.alloc_label("if_end");
        let _ = writeln!(self.text, "    @!{branch_pred} bra {end_label};");
        if let Some(child) = body.child_bodies.get(body_id as usize) {
            self.emit_body(child)?;
        }
        let _ = writeln!(self.text, "{end_label}:");
        Ok(())
    }

    pub(super) fn emit_structured_if_then_else(
        &mut self,
        body: &KernelBody,
        op: &KernelOp,
    ) -> Result<(), EmitError> {
        let cond_id = *op.operands.first().ok_or_else(|| {
            EmitError::InvalidDescriptor("StructuredIfThenElse missing cond".into())
        })?;
        let then_id = *op.operands.get(1).ok_or_else(|| {
            EmitError::InvalidDescriptor("StructuredIfThenElse missing then index".into())
        })?;
        let else_id = *op.operands.get(2).ok_or_else(|| {
            EmitError::InvalidDescriptor("StructuredIfThenElse missing else index".into())
        })?;
        let cond_reg = self.lookup_operand(cond_id)?;
        if let (Some(then_body), Some(else_body)) = (
            body.child_bodies.get(then_id as usize),
            body.child_bodies.get(else_id as usize),
        ) {
            if then_body.ops.len() <= MAX_PREDICATED_BODY_OPS
                && else_body.ops.len() <= MAX_PREDICATED_BODY_OPS
                && predicated_store_body_supported(then_body)
                && predicated_store_body_supported(else_body)
            {
                let pred = self.pred_from_boolish(cond_reg);
                let then_emitted = self.emit_predicated_store_body(then_body, pred, false)?;
                let else_emitted = self.emit_predicated_store_body(else_body, pred, true)?;
                if then_emitted && else_emitted {
                    return Ok(());
                }
            }
        }
        let branch_pred = self.pred_from_boolish(cond_reg);
        let else_label = self.alloc_label("if_else");
        let end_label = self.alloc_label("if_end");
        let _ = writeln!(self.text, "    @!{branch_pred} bra {else_label};");
        if let Some(child) = body.child_bodies.get(then_id as usize) {
            self.emit_body(child)?;
        }
        let _ = writeln!(self.text, "    bra {end_label};");
        let _ = writeln!(self.text, "{else_label}:");
        if let Some(child) = body.child_bodies.get(else_id as usize) {
            self.emit_body(child)?;
        }
        let _ = writeln!(self.text, "{end_label}:");
        Ok(())
    }

    fn emit_predicated_store_body(
        &mut self,
        child: &KernelBody,
        pred: crate::reg::Reg,
        negate: bool,
    ) -> Result<bool, EmitError> {
        if !predicated_store_body_supported(child) {
            return Ok(false);
        }
        let mut emitted_store = false;
        for op in &child.ops {
            if matches!(
                op.kind,
                KernelOpKind::StoreGlobal | KernelOpKind::StoreShared
            ) {
                emitted_store |= self.emit_predicated_store(op, pred, negate)?;
            } else {
                self.emit_op(child, op)?;
            }
        }
        Ok(emitted_store)
    }

    pub(super) fn emit_structured_for_loop(
        &mut self,
        body: &KernelBody,
        op: &KernelOp,
        loop_var: &str,
    ) -> Result<(), EmitError> {
        let lo_id = *op
            .operands
            .first()
            .ok_or_else(|| EmitError::InvalidDescriptor("StructuredForLoop missing lo".into()))?;
        let hi_id = *op
            .operands
            .get(1)
            .ok_or_else(|| EmitError::InvalidDescriptor("StructuredForLoop missing hi".into()))?;
        let body_id = *op.operands.get(2).ok_or_else(|| {
            EmitError::InvalidDescriptor("StructuredForLoop missing body index".into())
        })?;
        let lo_reg = self.lookup_operand(lo_id)?;
        let hi_reg = self.lookup_operand(hi_id)?;
        let var_reg = self.alloc(PtxType::U32);
        let cond_reg = self.alloc(PtxType::Bool);
        let one_reg = self.alloc(PtxType::U32);
        let head = self.alloc_label("for_head");
        let exit = self.alloc_label("for_exit");
        let _ = writeln!(self.text, "    // for {loop_var} in [{lo_reg}, {hi_reg})");
        let _ = writeln!(self.text, "    mov.u32    {var_reg}, {lo_reg};");
        let _ = writeln!(self.text, "    mov.u32    {one_reg}, 1;");
        let _ = writeln!(self.text, "{head}:");
        let _ = writeln!(
            self.text,
            "    setp.ge.u32 {cond_reg}, {var_reg}, {hi_reg};"
        );
        let _ = writeln!(self.text, "    @{cond_reg} bra {exit};");
        self.loop_indices.insert(loop_var.into(), var_reg);
        if let Some(child) = body.child_bodies.get(body_id as usize) {
            self.emit_body(child)?;
        }
        self.loop_indices.remove(loop_var);
        let _ = writeln!(self.text, "    add.u32    {var_reg}, {var_reg}, {one_reg};");
        let _ = writeln!(self.text, "    bra {head};");
        let _ = writeln!(self.text, "{exit}:");
        Ok(())
    }

    pub(super) fn emit_loop_index(
        &mut self,
        op: &KernelOp,
        loop_var: &str,
    ) -> Result<(), EmitError> {
        let reg = *self.loop_indices.get(loop_var).ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "LoopIndex `{loop_var}` appeared outside its StructuredForLoop"
            ))
        })?;
        self.bind_result(op, reg)
    }
}

fn predicated_store_body_supported(body: &KernelBody) -> bool {
    let mut has_store = false;
    for op in &body.ops {
        if matches!(
            op.kind,
            KernelOpKind::StoreGlobal | KernelOpKind::StoreShared
        ) {
            has_store = true;
            continue;
        }
        if !is_schedulable_pure_op(op) {
            return false;
        }
    }
    has_store
}
