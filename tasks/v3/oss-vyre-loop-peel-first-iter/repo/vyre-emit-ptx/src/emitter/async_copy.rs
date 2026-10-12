use std::fmt::Write as _;

use vyre_foundation::ir::DataType;
use vyre_lower::descriptor::Name;
use vyre_lower::MemoryClass;

use super::memory::AsyncCopyDirection;
use super::BodyCtx;
use crate::reg::{PtxType, Reg};
use crate::EmitError;

impl BodyCtx<'_> {
    pub(super) fn emit_u32_const(&mut self, value: u32) -> Reg {
        let reg = self.alloc(PtxType::U32);
        let _ = writeln!(self.text, "    mov.u32    {reg}, {value};");
        reg
    }

    fn emit_words_from_byte_size(&mut self, size_reg: Reg) -> Reg {
        let rounded = self.alloc(PtxType::U32);
        let _ = writeln!(self.text, "    add.u32    {rounded}, {size_reg}, 3;");
        let words = self.alloc(PtxType::U32);
        let _ = writeln!(self.text, "    shr.u32    {words}, {rounded}, 2;");
        words
    }

    fn emit_word_offset_from_byte_offset(&mut self, offset_reg: Reg) -> Reg {
        let words = self.alloc(PtxType::U32);
        let _ = writeln!(self.text, "    shr.u32    {words}, {offset_reg}, 2;");
        words
    }

    fn emit_min_u32(&mut self, left: Reg, right: Reg) -> Reg {
        let pred = self.alloc(PtxType::Bool);
        let out = self.alloc(PtxType::U32);
        let _ = writeln!(self.text, "    setp.lt.u32    {pred}, {left}, {right};");
        let _ = writeln!(self.text, "    selp.u32    {out}, {left}, {right}, {pred};");
        out
    }

    fn emit_binding_len_or_max(&mut self, slot: u32) -> Result<Reg, EmitError> {
        let count = self
            .binding_for_slot(slot)?
            .element_count
            .unwrap_or(u32::MAX);
        Ok(self.emit_u32_const(count))
    }

    pub(super) fn emit_async_copy_loop(
        &mut self,
        tag: &str,
        source_slot: u32,
        destination_slot: u32,
        offset_id: u32,
        size_id: u32,
        direction: AsyncCopyDirection,
    ) -> Result<(), EmitError> {
        let source_binding = self.binding_for_slot(source_slot)?;
        let source_type = source_binding.element_type.clone();
        let source_class = source_binding.memory_class;
        let destination_binding = self.binding_for_slot(destination_slot)?;
        let destination_type = destination_binding.element_type.clone();
        let destination_class = destination_binding.memory_class;
        let offset_reg = self.lookup_operand(offset_id)?;
        let size_reg = self.lookup_operand(size_id)?;
        let requested_words = self.emit_words_from_byte_size(size_reg);
        let source_len = self.emit_binding_len_or_max(source_slot)?;
        let destination_len = self.emit_binding_len_or_max(destination_slot)?;
        let offset_words = self.emit_word_offset_from_byte_offset(offset_reg);
        let zero = self.emit_u32_const(0);
        let copy_words = match direction {
            AsyncCopyDirection::Load => self.emit_min_u32(requested_words, destination_len),
            AsyncCopyDirection::Store => {
                let has_space = self.alloc(PtxType::Bool);
                let remaining = self.alloc(PtxType::U32);
                let destination_remaining = self.alloc(PtxType::U32);
                let _ = writeln!(
                    self.text,
                    "    setp.lt.u32    {has_space}, {offset_words}, {destination_len};"
                );
                let _ = writeln!(
                    self.text,
                    "    sub.u32    {remaining}, {destination_len}, {offset_words};"
                );
                let _ = writeln!(
                    self.text,
                    "    selp.u32    {destination_remaining}, {remaining}, {zero}, {has_space};"
                );
                let request_or_source = self.emit_min_u32(requested_words, source_len);
                self.emit_min_u32(request_or_source, destination_remaining)
            }
        };

        let loop_index = self.emit_u32_const(0);
        let loop_label = self.alloc_label("async_copy");
        let done_label = self.alloc_label("async_done");
        let _ = writeln!(self.text, "{loop_label}:");
        let done_pred = self.alloc(PtxType::Bool);
        let _ = writeln!(
            self.text,
            "    setp.ge.u32    {done_pred}, {loop_index}, {copy_words};"
        );
        let _ = writeln!(self.text, "    @{done_pred} bra    {done_label};");

        let (source_index, destination_index) = match direction {
            AsyncCopyDirection::Load => {
                let source_index = self.alloc(PtxType::U32);
                let _ = writeln!(
                    self.text,
                    "    add.u32    {source_index}, {offset_words}, {loop_index};"
                );
                (source_index, loop_index)
            }
            AsyncCopyDirection::Store => {
                let destination_index = self.alloc(PtxType::U32);
                let _ = writeln!(
                    self.text,
                    "    add.u32    {destination_index}, {offset_words}, {loop_index};"
                );
                (loop_index, destination_index)
            }
        };

        let elem_ty = PtxType::from_dtype(&source_type)?;
        let value = self.alloc(elem_ty);
        let in_bounds = self.alloc(PtxType::Bool);
        let source_addr = self.emit_memory_address_from_index_reg(
            source_slot,
            source_index,
            &source_type,
            source_class,
        )?;
        let _ = writeln!(
            self.text,
            "    setp.lt.u32    {in_bounds}, {source_index}, {source_len};"
        );
        let load_space = self.load_space_for(source_slot, source_class);
        let _ = write!(
            self.text,
            "    @{in_bounds} ld.{}.{}    {value}, ",
            load_space,
            elem_ty.ptx_type_str(),
        );
        self.write_mem_operand(source_addr.operand)?;
        self.text.push_str(";\n");
        let zero_text = match source_type {
            DataType::F32 => "0F".to_string(),
            DataType::U32 => "0".to_string(),
            DataType::I32 => "0".to_string(),
            DataType::Bool => "0".to_string(),
            _ => return Err(EmitError::UnsupportedDataType(format!(
                "Async copy fallback does not support {source_type:?}. Fix: add zero literal for this type or use U32 staging."
            ))),
        };
        let _ = writeln!(
            self.text,
            "    @!{in_bounds} mov.{}    {value}, {zero_text};",
            elem_ty.ptx_type_str()
        );

        let destination_addr = self.emit_memory_address_from_index_reg(
            destination_slot,
            destination_index,
            &destination_type,
            destination_class,
        )?;
        let dst_elem_ty = PtxType::from_dtype(&destination_type)?;
        let _ = write!(
            self.text,
            "    st.{}.{}    ",
            destination_addr.space,
            dst_elem_ty.ptx_type_str()
        );
        self.write_mem_operand(destination_addr.operand)?;
        let _ = writeln!(self.text, ", {value};");
        let _ = writeln!(self.text, "    add.u32    {loop_index}, {loop_index}, 1;");
        let _ = writeln!(self.text, "    bra    {loop_label};");
        let _ = writeln!(self.text, "{done_label}:");
        let _ = writeln!(
            self.text,
            "    // async_copy tag={tag} lowered as bounded synchronous copy"
        );
        Ok(())
    }

    pub(super) fn emit_cp_async_load_loop(
        &mut self,
        tag: &Name,
        source_slot: u32,
        destination_slot: u32,
        offset_id: u32,
        size_id: u32,
    ) -> Result<bool, EmitError> {
        if !self.options.target.supports_async_copy() {
            return Ok(false);
        }
        let (source_type, source_class) =
            match self.require_u32_slot(source_slot, "cp.async source") {
                Ok(v) => v,
                Err(_) => return Ok(false),
            };
        let (destination_type, destination_class) =
            match self.require_u32_slot(destination_slot, "cp.async destination") {
                Ok(v) => v,
                Err(_) => return Ok(false),
            };
        if source_class != MemoryClass::Global || destination_class != MemoryClass::Shared {
            return Ok(false);
        }

        let offset_reg = self.lookup_operand(offset_id)?;
        let size_reg = self.lookup_operand(size_id)?;
        let requested_words = self.emit_words_from_byte_size(size_reg);
        let destination_len = self.emit_binding_len_or_max(destination_slot)?;
        let source_len = self.emit_binding_len_or_max(source_slot)?;
        let offset_words = self.emit_word_offset_from_byte_offset(offset_reg);
        let copy_words = self.emit_min_u32(requested_words, destination_len);
        let zero = self.emit_u32_const(0);
        let loop_index = self.emit_u32_const(0);
        let loop_label = self.alloc_label("cp_async");
        let done_label = self.alloc_label("cp_async_done");

        let _ = writeln!(
            self.text,
            "    // cp.async_load tag={tag} src=slot{source_slot} dst=slot{destination_slot}"
        );
        let _ = writeln!(self.text, "{loop_label}:");
        let done_pred = self.alloc(PtxType::Bool);
        let _ = writeln!(
            self.text,
            "    setp.ge.u32    {done_pred}, {loop_index}, {copy_words};"
        );
        let _ = writeln!(self.text, "    @{done_pred} bra    {done_label};");

        let source_index = self.alloc(PtxType::U32);
        let _ = writeln!(
            self.text,
            "    add.u32    {source_index}, {offset_words}, {loop_index};"
        );
        let destination_index = loop_index;
        let source_addr = self.emit_memory_address_from_index_reg(
            source_slot,
            source_index,
            &source_type,
            source_class,
        )?;
        let destination_addr = self.emit_memory_address_from_index_reg(
            destination_slot,
            destination_index,
            &destination_type,
            destination_class,
        )?;
        let in_bounds = self.alloc(PtxType::Bool);
        let _ = writeln!(
            self.text,
            "    setp.lt.u32    {in_bounds}, {source_index}, {source_len};"
        );
        let _ = write!(self.text, "    @{in_bounds} cp.async.ca.shared.global    ");
        self.write_mem_operand(destination_addr.operand)?;
        self.text.push_str(", ");
        self.write_mem_operand(source_addr.operand)?;
        self.text.push_str(", 4;\n");
        let _ = write!(self.text, "    @!{in_bounds} st.shared.u32    ");
        self.write_mem_operand(destination_addr.operand)?;
        let _ = writeln!(self.text, ", {zero};");
        let _ = writeln!(self.text, "    add.u32    {loop_index}, {loop_index}, 1;");
        let _ = writeln!(self.text, "    bra    {loop_label};");
        let _ = writeln!(self.text, "{done_label}:");
        let _ = writeln!(self.text, "    cp.async.commit_group;");
        self.pending_cp_async_tags.insert(tag.clone());
        Ok(true)
    }

    pub(super) fn emit_cp_async_wait_for_tag(&mut self, tag: &str) -> bool {
        if !self.pending_cp_async_tags.remove(tag) {
            return false;
        }
        let _ = writeln!(self.text, "    cp.async.wait_group 0;");
        let _ = writeln!(self.text, "    membar.cta;");
        true
    }
}
