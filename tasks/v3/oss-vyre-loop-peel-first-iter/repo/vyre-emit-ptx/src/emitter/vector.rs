use std::fmt::Write as _;

use smallvec::SmallVec;
use vyre_foundation::ir::DataType;
use vyre_lower::{KernelBody, KernelOpKind};

use super::facts::EmitFacts;
use super::format::{is_ptx_vectorizable_dtype, write_reg_tuple};
use super::operands::{read_store_operands, read_two_operands};
use super::schedule::{is_schedulable_pure_op, is_scheduling_fence};
use super::{body_descendants_read_operand, BodyCtx};
use crate::reg::PtxType;
use crate::EmitError;

type VectorChain = SmallVec<[usize; 4]>;
const PTX_VECTOR_WIDTH_V2: usize = 2;
const PTX_VECTOR_WIDTH_V4: usize = 4;
const PTX_VECTOR_LOAD_GLOBAL_PREFIX: &str = "ld.global";
const PTX_VECTOR_LOAD_SHARED_PREFIX: &str = "ld.shared";

impl BodyCtx<'_> {
    pub(super) fn collect_vec_load_chain(
        &self,
        body: &KernelBody,
        facts: &EmitFacts,
        start_idx: usize,
    ) -> Result<Option<VectorChain>, EmitError> {
        let op = &body.ops[start_idx];
        if !is_vector_load_op(&op.kind) {
            return Ok(None);
        }
        let (slot, base_idx_id) = read_two_operands(op, "vector load")?;
        let binding = self.binding_for_slot(slot)?;
        if !is_ptx_vectorizable_dtype(&binding.element_type) || op.result.is_none() {
            return Ok(None);
        }

        let mut chain = SmallVec::from_buf([start_idx, 0, 0, 0]);
        chain.truncate(1);
        let mut prev_idx_id = base_idx_id;
        let mut scan_idx = start_idx + 1;
        while scan_idx < body.ops.len() && chain.len() < PTX_VECTOR_WIDTH_V4 {
            let mut next_idx = scan_idx;
            while next_idx < body.ops.len() {
                let next = &body.ops[next_idx];
                if is_vector_load_op(&next.kind) {
                    break;
                }
                if is_scheduling_fence(next) || !is_schedulable_pure_op(next) {
                    break;
                }
                next_idx += 1;
            }

            if next_idx >= body.ops.len() {
                break;
            }
            let next = &body.ops[next_idx];
            if !is_vector_load_op(&next.kind) || next.result.is_none() {
                break;
            }
            let (next_slot, next_index_id) = read_two_operands(next, "vector load")?;
            if next_slot != slot || !facts.is_index_plus_one(body, next_index_id, prev_idx_id) {
                break;
            }
            chain.push(next_idx);
            prev_idx_id = next_index_id;
            scan_idx = next_idx + 1;
        }

        align_vector_chain(body, facts, base_idx_id, chain)
    }

    pub(super) fn collect_vec_store_chain(
        &self,
        body: &KernelBody,
        facts: &EmitFacts,
        start_idx: usize,
    ) -> Result<Option<VectorChain>, EmitError> {
        let op = &body.ops[start_idx];
        if !matches!(op.kind, KernelOpKind::StoreGlobal) {
            return Ok(None);
        }
        let (slot, base_idx_id, _) = read_store_operands(op)?;
        let binding = self.binding_for_slot(slot)?;
        if !is_ptx_vectorizable_dtype(&binding.element_type) {
            return Ok(None);
        }

        let mut chain = SmallVec::from_buf([start_idx, 0, 0, 0]);
        chain.truncate(1);
        let mut prev_idx_id = base_idx_id;
        let mut scan_idx = start_idx + 1;
        while scan_idx < body.ops.len() && chain.len() < PTX_VECTOR_WIDTH_V4 {
            let mut next_idx = scan_idx;
            let mut skipped_pure_ops: SmallVec<[usize; 4]> = SmallVec::new();
            while next_idx < body.ops.len() {
                let next = &body.ops[next_idx];
                if matches!(next.kind, KernelOpKind::StoreGlobal) {
                    break;
                }
                if is_scheduling_fence(next) || !is_schedulable_pure_op(next) {
                    break;
                }
                skipped_pure_ops.push(next_idx);
                next_idx += 1;
            }

            if next_idx >= body.ops.len() {
                break;
            }
            let next = &body.ops[next_idx];
            if !matches!(next.kind, KernelOpKind::StoreGlobal) {
                break;
            }
            let (next_slot, next_index_id, next_value_id) = read_store_operands(next)?;
            if skipped_pure_ops
                .iter()
                .any(|op_idx| body.ops[*op_idx].result == Some(next_value_id))
            {
                break;
            }
            if next_slot != slot || !facts.is_index_plus_one(body, next_index_id, prev_idx_id) {
                break;
            }
            chain.push(next_idx);
            prev_idx_id = next_index_id;
            scan_idx = next_idx + 1;
        }

        let Some(chain) = align_vector_chain(body, facts, base_idx_id, chain)? else {
            return Ok(None);
        };
        if !self.store_chain_values_are_vector_safe(body, facts, &chain)? {
            return Ok(None);
        }
        Ok(Some(chain))
    }

    fn store_chain_values_are_vector_safe(
        &self,
        body: &KernelBody,
        facts: &EmitFacts,
        chain: &[usize],
    ) -> Result<bool, EmitError> {
        let mut load_producers: SmallVec<[usize; 4]> = SmallVec::new();
        for &op_idx in chain {
            let (_, _, value_id) = read_store_operands(&body.ops[op_idx])?;
            let Some(producer_idx) = facts.producer_idx(value_id) else {
                continue;
            };
            if is_vector_load_op(&body.ops[producer_idx].kind) {
                load_producers.push(producer_idx);
            }
        }
        if load_producers.is_empty() {
            return Ok(true);
        }
        if load_producers.len() != chain.len() {
            return Ok(false);
        }
        let Some(load_chain) = self.collect_vec_load_chain(body, facts, load_producers[0])? else {
            return Ok(false);
        };
        Ok(load_chain.len() >= load_producers.len()
            && load_producers
                .iter()
                .zip(load_chain.iter())
                .all(|(producer, fused)| producer == fused))
    }

    pub(super) fn emit_vec_load_chain(
        &mut self,
        body: &KernelBody,
        chain: &[usize],
    ) -> Result<(), EmitError> {
        let first = &body.ops[chain[0]];
        let (binding_slot, index_op_id) = read_two_operands(first, "vector load")?;
        let element_type = self.binding_for_slot(binding_slot)?.element_type.clone();
        let memory_class = self.binding_for_slot(binding_slot)?.memory_class;
        let elem_ty = PtxType::from_dtype(&element_type)?;
        let vector_ty = vector_memory_type(&element_type, elem_ty);
        let final_addr =
            self.emit_global_address_operand(binding_slot, index_op_id, &element_type)?;
        let load_space = self.load_space_for(binding_slot, memory_class);
        let mut regs: SmallVec<[crate::reg::Reg; 4]> = SmallVec::new();
        for &_op_idx in chain {
            let reg = self.alloc(vector_ty);
            regs.push(reg);
        }
        let (mnemonic_prefix, cache_suffix) =
            vector_load_mnemonic_parts(load_space).ok_or_else(|| {
                EmitError::InvalidDescriptor(format!(
                    "unsupported PTX vector load space `{load_space}` for fused vector load"
                ))
            })?;
        let _ = write!(
            self.text,
            "    {mnemonic_prefix}{cache_suffix}.v{}.{}    ",
            chain.len(),
            vector_ty.ptx_type_str()
        );
        write_reg_tuple(&mut self.text, &regs);
        self.text.push_str(", ");
        self.write_mem_operand(final_addr)?;
        self.text.push_str(";\n");
        for (&op_idx, reg) in chain.iter().zip(regs.iter().copied()) {
            let canonical = if matches!(element_type, DataType::Bool) {
                let pred = self.alloc(PtxType::Bool);
                let _ = writeln!(self.text, "    setp.ne.u32    {pred}, {reg}, 0;");
                pred
            } else {
                self.canonicalize_f32(reg)
            };
            self.bind_result(&body.ops[op_idx], canonical)?;
        }
        Ok(())
    }

    pub(super) fn emit_vec_store_chain(
        &mut self,
        body: &KernelBody,
        chain: &[usize],
    ) -> Result<(), EmitError> {
        let first = &body.ops[chain[0]];
        let (binding_slot, index_op_id, _) = read_store_operands(first)?;
        let element_type = self.binding_for_slot(binding_slot)?.element_type.clone();
        let elem_ty = PtxType::from_dtype(&element_type)?;
        let vector_ty = vector_memory_type(&element_type, elem_ty);
        let final_addr =
            self.emit_global_address_operand(binding_slot, index_op_id, &element_type)?;
        let mut regs: SmallVec<[crate::reg::Reg; 4]> = SmallVec::new();
        for &op_idx in chain {
            let (_, _, value_op_id) = read_store_operands(&body.ops[op_idx])?;
            let value = self.lookup_operand(value_op_id)?;
            regs.push(if matches!(element_type, DataType::Bool) {
                let pred = self.pred_from_boolish(value);
                let word = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    selp.u32    {word}, 1, 0, {pred};");
                word
            } else if elem_ty == PtxType::F32 {
                self.canonicalize_f32(value)
            } else {
                value
            });
        }
        let _ = write!(
            self.text,
            "    st.global.v{}.{}    ",
            chain.len(),
            vector_ty.ptx_type_str()
        );
        self.write_mem_operand(final_addr)?;
        self.text.push_str(", ");
        write_reg_tuple(&mut self.text, &regs);
        self.text.push_str(";\n");
        Ok(())
    }

    pub(super) fn mark_dead_vec_index_ops(
        &self,
        body: &KernelBody,
        facts: &EmitFacts,
        chain: &[usize],
        skip: &mut [bool],
    ) -> Result<(), EmitError> {
        let mut candidate_producers: SmallVec<[usize; 8]> = SmallVec::new();
        for &op_idx in chain.iter().skip(1) {
            let op = &body.ops[op_idx];
            let index_id = match op.kind {
                KernelOpKind::LoadGlobal
                | KernelOpKind::LoadShared
                | KernelOpKind::LoadConstant => read_two_operands(op, "vector load")?.1,
                KernelOpKind::StoreGlobal => read_store_operands(op)?.1,
                _ => continue,
            };
            let Some(producer_idx) = facts.producer_idx(index_id) else {
                continue;
            };
            if producer_idx >= op_idx || !is_schedulable_pure_op(&body.ops[producer_idx]) {
                continue;
            }
            if !candidate_producers.contains(&producer_idx) {
                candidate_producers.push(producer_idx);
            }
        }

        for &producer_idx in &candidate_producers {
            let Some(result_id) = body.ops[producer_idx].result else {
                continue;
            };
            if body_descendants_read_operand(body, result_id) {
                continue;
            }
            let consumers = facts.consumer_indices(result_id).unwrap_or(&[]);
            if consumers.iter().all(|consumer_idx| {
                chain.contains(consumer_idx)
                    || candidate_producers.contains(consumer_idx)
                    || skip.get(*consumer_idx).copied().unwrap_or(false)
            }) {
                skip[producer_idx] = true;
            }
        }
        Ok(())
    }
}

