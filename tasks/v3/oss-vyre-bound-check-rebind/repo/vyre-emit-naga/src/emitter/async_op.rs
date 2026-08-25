//! AsyncLoad / AsyncStore op emitters. Both lower to a counted u32
//! word-copy loop that respects source/destination buffer lengths.

use naga::{Expression, Span, Statement};
use vyre_lower::KernelOp;

use super::BodyBuilder;
use crate::EmitError;

impl BodyBuilder<'_> {
    pub(super) fn emit_async_load(&mut self, op: &KernelOp) -> Result<(), EmitError> {
        let source_slot = self.slot_operand(op, 0)?;
        let destination_slot = self.slot_operand(op, 1)?;
        self.require_u32_slot(source_slot, "AsyncLoad source")?;
        self.require_u32_slot(destination_slot, "AsyncLoad destination")?;

        let offset = self.value_operand(op, 2)?;
        let size = self.value_operand(op, 3)?;
        let four = self.literal_u32(4);
        let source_start = self.div_u32(offset, four);
        let requested_words = self.byte_size_to_words(size);
        let destination_len = self.buffer_len_expr(destination_slot)?;
        let source_len = self.buffer_len_expr(source_slot)?;
        let copy_words = self.min_u32(requested_words, destination_len);

        self.emit_counted_u32_loop("async_load_word", copy_words, |this, index| {
            let source_index = this.add_u32(source_start, index);
            let in_bounds = this.lt_u32(source_index, source_len);
            let source_value = {
                let pointer = this.binding_element_pointer_by_slot(source_slot, source_index)?;
                this.append_expr(Expression::Load { pointer })
            };
            let zero = this.literal_u32(0);
            let value = this.append_expr(Expression::Select {
                condition: in_bounds,
                accept: source_value,
                reject: zero,
            });
            let destination_pointer =
                this.binding_element_pointer_by_slot(destination_slot, index)?;
            this.function.body.push(
                Statement::Store {
                    pointer: destination_pointer,
                    value,
                },
                Span::UNDEFINED,
            );
            Ok(())
        })
    }

    pub(super) fn emit_async_store(&mut self, op: &KernelOp) -> Result<(), EmitError> {
        let source_slot = self.slot_operand(op, 0)?;
        let destination_slot = self.slot_operand(op, 1)?;
        self.require_u32_slot(source_slot, "AsyncStore source")?;
        self.require_u32_slot(destination_slot, "AsyncStore destination")?;

        let offset = self.value_operand(op, 2)?;
        let size = self.value_operand(op, 3)?;
        let four = self.literal_u32(4);
        let destination_start = self.div_u32(offset, four);
        let requested_words = self.byte_size_to_words(size);
        let source_len = self.buffer_len_expr(source_slot)?;
        let destination_len = self.buffer_len_expr(destination_slot)?;
        let destination_remaining = {
            let has_space = self.lt_u32(destination_start, destination_len);
            let remaining = self.sub_u32(destination_len, destination_start);
            let zero = self.literal_u32(0);
            self.append_expr(Expression::Select {
                condition: has_space,
                accept: remaining,
                reject: zero,
            })
        };
        let request_or_source = self.min_u32(requested_words, source_len);
        let copy_words = self.min_u32(request_or_source, destination_remaining);

        self.emit_counted_u32_loop("async_store_word", copy_words, |this, index| {
            let destination_index = this.add_u32(destination_start, index);
            let source_pointer = this.binding_element_pointer_by_slot(source_slot, index)?;
            let value = this.append_expr(Expression::Load {
                pointer: source_pointer,
            });
            let destination_pointer =
                this.binding_element_pointer_by_slot(destination_slot, destination_index)?;
            this.function.body.push(
                Statement::Store {
                    pointer: destination_pointer,
                    value,
                },
                Span::UNDEFINED,
            );
            Ok(())
        })
    }
}
