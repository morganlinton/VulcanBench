//! Trap + atomic op emitters. Owns the trap-sidecar exchange protocol
//! (compare-exchange on word 0 to claim the trap, then atomic-exchange
//! on words 1-3 to write address/tag/lane) and the fetch-NAND
//! compare-exchange retry loop.

use naga::{BinaryOperator, Block, Expression, LocalVariable, Span, Statement, UnaryOperator};
use vyre_foundation::ir::AtomicOp;
use vyre_lower::{KernelOp, TRAP_SIDECAR_NAME};

use super::BodyBuilder;
use crate::EmitError;

impl BodyBuilder<'_> {
    pub(super) fn emit_atomic(
        &mut self,
        op: &KernelOp,
        atomic_op: AtomicOp,
    ) -> Result<(), EmitError> {
        if matches!(atomic_op, AtomicOp::FetchNand) {
            return self.emit_fetch_nand_atomic(op);
        }
        let compare_exchange = matches!(
            atomic_op,
            AtomicOp::CompareExchange | AtomicOp::CompareExchangeWeak
        );
        let fun = match atomic_op {
            AtomicOp::Add => naga::AtomicFunction::Add,
            AtomicOp::And => naga::AtomicFunction::And,
            AtomicOp::Or => naga::AtomicFunction::InclusiveOr,
            AtomicOp::Xor => naga::AtomicFunction::ExclusiveOr,
            AtomicOp::Min => naga::AtomicFunction::Min,
            AtomicOp::Max | AtomicOp::LruUpdate => naga::AtomicFunction::Max,
            AtomicOp::Exchange => naga::AtomicFunction::Exchange { compare: None },
            AtomicOp::CompareExchange | AtomicOp::CompareExchangeWeak => {
                naga::AtomicFunction::Exchange {
                    compare: Some(self.value_operand(op, 2)?),
                }
            }
            AtomicOp::Opaque(id) => {
                return Err(EmitError::InvalidDescriptor(format!(
                    "opaque atomic op id {:#010x} reached Naga descriptor emission. Fix: lower the extension to concrete KernelDescriptor ops or register a Naga descriptor atomic emitter.",
                    id.0
                )));
            }
            other => {
                return Err(EmitError::InvalidDescriptor(format!(
                    "atomic op `{other:?}` has no Naga descriptor mapping. Fix: add the concrete atomic lowering before this op reaches Naga emission."
                )));
            }
        };
        let pointer = self.binding_element_pointer(op, 0, 1)?;
        let value = self.value_operand(op, if compare_exchange { 3 } else { 2 })?;
        let result = op.result.map(|_| {
            self.function.expressions.append(
                Expression::AtomicResult {
                    ty: if compare_exchange {
                        self.types.atomic_compare_exchange_u32_ty
                    } else {
                        self.types.u32_ty
                    },
                    comparison: compare_exchange,
                },
                Span::UNDEFINED,
            )
        });
        self.function.body.push(
            Statement::Atomic {
                pointer,
                fun,
                value,
                result,
            },
            Span::UNDEFINED,
        );
        if let Some(result) = result {
            if compare_exchange {
                let old_value = self.append_expr(Expression::AccessIndex {
                    base: result,
                    index: 0,
                });
                self.bind_result_typed(op, old_value, self.types.u32_ty)
            } else {
                self.bind_result_typed(op, result, self.types.u32_ty)
            }
        } else {
            Ok(())
        }
    }

    pub(super) fn emit_trap(
        &mut self,
        op: &KernelOp,
        tag: &vyre_lower::descriptor::Name,
    ) -> Result<(), EmitError> {
        let tag_code = *self.trap_tag_codes.get(tag).ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "Trap tag `{tag}` has no descriptor sidecar code. Fix: collect trap tags before Naga descriptor emission."
            ))
        })?;
        let address_value = self.value_operand(op, 0)?;
        let flag_pointer = self.trap_sidecar_pointer(0)?;
        let expected_zero = self.literal_u32(0);
        let flag_one = self.literal_u32(1);
        let result = self.function.expressions.append(
            Expression::AtomicResult {
                ty: self.types.atomic_compare_exchange_u32_ty,
                comparison: true,
            },
            Span::UNDEFINED,
        );
        self.function.body.push(
            Statement::Atomic {
                pointer: flag_pointer,
                fun: naga::AtomicFunction::Exchange {
                    compare: Some(expected_zero),
                },
                value: flag_one,
                result: Some(result),
            },
            Span::UNDEFINED,
        );

        let previous_flag = self.append_expr(Expression::AccessIndex {
            base: result,
            index: 0,
        });
        let zero = self.literal_u32(0);
        let first_trap = self.append_expr(Expression::Binary {
            op: BinaryOperator::Equal,
            left: previous_flag,
            right: zero,
        });

        let outer = std::mem::take(&mut self.function.body);
        self.trap_sidecar_atomic_exchange(1, address_value)?;
        let tag_value = self.literal_u32(tag_code);
        self.trap_sidecar_atomic_exchange(2, tag_value)?;
        let lane = self.global_invocation_axis(0);
        self.trap_sidecar_atomic_exchange(3, lane)?;
        let accept = std::mem::replace(&mut self.function.body, outer);
        self.function.body.push(
            Statement::If {
                condition: first_trap,
                accept,
                reject: Block::new(),
            },
            Span::UNDEFINED,
        );
        self.function
            .body
            .push(Statement::Return { value: None }, Span::UNDEFINED);
        Ok(())
    }

    pub(super) fn trap_sidecar_pointer(
        &mut self,
        word: u32,
    ) -> Result<naga::Handle<Expression>, EmitError> {
        let slot = self.trap_sidecar_slot.ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "Trap op requires descriptor sidecar `{TRAP_SIDECAR_NAME}`. Fix: lower through vyre-lower so the sidecar binding is present."
            ))
        })?;
        let index = self.literal_u32(word);
        self.binding_element_pointer_by_slot(slot, index)
    }

    pub(super) fn trap_sidecar_atomic_exchange(
        &mut self,
        word: u32,
        value: naga::Handle<Expression>,
    ) -> Result<(), EmitError> {
        let pointer = self.trap_sidecar_pointer(word)?;
        let result = self.function.expressions.append(
            Expression::AtomicResult {
                ty: self.types.u32_ty,
                comparison: false,
            },
            Span::UNDEFINED,
        );
        self.function.body.push(
            Statement::Atomic {
                pointer,
                fun: naga::AtomicFunction::Exchange { compare: None },
                value,
                result: Some(result),
            },
            Span::UNDEFINED,
        );
        Ok(())
    }

    pub(super) fn emit_fetch_nand_atomic(&mut self, op: &KernelOp) -> Result<(), EmitError> {
        let pointer = self.binding_element_pointer(op, 0, 1)?;
        let value = self.value_operand(op, 2)?;

        let old_local = self.function.local_variables.append(
            LocalVariable {
                name: Some("__vyre_fetch_nand_old".to_owned()),
                ty: self.types.u32_ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        let previous_local = self.function.local_variables.append(
            LocalVariable {
                name: Some("__vyre_fetch_nand_previous".to_owned()),
                ty: self.types.u32_ty,
                init: None,
            },
            Span::UNDEFINED,
        );

        let old_result = self.function.expressions.append(
            Expression::AtomicResult {
                ty: self.types.u32_ty,
                comparison: false,
            },
            Span::UNDEFINED,
        );
        let zero = self.literal_u32(0);
        self.function.body.push(
            Statement::Atomic {
                pointer,
                fun: naga::AtomicFunction::Add,
                value: zero,
                result: Some(old_result),
            },
            Span::UNDEFINED,
        );
        let old_pointer = self.append_expr(Expression::LocalVariable(old_local));
        self.function.body.push(
            Statement::Store {
                pointer: old_pointer,
                value: old_result,
            },
            Span::UNDEFINED,
        );

        let outer = std::mem::take(&mut self.function.body);
        let old_pointer = self.append_expr(Expression::LocalVariable(old_local));
        let old = self.append_expr(Expression::Load {
            pointer: old_pointer,
        });
        let masked = self.append_expr(Expression::Binary {
            op: BinaryOperator::And,
            left: old,
            right: value,
        });
        let next = self.append_expr(Expression::Unary {
            op: UnaryOperator::BitwiseNot,
            expr: masked,
        });
        let cx_result = self.function.expressions.append(
            Expression::AtomicResult {
                ty: self.types.atomic_compare_exchange_u32_ty,
                comparison: true,
            },
            Span::UNDEFINED,
        );
        self.function.body.push(
            Statement::Atomic {
                pointer,
                fun: naga::AtomicFunction::Exchange { compare: Some(old) },
                value: next,
                result: Some(cx_result),
            },
            Span::UNDEFINED,
        );
        let previous = self.append_expr(Expression::AccessIndex {
            base: cx_result,
            index: 0,
        });
        let exchanged = self.append_expr(Expression::AccessIndex {
            base: cx_result,
            index: 1,
        });
        let previous_pointer = self.append_expr(Expression::LocalVariable(previous_local));
        self.function.body.push(
            Statement::Store {
                pointer: previous_pointer,
                value: previous,
            },
            Span::UNDEFINED,
        );
        let retry_pointer = self.append_expr(Expression::LocalVariable(old_local));
        self.function.body.push(
            Statement::Store {
                pointer: retry_pointer,
                value: previous,
            },
            Span::UNDEFINED,
        );
        let mut accept = Block::new();
        accept.push(Statement::Break, Span::UNDEFINED);
        self.function.body.push(
            Statement::If {
                condition: exchanged,
                accept,
                reject: Block::new(),
            },
            Span::UNDEFINED,
        );
        let loop_body = std::mem::replace(&mut self.function.body, outer);
        self.function.body.push(
            Statement::Loop {
                body: loop_body,
                continuing: Block::new(),
                break_if: None,
            },
            Span::UNDEFINED,
        );

        if op.result.is_some() {
            let previous_pointer = self.append_expr(Expression::LocalVariable(previous_local));
            let previous = self.append_expr(Expression::Load {
                pointer: previous_pointer,
            });
            self.bind_result_typed(op, previous, self.types.u32_ty)?;
        }
        Ok(())
    }
}