fn truncate_vector_chain(mut chain: VectorChain) -> Result<Option<VectorChain>, EmitError> {
    if chain.len() >= PTX_VECTOR_WIDTH_V4 {
        chain.truncate(PTX_VECTOR_WIDTH_V4);
        Ok(Some(chain))
    } else if chain.len() >= PTX_VECTOR_WIDTH_V2 {
        chain.truncate(PTX_VECTOR_WIDTH_V2);
        Ok(Some(chain))
    } else {
        Ok(None)
    }
}

fn align_vector_chain(
    body: &KernelBody,
    facts: &EmitFacts,
    base_idx_id: u32,
    chain: VectorChain,
) -> Result<Option<VectorChain>, EmitError> {
    let Some(mut chain) = truncate_vector_chain(chain)? else {
        return Ok(None);
    };
    if index_may_be_aligned_for_vector_width(body, facts, base_idx_id, chain.len() as u32) {
        return Ok(Some(chain));
    }
    if chain.len() >= PTX_VECTOR_WIDTH_V4 {
        chain.truncate(PTX_VECTOR_WIDTH_V2);
        if index_may_be_aligned_for_vector_width(
            body,
            facts,
            base_idx_id,
            PTX_VECTOR_WIDTH_V2 as u32,
        ) {
            return Ok(Some(chain));
        }
    }
    Ok(None)
}

