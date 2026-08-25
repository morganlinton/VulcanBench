//! `BodyBuilder` binding/operand/result-binding helpers. The leaf
//! pieces every op-emit path consumes:
//!
//! - `slot_operand` / `value_operand` / `value_type_operand`  -  read
//!   typed pieces out of a `KernelOp`'s `operands` Vec.
//! - `binding_element_pointer*` / `buffer_len_expr`  -  turn a
//!   `(slot, index)` pair into a naga pointer / length expression.
//! - `bind_result` / `bind_result_typed`  -  record an emitted value's
//!   handle (and type) for downstream consumers, with Q7 carrier-store
//!   plumbing for loop-carried ids.
//! - `child_block` / `inline_axis` / `require_u32_slot`  -  small one-off
//!   guards.
//! - `literal_type` / `type_for_data_type` / `binary_result_type`  -
//!   IR-type → naga-type lookups.

use naga::{BinaryOperator, Expression, Span, Statement, Type};
use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::{KernelBody, KernelOp, LiteralValue};

use super::BodyBuilder;
use crate::EmitError;

impl BodyBuilder<'_> {
    pub(super) fn inline_axis(&self, op: &KernelOp) -> Result<u32, EmitError> {
        let axis = *op.operands.first().ok_or_else(|| {
            EmitError::InvalidDescriptor("builtin id op missing axis operand".to_owned())
        })?;
        if axis > 2 {
            return Err(EmitError::InvalidDescriptor(format!(
                "builtin id axis {axis} is out of range"
            )));
        }
        Ok(axis)
    }

    pub(super) fn slot_operand(&self, op: &KernelOp, index: usize) -> Result<u32, EmitError> {
        op.operands.get(index).copied().ok_or_else(|| {
            EmitError::InvalidDescriptor(format!("{:?} missing slot operand {index}", op.kind))
        })
    }

    pub(super) fn require_u32_slot(&self, slot: u32, context: &str) -> Result<(), EmitError> {
        let ty =
            self.binding_types
                .get(&slot)
                .copied()
                .ok_or_else(|| EmitError::InvalidBinding {
                    slot,
                    reason: format!("{context} has no tracked scalar type"),
                })?;
        if ty == self.types.u32_ty {
            Ok(())
        } else {
            Err(EmitError::InvalidBinding {
                slot,
                reason: format!("{context} must use u32 word buffers for descriptor async copy"),
            })
        }
    }

    pub(super) fn binding_element_pointer(
        &mut self,
        op: &KernelOp,
        slot_operand: usize,
        index_operand: usize,
    ) -> Result<naga::Handle<Expression>, EmitError> {
        let slot = *op.operands.get(slot_operand).ok_or_else(|| {
            EmitError::InvalidDescriptor(format!("{:?} missing binding slot", op.kind))
        })?;
        let index = self.value_operand(op, index_operand)?;
        self.binding_element_pointer_by_slot(slot, index)
    }

    pub(super) fn binding_element_pointer_by_slot(
        &mut self,
        slot: u32,
        index: naga::Handle<Expression>,
    ) -> Result<naga::Handle<Expression>, EmitError> {
        let global = *self
            .globals
            .get(&slot)
            .ok_or_else(|| EmitError::InvalidBinding {
                slot,
                reason: "no global variable was declared for this slot".to_owned(),
            })?;
        let base = self.append_expr(Expression::GlobalVariable(global));
        // Coerce the array index to u32  -  naga rejects Bool / Sint / Float
        // indices with `InvalidIndexType`. With the Q7 carrier-publish
        // round-trip a Bool result that originally inlined as `1u`/`0u`
        // can now Load as Bool; force u32 here.
        let index = self.coerce_value_to_type(index, self.types.u32_ty);
        Ok(self.append_expr(Expression::Access { base, index }))
    }

    pub(super) fn buffer_len_expr(
        &mut self,
        slot: u32,
    ) -> Result<naga::Handle<Expression>, EmitError> {
        if let Some(Some(count)) = self.binding_counts.get(&slot) {
            return Ok(self.literal_u32(*count));
        }
        let global = *self
            .globals
            .get(&slot)
            .ok_or_else(|| EmitError::InvalidBinding {
                slot,
                reason: "no global variable was declared for this slot".to_owned(),
            })?;
        let pointer = self.append_expr(Expression::GlobalVariable(global));
        let words = self.append_expr(Expression::ArrayLength(pointer));
        if matches!(
            self.binding_data_types.get(&slot),
            Some(DataType::U8 | DataType::I8)
        ) {
            let bytes_per_word = self.literal_u32(4);
            return Ok(self.append_expr(Expression::Binary {
                op: BinaryOperator::Multiply,
                left: words,
                right: bytes_per_word,
            }));
        }
        Ok(words)
    }

    pub(super) fn child_block(
        &mut self,
        body: &KernelBody,
        op: &KernelOp,
        operand: usize,
    ) -> Result<naga::Block, EmitError> {
        let child_id = *op.operands.get(operand).ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "{:?} missing child-body operand {operand}",
                op.kind
            ))
        })?;
        let child = body.child_bodies.get(child_id as usize).ok_or_else(|| {
            EmitError::InvalidDescriptor(format!("child body {child_id} is not present"))
        })?;
        let outer = std::mem::take(&mut self.function.body);
        self.child_body_depth = self.child_body_depth.saturating_add(1);
        let result = self.emit_body(child);
        self.child_body_depth = self.child_body_depth.saturating_sub(1);
        result?;
        let child_block = std::mem::replace(&mut self.function.body, outer);
        Ok(child_block)
    }

    pub(super) fn value_operand(
        &mut self,
        op: &KernelOp,
        index: usize,
    ) -> Result<naga::Handle<Expression>, EmitError> {
        let id = *op.operands.get(index).ok_or_else(|| {
            EmitError::InvalidDescriptor(format!("{:?} missing value operand {index}", op.kind))
        })?;
        // Q7 carrier scope-fix: if `id` is a loop-carried result, the cached
        // handle in `self.values` is a `Load` whose `Statement::Emit` lives
        // inside the block where bind_result fired (often a loop body).  When
        // a consumer in a later/outer block reads the carrier, that cached
        // handle is no longer in naga's scope tree and validation rejects
        // the use as `NotInScope`.  Synthesize a fresh Load in the *current*
        // block so the Emit lands where the consumer needs it.  The
        // `LocalVariable` is function-scoped so always in scope; the new
        // Load's Emit is appended to `self.function.body` (the current block).
        if let Some(handle) = self.value_handle_for_id(id) {
            return Ok(handle);
        }
        Err(EmitError::InvalidDescriptor(format!(
            "{:?} references missing value id {id}",
            op.kind
        )))
    }

    /// Resolve the Naga expression handle for a vyre SSA value id in the
    /// CURRENT block scope.
    ///
    /// If `id` is a loop-carried result, synthesizes a fresh
    /// `LocalVariable` + `Load` pair in the current block (so the
    /// `Statement::Emit` lands where the consumer needs it).  Otherwise
    /// returns whatever was cached in `self.values`.
    pub(super) fn value_handle_for_id(&mut self, id: u32) -> Option<naga::Handle<Expression>> {
        // If the cached carrier local was allocated with a different
        // scalar type than the authoritative `value_types[id]` (a
        // later `bind_result_typed` rebound the same vyre id to a
        // different kind  -  e.g. id 873 was first a Bool comparison,
        // then reused as a u32 LoopIndex), Load through the cached
        // local AND coerce the result to the expected type. Just
        // dropping the cached entry sends the consumer to a
        // self.values handle whose Emit may be in a closed scope
        // (the original NotInScope failure mode); coercion preserves
        // the scope-safety of the LocalVariable round-trip while
        // honoring the consumer's expected type.
        let expected_ty = self.value_types.get(&id).copied();
        // A descriptor can reuse a numeric id across distinct SSA values after
        // control-flow lowering. When that happens, a later block-scoped bind
        // is the current SSA value and must shadow an older loop-carrier local;
        // otherwise consumers reload stale carrier state, as in
        // utf8_shape_counts resetting `expected` from a literal carrier.
        if let Some(local) = self.block_scoped_locals.get(&id).copied() {
            let pointer = self.append_expr(Expression::LocalVariable(local));
            let load = self.append_expr(Expression::Load { pointer });
            let coerced = match expected_ty {
                Some(t) => self.coerce_value_to_type(load, t),
                None => load,
            };
            return Some(coerced);
        }
        if let Some(local) = self.loop_carrier_locals.get(&id).copied() {
            let pointer = self.append_expr(Expression::LocalVariable(local));
            let load = self.append_expr(Expression::Load { pointer });
            let coerced = match expected_ty {
                Some(t) => self.coerce_value_to_type(load, t),
                None => load,
            };
            return Some(coerced);
        }
        if let Some(value) = self.values.get(&id).copied() {
            return Some(value);
        }
        // Last-resort recovery for a malformed descriptor path that
        // references a named carrier result without a bound snapshot.
        // Correct descriptors bind every `LoopCarrier` result through
        // `emit_loop_carrier_read`; consumers must see that SSA snapshot,
        // not a fresh carrier reload after later `LoopCarrierEnd` writes.
        if let Some(name) = self.named_carrier_result_ids.get(&id).cloned() {
            if let Some(local) = self.named_carrier_locals.get(&name).copied() {
                let pointer = self.append_expr(Expression::LocalVariable(local));
                let load = self.append_expr(Expression::Load { pointer });
                let coerced = match expected_ty {
                    Some(t) => self.coerce_value_to_type(load, t),
                    None => load,
                };
                return Some(coerced);
            }
        }
        None
    }

    pub(super) fn bind_result(
        &mut self,
        op: &KernelOp,
        value: naga::Handle<Expression>,
    ) -> Result<(), EmitError> {
        let result = op.result.ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "{:?} produced a value without result id",
                op.kind
            ))
        })?;
        // Q7: when this id is loop-carried, also Store to the carrier
        // local so the post-loop reader sees the final iteration's
        // value, and rebind `self.values[id]` to a fresh Load so any
        // subsequent reader inside the same iteration also reads the
        // updated value (rather than the just-computed expression
        // handle, which would shadow later iterations).
        if self.loop_carrier_targets.contains(&result) {
            // Numeric descriptor ids are not globally unique after structured
            // lowering. A previous child block may have allocated a
            // block-scoped local for the same number, but this bind is the
            // current loop-carried SSA value. If the stale block local stays
            // in the map, value_handle_for_id resolves future accumulator
            // reads to the older block constant before it ever reaches the
            // live loop carrier. Sinkhorn's GEMM accumulator hit exactly that:
            // id 99 first denoted matrix width `2`, then the accumulator
            // snapshot, causing every sum to start at 2. Drop the stale block
            // shadow when a real carrier owns the id.
            self.block_scoped_locals.remove(&result);
            let init = value;
            if std::env::var("VYRE_PUBLISH_TRACE").is_ok() {
                let underlying = self.resolve_underlying_local_kind(init);
                let scalar = self.scalar_kind_of_expression(init, 0);
                tracing::debug!(
                    "[publish] result={result} op={kind:?} init={init:?} scalar={scalar:?} underlying_local={underlying:?}",
                    kind = op.kind,
                );
            }
            let local = self.allocate_carrier_local(result, &init);
            // Coerce the value to the carrier's actual type. The local
            // is typed from the *first* init we saw; later writes to
            // the same carrier may carry a different scalar kind (e.g.
            // a u32 carrier reused as a bool flag), and naga rejects
            // the store with InvalidStoreTypes if we don't match.
            let local_ty = self.function.local_variables[local].ty;
            let init = self.coerce_value_to_type(init, local_ty);
            let pointer = self.append_expr(Expression::LocalVariable(local));
            self.function.body.push(
                Statement::Store {
                    pointer,
                    value: init,
                },
                Span::UNDEFINED,
            );
            let load_pointer = self.append_expr(Expression::LocalVariable(local));
            let load = self.append_expr(Expression::Load {
                pointer: load_pointer,
            });
            self.values.insert(result, load);
        } else if self.child_body_depth > 0 {
            // Conservative block-scoping fix: any value produced inside a
            // child block (if-then arm, loop body, etc.) gets a function-
            // scope LocalVariable so it can be re-Loaded in a different
            // block. Without this, naga's expression-scoping validator
            // rejects the SSA handle as NotInScope when it's referenced
            // from a block that doesn't contain its Statement::Emit.
            let init = value;
            let local = self.allocate_block_scoped_local(result, &init);
            let local_ty = self.function.local_variables[local].ty;
            let init = self.coerce_value_to_type(init, local_ty);
            let pointer = self.append_expr(Expression::LocalVariable(local));
            self.function.body.push(
                Statement::Store {
                    pointer,
                    value: init,
                },
                Span::UNDEFINED,
            );
            let load_pointer = self.append_expr(Expression::LocalVariable(local));
            let load = self.append_expr(Expression::Load {
                pointer: load_pointer,
            });
            self.values.insert(result, load);
        } else {
            // A top-level rebind is authoritative in the current SSA scope.
            // Clear older function-local shadows for the same numeric id so
            // later operands do not prefer a stale child-block or loop-carrier
            // value over the freshly inserted inline handle.
            self.block_scoped_locals.remove(&result);
            self.loop_carrier_locals.remove(&result);
            self.values.insert(result, value);
        }

        if let Ok(log_path) = std::env::var("VYRE_BIND_RESULT_LOG") {
            let entry = crate::BindResultEntry {
                vyre_op_id: result,
                op_kind: format!("{:?}", op.kind),
                init_handle: value.index() as u32,
                init_scalar_kind: self
                    .scalar_kind_of_expression(value, 0)
                    .map(|s| format!("{:?}", s)),
                child_body_depth: self.child_body_depth,
                value_types_at_call: self.value_types.get(&result).map(|t| t.index() as u32),
                publish_path: if self.loop_carrier_targets.contains(&result) {
                    "LoopCarrier".to_string()
                } else if self.child_body_depth > 0 {
                    "BlockScoped".to_string()
                } else {
                    "Inline".to_string()
                },
                local_allocated_ty: if self.loop_carrier_targets.contains(&result) {
                    Some(
                        self.function.local_variables[self.loop_carrier_locals[&result]]
                            .ty
                            .index() as u32,
                    )
                } else if self.child_body_depth > 0 {
                    Some(
                        self.function.local_variables[self.block_scoped_locals[&result]]
                            .ty
                            .index() as u32,
                    )
                } else {
                    None
                },
            };
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
            {
                let _ = writeln!(
                    file,
                    "{}",
                    serde_json::to_string(&entry).unwrap_or_else(|_| unreachable!("unwrap site"))
                );
            }
        }

        Ok(())
    }

    pub(super) fn bind_result_typed(
        &mut self,
        op: &KernelOp,
        value: naga::Handle<Expression>,
        ty: naga::Handle<Type>,
    ) -> Result<(), EmitError> {
        // Insert the type BEFORE bind_result so the publish-via-local
        // path (in `bind_result`) can read the authoritative type from
        // `value_types[id]` instead of falling back to
        // `scalar_kind_of_expression`, which returns `None` on complex
        // operands like nested `Select`/`Binary` and defaults the local
        // to u32  -  silently mis-typing Bool results and breaking
        // downstream Select-arm validation.
        let id = op.result.ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "{:?} produced a value without result id",
                op.kind
            ))
        })?;
        self.value_types.insert(id, ty);
        self.bind_result(op, value)
    }

    pub(super) fn value_type_operand(
        &self,
        op: &KernelOp,
        index: usize,
    ) -> Result<naga::Handle<Type>, EmitError> {
        let id = *op.operands.get(index).ok_or_else(|| {
            EmitError::InvalidDescriptor(format!("{:?} missing value operand {index}", op.kind))
        })?;
        self.value_types.get(&id).copied().ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "{:?} references value id {id} before its type was tracked",
                op.kind
            ))
        })
    }

    pub(super) fn literal_type(&self, literal: &LiteralValue) -> naga::Handle<Type> {
        match literal {
            LiteralValue::U32(_) => self.types.u32_ty,
            LiteralValue::I32(_) => self.types.i32_ty,
            LiteralValue::F32(_) => self.types.f32_ty,
            LiteralValue::Bool(_) => self.types.bool_ty,
        }
    }

    pub(super) fn type_for_data_type(
        &self,
        data_type: &DataType,
    ) -> Result<naga::Handle<Type>, EmitError> {
        match data_type {
            DataType::Bool => Ok(self.types.bool_ty),
            DataType::U8 | DataType::U16 | DataType::U32 => Ok(self.types.u32_ty),
            DataType::I8 | DataType::I16 | DataType::I32 => Ok(self.types.i32_ty),
            DataType::F32 => Ok(self.types.f32_ty),
            // `Bytes` has no scalar Naga value type: it is a packed-byte buffer
            // element. Lowering it to `u32_ty` would silently reinterpret a byte
            // stream as a word (Law 10). The buffer-element path already rejects
            // `Bytes` (it needs a pack-to-u32 pre-pass); the value/cast path must
            // be symmetric and fail closed too, never widen a byte to a word.
            DataType::Bytes => Err(EmitError::NagaConstructionFailed(
                "data type `Bytes` has no scalar Naga value type: `Bytes` is a \
                 packed-byte buffer element and must be unpacked via a pack-to-u32 \
                 pre-pass, never reinterpreted as a u32 word"
                    .to_owned(),
            )),
            // WGSL has no native 64-bit integer: U64/I64/Vec2U32 values are
            // backed by vec2<u32> (low word `.x`, high word `.y`). The native
            // `u64_ty`/`i64_ty` handles are reserved for the subgroup/literal
            // paths that emit them directly under a u64 capability.
            DataType::U64 | DataType::I64 | DataType::Vec2U32 => Ok(self.types.vec2_u32_ty),
            DataType::Vec4U32 => Ok(self.types.vec4_u32_ty),
            other => Err(EmitError::NagaConstructionFailed(format!(
                "data type `{other:?}` has no scalar Naga descriptor type"
            ))),
        }
    }

    pub(super) fn binary_result_type(
        &self,
        op: &KernelOp,
        binop: BinOp,
    ) -> Result<naga::Handle<Type>, EmitError> {
        Ok(match binop {
            BinOp::Eq
            | BinOp::Ne
            | BinOp::Lt
            | BinOp::Le
            | BinOp::Gt
            | BinOp::Ge
            | BinOp::And
            | BinOp::Or => self.types.bool_ty,
            _ => self.value_type_operand(op, 0)?,
        })
    }
}
