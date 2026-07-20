use std::fmt::Write as _;

use vyre_foundation::ir::DataType;
use vyre_lower::MemoryClass;

use super::BodyCtx;
use crate::reg::{PtxType, Reg};
use crate::EmitError;

pub(super) struct MemAddress {
    pub(super) space: &'static str,
    pub(super) operand: MemOperand,
}

#[derive(Clone, Copy)]
pub(super) enum MemOperand {
    Reg(Reg),
    RegOffset(Reg, u64),
    SharedSlotOffset(u32, u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AsyncCopyDirection {
    Load,
    Store,
}

impl BodyCtx<'_> {
    pub(super) fn emit_load_value(
        &mut self,
        address: MemAddress,
        load_space: &str,
        element_type: &DataType,
        elem_ty: PtxType,
    ) -> Result<Reg, EmitError> {
        match element_type {
            DataType::U8 => {
                let out = self.alloc(PtxType::U32);
                let _ = write!(self.text, "    ld.{load_space}.u8    {out}, ");
                self.write_mem_operand(address.operand)?;
                self.text.push_str(";\n");
                Ok(out)
            }
            DataType::I8 => {
                let out = self.alloc(PtxType::I32);
                let _ = write!(self.text, "    ld.{load_space}.s8    {out}, ");
                self.write_mem_operand(address.operand)?;
                self.text.push_str(";\n");
                Ok(out)
            }
            DataType::U16 => {
                let out = self.alloc(PtxType::U32);
                let _ = write!(self.text, "    ld.{load_space}.u16    {out}, ");
                self.write_mem_operand(address.operand)?;
                self.text.push_str(";\n");
                Ok(out)
            }
            DataType::I16 => {
                let out = self.alloc(PtxType::I32);
                let _ = write!(self.text, "    ld.{load_space}.s16    {out}, ");
                self.write_mem_operand(address.operand)?;
                self.text.push_str(";\n");
                Ok(out)
            }
            DataType::Bool => {
                let word = self.alloc(PtxType::U32);
                let out = self.alloc(PtxType::Bool);
                let _ = write!(self.text, "    ld.{load_space}.u32    {word}, ");
                self.write_mem_operand(address.operand)?;
                self.text.push_str(";\n");
                let _ = writeln!(self.text, "    setp.ne.u32    {out}, {word}, 0;");
                Ok(out)
            }
            DataType::F16 => {
                let packed = self.alloc(PtxType::B16);
                let out = self.alloc(PtxType::F32);
                let _ = write!(self.text, "    ld.{load_space}.b16    {packed}, ");
                self.write_mem_operand(address.operand)?;
                self.text.push_str(";\n");
                let _ = writeln!(self.text, "    cvt.f32.f16    {out}, {packed};");
                Ok(out)
            }
            DataType::BF16 => {
                let packed = self.alloc(PtxType::B16);
                let out = self.alloc(PtxType::F32);
                let _ = write!(self.text, "    ld.{load_space}.b16    {packed}, ");
                self.write_mem_operand(address.operand)?;
                self.text.push_str(";\n");
                let _ = writeln!(self.text, "    cvt.f32.bf16    {out}, {packed};");
                Ok(out)
            }
            _ => {
                let val_reg = self.alloc(elem_ty);
                let _ = write!(
                    self.text,
                    "    ld.{load_space}.{}    {val_reg}, ",
                    elem_ty.ptx_type_str(),
                );
                self.write_mem_operand(address.operand)?;
                self.text.push_str(";\n");
                Ok(self.canonicalize_f32(val_reg))
            }
        }
    }

    pub(super) fn emit_store_value(
        &mut self,
        guard: Option<(String, Reg)>,
        address: MemAddress,
        element_type: &DataType,
        value_reg: Reg,
    ) -> Result<(), EmitError> {
        match element_type {
            DataType::U8 | DataType::I8 => self.emit_raw_store(guard, address, "u8", value_reg),
            DataType::U16 | DataType::I16 => self.emit_raw_store(guard, address, "u16", value_reg),
            DataType::Bool => {
                let pred = self.pred_from_boolish(value_reg);
                let word = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    selp.u32    {word}, 1, 0, {pred};");
                self.emit_raw_store(guard, address, "u32", word)
            }
            DataType::F16 => {
                let f32_value = self.ensure_f32_store_operand(value_reg);
                let packed = self.alloc(PtxType::B16);
                let _ = writeln!(self.text, "    cvt.rn.f16.f32    {packed}, {f32_value};");
                self.emit_raw_store(guard, address, "b16", packed)
            }
            DataType::BF16 => {
                let f32_value = self.ensure_f32_store_operand(value_reg);
                let packed = self.alloc(PtxType::B16);
                let _ = writeln!(self.text, "    cvt.rn.bf16.f32    {packed}, {f32_value};");
                self.emit_raw_store(guard, address, "b16", packed)
            }
            _ => {
                let elem_ty = PtxType::from_dtype(element_type)?;
                let value_reg = if elem_ty == PtxType::F32 {
                    self.canonicalize_f32(value_reg)
                } else {
                    value_reg
                };
                self.emit_raw_store(guard, address, elem_ty.ptx_type_str(), value_reg)
            }
        }
    }

    fn ensure_f32_store_operand(&mut self, value_reg: Reg) -> Reg {
        match value_reg.0 {
            PtxType::F32 => value_reg,
            PtxType::I32 => {
                let out = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    cvt.rn.f32.s32    {out}, {value_reg};");
                out
            }
            PtxType::Bool => {
                let word = self.alloc(PtxType::U32);
                let out = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    selp.u32    {word}, 1, 0, {value_reg};");
                let _ = writeln!(self.text, "    cvt.rn.f32.u32    {out}, {word};");
                out
            }
            PtxType::B16 | PtxType::U32 => {
                let out = self.alloc(PtxType::F32);
                let _ = writeln!(self.text, "    cvt.rn.f32.u32    {out}, {value_reg};");
                out
            }
            PtxType::U64 => {
                let out = self.alloc(PtxType::F32);
                // PTX cvt.rn.f32.u64 converts the full 64-bit value to F32
                // with round-to-nearest. The previous two-step path
                // (cvt.u32.u64 + cvt.rn.f32.u32) silently discarded the high
                // 32 bits before conversion — a silent truncation of any value
                // > 0xFFFFFFFF.
                let _ = writeln!(self.text, "    cvt.rn.f32.u64    {out}, {value_reg};");
                out
            }
        }
    }

    fn emit_raw_store(
        &mut self,
        guard: Option<(String, Reg)>,
        address: MemAddress,
        ptx_type: &str,
        value_reg: Reg,
    ) -> Result<(), EmitError> {
        match guard {
            Some((guard, pred)) => {
                let _ = write!(
                    self.text,
                    "    {guard}{pred} st.{}.{}    ",
                    address.space, ptx_type,
                );
            }
            None => {
                let _ = write!(self.text, "    st.{}.{}    ", address.space, ptx_type);
            }
        }
        self.write_mem_operand(address.operand)?;
        let _ = writeln!(self.text, ", {value_reg};");
        Ok(())
    }

    pub(super) fn emit_global_address_operand(
        &mut self,
        binding_slot: u32,
        index_op_id: u32,
        element_type: &DataType,
    ) -> Result<MemOperand, EmitError> {
        let global_ptr =
            *self
                .slot_to_ptr
                .get(&binding_slot)
                .ok_or_else(|| EmitError::InvalidBinding {
                    slot: binding_slot,
                    reason: "global pointer not preloaded".into(),
                })?;
        if let Some(byte_offset) = self.immediate_byte_offset(index_op_id, element_type)? {
            return Ok(MemOperand::RegOffset(global_ptr, byte_offset));
        }
        let index_reg = self.lookup_operand(index_op_id)?;
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
        Ok(MemOperand::Reg(final_addr))
    }

    pub(super) fn emit_memory_address(
        &mut self,
        binding_slot: u32,
        index_op_id: u32,
        element_type: &DataType,
        memory_class: MemoryClass,
    ) -> Result<MemAddress, EmitError> {
        if let Some(byte_offset) = self.immediate_byte_offset(index_op_id, element_type)? {
            return self.emit_memory_address_immediate(binding_slot, byte_offset, memory_class);
        }
        let index_reg = self.lookup_operand(index_op_id)?;
        self.emit_memory_address_from_index_reg(binding_slot, index_reg, element_type, memory_class)
    }

    fn emit_memory_address_immediate(
        &self,
        binding_slot: u32,
        byte_offset: u64,
        memory_class: MemoryClass,
    ) -> Result<MemAddress, EmitError> {
        match memory_class {
            MemoryClass::Global | MemoryClass::Constant | MemoryClass::Uniform => {
                let global_ptr = *self.slot_to_ptr.get(&binding_slot).ok_or_else(|| {
                    EmitError::InvalidBinding {
                        slot: binding_slot,
                        reason: "global pointer not preloaded".into(),
                    }
                })?;
                Ok(MemAddress {
                    space: "global",
                    operand: MemOperand::RegOffset(global_ptr, byte_offset),
                })
            }
            MemoryClass::Shared => {
                if !self.slot_to_shared_symbol.contains_key(&binding_slot) {
                    return Err(EmitError::InvalidBinding {
                        slot: binding_slot,
                        reason: "shared symbol not allocated".into(),
                    });
                }
                Ok(MemAddress {
                    space: "shared",
                    operand: MemOperand::SharedSlotOffset(binding_slot, byte_offset),
                })
            }
            MemoryClass::Scratch => Err(EmitError::InvalidBinding {
                slot: binding_slot,
                reason: "scratch bindings must be resolved before PTX emission".into(),
            }),
        }
    }

    fn immediate_byte_offset(
        &self,
        index_op_id: u32,
        element_type: &DataType,
    ) -> Result<Option<u64>, EmitError> {
        let Some(index) = self.u32_literals.get(&index_op_id).copied() else {
            return Ok(None);
        };
        let stride = element_type
            .size_bytes()
            .ok_or_else(|| EmitError::UnsupportedDataType(format!("{element_type:?}")))?;
        let Some(byte_offset) = u64::from(index).checked_mul(stride as u64) else {
            return Ok(None);
        };
        if byte_offset <= i32::MAX as u64 {
            Ok(Some(byte_offset))
        } else {
            Ok(None)
        }
    }

    pub(super) fn emit_memory_address_from_index_reg(
        &mut self,
        binding_slot: u32,
        index_reg: Reg,
        element_type: &DataType,
        memory_class: MemoryClass,
    ) -> Result<MemAddress, EmitError> {
        match memory_class {
            MemoryClass::Global => {
                let global_ptr = *self.slot_to_ptr.get(&binding_slot).ok_or_else(|| {
                    EmitError::InvalidBinding {
                        slot: binding_slot,
                        reason: "global pointer not preloaded".into(),
                    }
                })?;
                let safe_index = self.clamp_index_to_buffer_length(binding_slot, index_reg);
                let byte_offset = self.emit_byte_offset(safe_index, element_type)?;
                let reg = self.alloc(PtxType::U64);
                let _ = writeln!(
                    self.text,
                    "    add.u64    {reg}, {global_ptr}, {byte_offset};"
                );
                Ok(MemAddress {
                    space: "global",
                    operand: MemOperand::Reg(reg),
                })
            }
            MemoryClass::Constant | MemoryClass::Uniform => {
                let global_ptr = *self.slot_to_ptr.get(&binding_slot).ok_or_else(|| {
                    EmitError::InvalidBinding {
                        slot: binding_slot,
                        reason: "constant/uniform pointer not preloaded".into(),
                    }
                })?;
                let safe_index = self.clamp_index_to_buffer_length(binding_slot, index_reg);
                let byte_offset = self.emit_byte_offset(safe_index, element_type)?;
                let reg = self.alloc(PtxType::U64);
                let _ = writeln!(
                    self.text,
                    "    add.u64    {reg}, {global_ptr}, {byte_offset};"
                );
                Ok(MemAddress {
                    space: "global",
                    operand: MemOperand::Reg(reg),
                })
            }
            MemoryClass::Shared => {
                let symbol = self
                    .slot_to_shared_symbol
                    .get(&binding_slot)
                    .cloned()
                    .ok_or_else(|| EmitError::InvalidBinding {
                        slot: binding_slot,
                        reason: "shared symbol not allocated".into(),
                    })?;
                let byte_offset = self.emit_shared_byte_offset(index_reg, element_type)?;
                let base = self.alloc(PtxType::U32);
                let addr = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    mov.u32    {base}, {symbol};");
                let _ = writeln!(self.text, "    add.u32    {addr}, {base}, {byte_offset};");
                Ok(MemAddress {
                    space: "shared",
                    operand: MemOperand::Reg(addr),
                })
            }
            MemoryClass::Scratch => Err(EmitError::InvalidBinding {
                slot: binding_slot,
                reason: "scratch bindings must be resolved before PTX emission".into(),
            }),
        }
    }

    /// Clamp a runtime index register so the resulting address stays
    /// inside the buffer. PTX has no built-in bounds checking; without
    /// this, speculative loads emitted by `Expr::select` arms can fault
    /// with `CUDA_ERROR_ILLEGAL_ADDRESS`. WGSL/naga does an equivalent
    /// clamp via its bounds-check policy  -  this matches the contract
    /// across backends.
    ///
    /// Lowered as: `safe = (idx < len) ? idx : 0`. When `len == 0` the
    /// dispatcher rejects the launch upstream, so the `0` fallback
    /// always points at a valid byte.
    fn clamp_index_to_buffer_length(&mut self, binding_slot: u32, raw_idx: Reg) -> Reg {
        let len_reg = self.ensure_buffer_length_reg(binding_slot);
        let in_bounds = self.alloc(PtxType::Bool);
        let safe_idx = self.alloc(PtxType::U32);
        let zero = self.alloc(PtxType::U32);
        let _ = writeln!(self.text, "    mov.u32    {zero}, 0;");
        let _ = writeln!(
            self.text,
            "    setp.lt.u32    {in_bounds}, {raw_idx}, {len_reg};"
        );
        let _ = writeln!(
            self.text,
            "    selp.u32    {safe_idx}, {raw_idx}, {zero}, {in_bounds};"
        );
        safe_idx
    }

    pub(super) fn emit_index_in_bounds_pred(
        &mut self,
        binding_slot: u32,
        index_op_id: u32,
    ) -> Result<Reg, EmitError> {
        let raw_idx = self.lookup_operand(index_op_id)?;
        Ok(self.emit_index_reg_in_bounds_pred(binding_slot, raw_idx))
    }

    pub(super) fn emit_index_reg_in_bounds_pred(&mut self, binding_slot: u32, raw_idx: Reg) -> Reg {
        let len_reg = self.ensure_buffer_length_reg(binding_slot);
        let in_bounds = self.alloc(PtxType::Bool);
        let _ = writeln!(
            self.text,
            "    setp.lt.u32    {in_bounds}, {raw_idx}, {len_reg};"
        );
        in_bounds
    }

    fn ensure_buffer_length_reg(&mut self, binding_slot: u32) -> Reg {
        if let Some(&reg) = self.slot_to_length_reg.get(&binding_slot) {
            return reg;
        }
        let reg = self.alloc(PtxType::U32);
        // All valid slots must be registered by `preload_bindings`, which
        // performs checked arithmetic and returns Err on overflow. Reaching
        // this branch at all indicates a caller bug: a slot that was never
        // preloaded is being used at emit time.
        //
        // Law 10: we must NEVER silently emit a plausible-looking load when
        // the offset computation would overflow. We emit `trap;` first so the
        // thread terminates loudly rather than reading a garbage length
        // register and producing silently wrong bounds-check predicates.
        match binding_slot.checked_mul(4).and_then(|v| v.checked_add(4)) {
            None => {
                // Overflow: the slot cannot produce a valid params-buffer
                // offset. Emit a hard trap so the kernel fails closed instead
                // of reading a u32::MAX-offset address and silently using the
                // garbage as a bounds-check length.
                let _ = writeln!(
                    self.text,
                    "    // BUG: slot={binding_slot} byte offset overflows u32; \
                     preload_bindings should have rejected this slot — killing thread"
                );
                let _ = writeln!(self.text, "    trap;");
            }
            Some(byte_offset) => {
                let _ = writeln!(
                    self.text,
                    "    ld.global.ca.u32    {reg}, [%rd0 + {byte_offset}];"
                );
            }
        }
        self.slot_to_length_reg.insert(binding_slot, reg);
        reg
    }

    fn emit_byte_offset(
        &mut self,
        index_reg: Reg,
        element_type: &DataType,
    ) -> Result<Reg, EmitError> {
        let stride = element_type
            .size_bytes()
            .ok_or_else(|| EmitError::UnsupportedDataType(format!("{element_type:?}")))?;
        let byte_offset = self.alloc(PtxType::U64);
        let _ = writeln!(
            self.text,
            "    mul.wide.u32    {byte_offset}, {index_reg}, {stride};"
        );
        Ok(byte_offset)
    }

    fn emit_shared_byte_offset(
        &mut self,
        index_reg: Reg,
        element_type: &DataType,
    ) -> Result<Reg, EmitError> {
        let stride = element_type
            .size_bytes()
            .ok_or_else(|| EmitError::UnsupportedDataType(format!("{element_type:?}")))?;
        let byte_offset = self.alloc(PtxType::U32);
        let _ = writeln!(
            self.text,
            "    mul.lo.u32    {byte_offset}, {index_reg}, {stride};"
        );
        Ok(byte_offset)
    }

    pub(super) fn write_mem_operand(&mut self, operand: MemOperand) -> Result<(), EmitError> {
        match operand {
            MemOperand::Reg(reg) | MemOperand::RegOffset(reg, 0) => {
                let _ = write!(self.text, "[{reg}]");
            }
            MemOperand::RegOffset(reg, offset) => {
                let _ = write!(self.text, "[{reg}+{offset}]");
            }
            MemOperand::SharedSlotOffset(slot, 0) => {
                let symbol = self.slot_to_shared_symbol.get(&slot).ok_or_else(|| {
                    EmitError::InvalidBinding {
                        slot,
                        reason: "shared symbol not allocated".into(),
                    }
                })?;
                let _ = write!(self.text, "[{symbol}]");
            }
            MemOperand::SharedSlotOffset(slot, offset) => {
                let symbol = self.slot_to_shared_symbol.get(&slot).ok_or_else(|| {
                    EmitError::InvalidBinding {
                        slot,
                        reason: "shared symbol not allocated".into(),
                    }
                })?;
                let _ = write!(self.text, "[{symbol}+{offset}]");
            }
        }
        Ok(())
    }

    pub(super) fn require_u32_slot(
        &self,
        slot: u32,
        context: &str,
    ) -> Result<(DataType, MemoryClass), EmitError> {
        let binding = self.binding_for_slot(slot)?;
        if binding.element_type != DataType::U32 {
            return Err(EmitError::InvalidBinding {
                slot,
                reason: format!("{context} must be a U32 binding"),
            });
        }
        Ok((binding.element_type.clone(), binding.memory_class))
    }
}