fn index_may_be_aligned_for_vector_width(
    body: &KernelBody,
    facts: &EmitFacts,
    base_idx_id: u32,
    width: u32,
) -> bool {
    matches!(facts.index_modulo(body, base_idx_id, width), Some(0))
}

fn vector_load_mnemonic_parts(load_space: &str) -> Option<(&'static str, &'static str)> {
    match load_space {
        "global" => Some((PTX_VECTOR_LOAD_GLOBAL_PREFIX, "")),
        "global.nc" => Some((PTX_VECTOR_LOAD_GLOBAL_PREFIX, ".nc")),
        "shared" => Some((PTX_VECTOR_LOAD_SHARED_PREFIX, "")),
        _ => None,
    }
}

fn vector_memory_type(element_type: &DataType, elem_ty: PtxType) -> PtxType {
    if matches!(element_type, DataType::Bool) {
        PtxType::U32
    } else {
        elem_ty
    }
}

fn is_vector_load_op(kind: &KernelOpKind) -> bool {
    // `LoadConstant` IS fusable here, and must be: this PTX backend has NO
    // `.const` state-space emission path. `load_space_for` maps
    // `MemoryClass::Constant` to the `"global"` (or `"global.nc"`) address space,
    // and the scalar dispatch in `emitter.rs` routes `LoadGlobal` and
    // `LoadConstant` through the *identical* `emit_memory_address` +
    // `load_space_for` + `emit_load_value` path (Constant pointers live in
    // `slot_to_ptr`, same as global). The fusion's address path
    // (`emit_global_address_operand` + `vector_load_mnemonic_parts`) handles
    // exactly `"global"`/`"global.nc"`, so a fused `LoadConstant` chain emits a
    // correct `ld.global[.nc].vN`.
    //
    // Excluding it was an active Law-7 pessimization: `const_buffer_promote`
    // rewrites read-only-global `LoadGlobal` → `LoadConstant` BEFORE emission, so
    // every promoted read-only buffer fell out of fusion and emitted 4× scalar
    // `ld.global.u32` instead of one `ld.global.v4.u32`. It also desynced the
    // emitter from `vec_memory_fusion::MemoryFusionKind::Load::matches`, which
    // already counts `LoadConstant` as a fusion candidate.
    //
    // `LoadShared` stays excluded: shared loads lower via `slot_to_shared_symbol`
    // (a distinct address-operand path) rather than `emit_global_address_operand`.
    matches!(
        kind,
        KernelOpKind::LoadGlobal | KernelOpKind::LoadConstant
    )
}
