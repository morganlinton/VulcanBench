//! Loop emission for `vyre-emit-naga`: `LoopIndex`, `StructuredForLoop`,
//! the loop guard / continuing / break-condition helpers, the
//! counted-u32 loop helper, and the Q7 loop-carried-value carrier
//! machinery.
//!
//! Split from `mod.rs` so the loop substrate (`emit_structured_for_loop`
//! plus the carrier helpers) lives in one auditable file. All methods
//! are `impl<'a> BodyBuilder<'a>` extensions; nothing here is public  -
//! callers go through `emit_op` which dispatches into these methods.

use naga::{
    BinaryOperator, Block, Expression, Literal, LocalVariable, ScalarKind, Span, Statement, Type,
};
use rustc_hash::FxHashSet;
use vyre_lower::{KernelBody, KernelOp};

use super::BodyBuilder;
use crate::EmitError;

impl<'a> BodyBuilder<'a> {
    pub(super) fn emit_loop_index(
        &mut self,
        op: &KernelOp,
        loop_var: &vyre_lower::descriptor::Name,
    ) -> Result<(), EmitError> {
        let local = *self.loop_locals.get(loop_var).ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "loop index `{loop_var}` was emitted outside its StructuredForLoop"
            ))
        })?;
        let ty = *self.loop_types.get(loop_var).ok_or_else(|| {
            EmitError::InvalidDescriptor(format!("loop index `{loop_var}` has no tracked type"))
        })?;
        let pointer = self.append_expr(Expression::LocalVariable(local));
        let value = self.append_expr(Expression::Load { pointer });
        self.bind_result_typed(op, value, ty)
    }

    pub(super) fn emit_structured_for_loop(
        &mut self,
        body: &KernelBody,
        op: &KernelOp,
        loop_var: &vyre_lower::descriptor::Name,
    ) -> Result<(), EmitError> {
        // Q7: identify loop-carried result ids  -  values produced
        // inside the loop child body that the parent body references
        // AFTER this loop op. Each becomes a function-scope
        // `LocalVariable` carrier so post-loop reads load the final
        // value instead of dangling on an SSA handle that is out of
        // scope.
        let prior_carriers = self.snapshot_loop_carriers();
        let child_idx = op.operands.get(2).copied().unwrap_or(u32::MAX);
        let new_targets = if let Some(child) = body.child_bodies.get(child_idx as usize) {
            self.collect_loop_carried_ids(body, op, child)
        } else {
            FxHashSet::default()
        };
        // Pre-loop init for any carrier whose id was bound before the
        // loop in the parent's SSA scope: we need to seed the local
        // with that value so iteration 0 reads the pre-loop initialiser.
        // Pre-size from `new_targets`: at most one (id, handle) per
        // tracked carrier, so we never resize during the seed scan.
        let mut pre_init: Vec<(u32, naga::Handle<Expression>)> =
            Vec::with_capacity(new_targets.len());
        for id in &new_targets {
            self.loop_carrier_targets.insert(*id);
            // Resolve the pre-loop value of `id` in the CURRENT (parent) scope.
            // Critical: the cached handle in `self.values` may be a `Load`
            // whose `Statement::Emit` lives inside an outer loop's body  -  out
            // of scope at this nested-loop's pre-init Store site.  The helper
            // synthesizes a fresh `LocalVariable + Load` here when `id` is a
            // carrier so the resulting handle is in scope where pre_init Stores.
            if let Some(handle) = self.value_handle_for_id(*id) {
                pre_init.push((*id, handle));
            }
        }

        let from = self.value_operand(op, 0)?;
        let mut to = self.value_operand(op, 1)?;
        let index_ty = self.value_type_operand(op, 0)?;
        // Soundness: when from/to disagree on scalar type (a common
        // shape when one bound is a literal and the other is a Load
        // result of a different scalar width), insert a Naga `As`
        // cast on `to` so both operands match `index_ty`. Previously
        // we rejected the descriptor outright; that blocked every
        // megakernel (memcpy_body, batch_fence_body, ...) whose
        // protocol-buffer-loaded bound was wider/narrower than the
        // literal start. Casting is the right semantic  -  both ends
        // are integer scalars, and the loop arithmetic operates on
        // index_ty afterwards.
        let to_ty = self.value_type_operand(op, 1)?;
        if to_ty != index_ty {
            // Map the loop's index type back to its scalar kind so we
            // can build a Naga `Expression::As` cast on `to`. The
            // TypesCache only holds the four handles vyre uses for
            // index variables (u32/i32/f32/bool); anything else is a
            // descriptor invariant violation upstream.
            let (kind, width) = if index_ty == self.types.u32_ty {
                (ScalarKind::Uint, 4u8)
            } else if index_ty == self.types.i32_ty {
                (ScalarKind::Sint, 4u8)
            } else if index_ty == self.types.f32_ty {
                (ScalarKind::Float, 4u8)
            } else {
                return Err(EmitError::InvalidDescriptor(format!(
                    "StructuredForLoop `{loop_var}` index type is not a scalar"
                )));
            };
            to = self.append_expr(Expression::As {
                expr: to,
                kind,
                convert: Some(width),
            });
        }

        let bound_local = self.function.local_variables.append(
            LocalVariable {
                name: Some(format!("{loop_var}_end")),
                ty: index_ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        let bound_pointer = self.append_expr(Expression::LocalVariable(bound_local));
        self.function.body.push(
            Statement::Store {
                pointer: bound_pointer,
                value: to,
            },
            Span::UNDEFINED,
        );

        let index_local = self.function.local_variables.append(
            LocalVariable {
                name: Some(loop_var.to_string()),
                ty: index_ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        let index_pointer = self.append_expr(Expression::LocalVariable(index_local));
        self.function.body.push(
            Statement::Store {
                pointer: index_pointer,
                value: from,
            },
            Span::UNDEFINED,
        );

        // Q7: pre-allocate carriers for loop-carried ids and seed them
        // from the pre-loop SSA value (if the parent body had bound the
        // same id beforehand  -  common shape: `Let("hash", u32(seed))`
        // outside the loop, `Assign("hash", ...)` inside).
        for (id, init_handle) in &pre_init {
            let local = self.allocate_carrier_local(*id, init_handle);
            let pointer = self.append_expr(Expression::LocalVariable(local));
            self.function.body.push(
                Statement::Store {
                    pointer,
                    value: *init_handle,
                },
                Span::UNDEFINED,
            );
        }

        let previous_local = self.loop_locals.insert(loop_var.clone(), index_local);
        let previous_type = self.loop_types.insert(loop_var.clone(), index_ty);
        let body_result = self.loop_body_block(body, op, index_local, bound_local);
        match previous_local {
            Some(previous) => {
                self.loop_locals.insert(loop_var.clone(), previous);
            }
            None => {
                self.loop_locals.remove(loop_var);
            }
        }
        match previous_type {
            Some(previous) => {
                self.loop_types.insert(loop_var.clone(), previous);
            }
            None => {
                self.loop_types.remove(loop_var);
            }
        }
        let loop_body = body_result?;
        let (continuing, break_if) =
            self.loop_continuing_block(index_local, bound_local, index_ty)?;
        self.function.body.push(
            Statement::Loop {
                body: loop_body,
                continuing,
                break_if: Some(break_if),
            },
            Span::UNDEFINED,
        );

        // Q7: after the loop closes, rebind every loop-carried id to a
        // fresh `Load` from its carrier local, in the parent block. The
        // load's `Statement::Emit` is appended via `append_expr` so any
        // post-loop reader resolves the operand to the loaded value
        // instead of an in-loop SSA handle that is now out of scope.
        for id in &new_targets {
            if let Some(local) = self.loop_carrier_locals.get(id).copied() {
                let pointer = self.append_expr(Expression::LocalVariable(local));
                let load = self.append_expr(Expression::Load { pointer });
                self.values.insert(*id, load);
            }
        }
        // Restore prior carrier state so a sibling loop in the same
        // parent body sees a clean slate.
        self.restore_loop_carriers(prior_carriers);
        Ok(())
    }

    pub(super) fn loop_body_block(
        &mut self,
        body: &KernelBody,
        op: &KernelOp,
        index_local: naga::Handle<LocalVariable>,
        bound_local: naga::Handle<LocalVariable>,
    ) -> Result<Block, EmitError> {
        let mut loop_body = self.loop_guard_block(index_local, bound_local)?;
        loop_body.extend_block(self.child_block(body, op, 2)?);
        Ok(loop_body)
    }

    pub(super) fn loop_guard_block(
        &mut self,
        index_local: naga::Handle<LocalVariable>,
        bound_local: naga::Handle<LocalVariable>,
    ) -> Result<Block, EmitError> {
        let outer = std::mem::replace(&mut self.function.body, Block::new());
        let condition = self.loop_break_condition(index_local, bound_local)?;
        let mut accept = Block::new();
        accept.push(Statement::Break, Span::UNDEFINED);
        self.function.body.push(
            Statement::If {
                condition,
                accept,
                reject: Block::new(),
            },
            Span::UNDEFINED,
        );
        Ok(std::mem::replace(&mut self.function.body, outer))
    }

    pub(super) fn loop_continuing_block(
        &mut self,
        index_local: naga::Handle<LocalVariable>,
        bound_local: naga::Handle<LocalVariable>,
        index_ty: naga::Handle<Type>,
    ) -> Result<(Block, naga::Handle<Expression>), EmitError> {
        let outer = std::mem::replace(&mut self.function.body, Block::new());
        let pointer = self.append_expr(Expression::LocalVariable(index_local));
        let current = self.append_expr(Expression::Load { pointer });
        let one = self.one_literal_for_type(index_ty)?;
        let next = self.append_expr(Expression::Binary {
            op: BinaryOperator::Add,
            left: current,
            right: one,
        });
        self.function.body.push(
            Statement::Store {
                pointer,
                value: next,
            },
            Span::UNDEFINED,
        );
        let break_if = self.loop_break_condition(index_local, bound_local)?;
        Ok((std::mem::replace(&mut self.function.body, outer), break_if))
    }

    pub(super) fn loop_break_condition(
        &mut self,
        index_local: naga::Handle<LocalVariable>,
        bound_local: naga::Handle<LocalVariable>,
    ) -> Result<naga::Handle<Expression>, EmitError> {
        let pointer = self.append_expr(Expression::LocalVariable(index_local));
        let current = self.append_expr(Expression::Load { pointer });
        let bound_pointer = self.append_expr(Expression::LocalVariable(bound_local));
        let end = self.append_expr(Expression::Load {
            pointer: bound_pointer,
        });
        Ok(self.append_expr(Expression::Binary {
            op: BinaryOperator::GreaterEqual,
            left: current,
            right: end,
        }))
    }

    pub(super) fn one_literal_for_type(
        &mut self,
        ty: naga::Handle<Type>,
    ) -> Result<naga::Handle<Expression>, EmitError> {
        if ty == self.types.u32_ty {
            Ok(self.append_expr(Expression::Literal(Literal::U32(1))))
        } else if ty == self.types.i32_ty {
            Ok(self.append_expr(Expression::Literal(Literal::I32(1))))
        } else {
            Err(EmitError::InvalidDescriptor(
                "StructuredForLoop bounds must be u32 or i32".to_owned(),
            ))
        }
    }

    pub(super) fn emit_counted_u32_loop(
        &mut self,
        label: &str,
        end_value: naga::Handle<Expression>,
        emit_body: impl FnOnce(&mut Self, naga::Handle<Expression>) -> Result<(), EmitError>,
    ) -> Result<(), EmitError> {
        let index_ty = self.types.u32_ty;
        let suffix = self.function.local_variables.len();
        let bound_local = self.function.local_variables.append(
            LocalVariable {
                name: Some(format!("__vyre_{label}_end_{suffix}")),
                ty: index_ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        let bound_pointer = self.append_expr(Expression::LocalVariable(bound_local));
        self.function.body.push(
            Statement::Store {
                pointer: bound_pointer,
                value: end_value,
            },
            Span::UNDEFINED,
        );

        let index_local = self.function.local_variables.append(
            LocalVariable {
                name: Some(format!("__vyre_{label}_{suffix}")),
                ty: index_ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        let index_pointer = self.append_expr(Expression::LocalVariable(index_local));
        let zero = self.literal_u32(0);
        self.function.body.push(
            Statement::Store {
                pointer: index_pointer,
                value: zero,
            },
            Span::UNDEFINED,
        );

        let mut loop_body = self.loop_guard_block(index_local, bound_local)?;
        let outer = std::mem::replace(&mut self.function.body, Block::new());
        let index_pointer = self.append_expr(Expression::LocalVariable(index_local));
        let index_value = self.append_expr(Expression::Load {
            pointer: index_pointer,
        });
        let result = emit_body(self, index_value);
        let emitted_body = std::mem::replace(&mut self.function.body, outer);
        result?;
        loop_body.extend_block(emitted_body);

        let (continuing, break_if) =
            self.loop_continuing_block(index_local, bound_local, index_ty)?;
        self.function.body.push(
            Statement::Loop {
                body: loop_body,
                continuing,
                break_if: Some(break_if),
            },
            Span::UNDEFINED,
        );
        Ok(())
    }
}
