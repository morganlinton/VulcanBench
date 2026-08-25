//! Loop-carried SSA carrier analysis for structured loop lowering.

use rustc_hash::{FxHashMap, FxHashSet};

use naga::{Expression, LocalVariable, ScalarKind, Span, Statement};
use vyre_lower::{KernelBody, KernelOp, KernelOpKind};

use super::BodyBuilder;
use crate::EmitError;

impl BodyBuilder<'_> {
    pub(super) fn snapshot_loop_carriers(
        &self,
    ) -> (
        FxHashSet<u32>,
        FxHashMap<u32, naga::Handle<LocalVariable>>,
        FxHashMap<u32, naga::Handle<LocalVariable>>,
    ) {
        (
            self.loop_carrier_targets.clone(),
            self.loop_carrier_locals.clone(),
            self.block_scoped_locals.clone(),
        )
    }

    /// Restore the carrier-target gate after a structured loop ends.
    ///
    /// `loop_carrier_targets` is restored (it is a per-loop gate that
    /// controls whether `bind_result` runs the Q7 carrier-publish path), and
    /// the carrier-local resolver map is restored to the parent scope. The
    /// allocated `naga::LocalVariable`s remain function-scoped, but their
    /// numeric ids must not keep shadowing parent SSA after the structured
    /// loop closes. Lowering can reuse descriptor ids for unrelated outer
    /// values and inner loop temporaries; leaving the inner carrier map live
    /// made Sinkhorn's inner GEMM loop replace the outer lane/count ids after
    /// the loop. Post-loop users that genuinely need the loop result are
    /// already rebound to a fresh Load in `emit_structured_for_loop`.
    pub(super) fn restore_loop_carriers(
        &mut self,
        snapshot: (
            FxHashSet<u32>,
            FxHashMap<u32, naga::Handle<LocalVariable>>,
            FxHashMap<u32, naga::Handle<LocalVariable>>,
        ),
    ) {
        let (targets, locals, block_locals) = snapshot;
        self.loop_carrier_targets = targets;
        self.loop_carrier_locals = locals;
        self.block_scoped_locals = block_locals;
    }

    pub(super) fn collect_loop_carried_ids(
        &self,
        parent: &KernelBody,
        loop_op: &KernelOp,
        loop_child: &KernelBody,
    ) -> FxHashSet<u32> {
        let child_idx = match loop_op.operands.get(2).copied() {
            Some(idx) => idx,
            None => return FxHashSet::default(),
        };
        let loop_pos = parent.ops.iter().position(|op| {
            matches!(&op.kind, KernelOpKind::StructuredForLoop { .. })
                && op.operands.get(2).copied() == Some(child_idx)
        });
        let Some(loop_pos) = loop_pos else {
            return FxHashSet::default();
        };

        let mut produced_inside = FxHashSet::default();
        collect_produced_ids(loop_child, &mut produced_inside);

        let mut referenced_after = FxHashSet::default();
        for op in parent.ops.iter().skip(loop_pos + 1) {
            collect_op_referenced_ids(op, parent, &mut referenced_after);
        }

        produced_inside
            .into_iter()
            .filter(|id| referenced_after.contains(id))
            .collect()
    }

    /// Carrier analysis for a generic structured op (`StructuredIfThen`,
    /// `StructuredIfThenElse`, etc.): every id produced inside any of
    /// the op's child bodies that the parent body references *after*
    /// this op. Returns the set so the caller can `loop_carrier_targets
    /// .insert(id)` for each, register `pre_init` Stores from the
    /// parent's pre-existing SSA (if any), and re-bind via fresh Loads
    /// after the child blocks emit.
    ///
    /// Without this, naga's WGSL writer emits `let _eN = ...;` inside
    /// the if-body, and the post-if reader uses `_eN` from the outer
    /// scope  -  wgpu rejects with `no definition in scope for identifier
    /// _eN`. The carrier-local round-trip is the same fix as for
    /// loops, just generalized over which structured op opens the child.
    pub(super) fn collect_child_carried_ids(
        &self,
        parent: &KernelBody,
        op_pos: usize,
        child_indices: &[u32],
    ) -> FxHashSet<u32> {
        let mut produced_inside = FxHashSet::default();
        for child_idx in child_indices {
            if let Some(child) = parent.child_bodies.get(*child_idx as usize) {
                collect_produced_ids(child, &mut produced_inside);
            }
        }

        let mut referenced_after = FxHashSet::default();
        for op in parent.ops.iter().skip(op_pos + 1) {
            collect_op_referenced_ids(op, parent, &mut referenced_after);
        }

        produced_inside
            .into_iter()
            .filter(|id| referenced_after.contains(id))
            .collect()
    }

    pub(super) fn allocate_carrier_local(
        &mut self,
        id: u32,
        init_handle: &naga::Handle<Expression>,
    ) -> naga::Handle<LocalVariable> {
        // Decide the authoritative type for this id at THIS point in
        // emission. value_types[id] is set by `bind_result_typed`, which
        // runs BEFORE bind_result. Constrain to types naga accepts as a
        // `LocalVariable`: canonical scalars AND the `vec2<u32>`/`vec3<u32>`
        // backings (U64/I64 lower to vec2<u32>). Atomic / array / struct
        // handles are still rejected by naga with `InvalidType`, so they
        // fall through to the scalar-kind heuristic below.
        let value_types_scalar = self.value_types.get(&id).copied().filter(|ty| {
            *ty == self.types.bool_ty
                || *ty == self.types.u32_ty
                || *ty == self.types.i32_ty
                || *ty == self.types.f32_ty
                || *ty == self.types.vec2_u32_ty
                || *ty == self.types.vec3_u32_ty
        });
        let ty = value_types_scalar.unwrap_or_else(|| {
            match self.scalar_kind_of_expression(*init_handle, 0) {
                Some(naga::ScalarKind::Bool) => self.types.bool_ty,
                Some(naga::ScalarKind::Sint) => self.types.i32_ty,
                Some(naga::ScalarKind::Float) => self.types.f32_ty,
                Some(naga::ScalarKind::Uint) => self.types.u32_ty,
                _ => self.types.u32_ty,
            }
        });
        // If a carrier local for this id was allocated earlier with a
        // *different* type (e.g. a Bool comparison op bound id 873 first,
        // then later a `LoopIndex` op rebound the same id to u32), the
        // cached Bool local is stale. Allocate a fresh u32 local now and
        // overwrite the entry  -  every consumer goes through
        // `value_handle_for_id` which Loads via the map, so subsequent
        // reads see the new typing.
        if let Some(existing) = self.loop_carrier_locals.get(&id).copied() {
            if self.function.local_variables[existing].ty == ty {
                return existing;
            }
        }
        let local = self.function.local_variables.append(
            LocalVariable {
                name: Some(format!("vyre_loop_carry_{id}")),
                ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        self.loop_carrier_locals.insert(id, local);
        local
    }

    /// Allocate a function-scope `LocalVariable` for a value produced
    /// inside a child block (if-then arm, loop body, etc.).  Unlike
    /// loop-carrier locals, these are NOT iteration-to-iteration carriers;
    /// they simply ensure the value is accessible from any block after it
    /// is computed, avoiding naga's `NotInScope` validation error.
    pub(super) fn allocate_block_scoped_local(
        &mut self,
        id: u32,
        init_handle: &naga::Handle<Expression>,
    ) -> naga::Handle<LocalVariable> {
        // Decide the canonical local type for THIS binding FIRST (the cache
        // reuse below depends on it). Constrain to types naga accepts as a
        // `LocalVariable`: canonical scalars AND the `vec2<u32>`/`vec3<u32>`
        // backings (U64/I64 lower to vec2<u32>, so a block-scoped 64-bit value
        // needs a vec2 local — not a u32 default, which would make the coerced
        // Store fail `InvalidStoreTypes`). The naïve
        // `value_types.get(&id).unwrap_or(u32_ty)` fallback also returned
        // atomic / array / struct handles, which naga rejects with
        // `InvalidType`; those still fall through to the u32 default below.
        let value_types_scalar = self.value_types.get(&id).copied().filter(|ty| {
            *ty == self.types.bool_ty
                || *ty == self.types.u32_ty
                || *ty == self.types.i32_ty
                || *ty == self.types.f32_ty
                || *ty == self.types.vec2_u32_ty
                || *ty == self.types.vec3_u32_ty
        });
        let ty = match self.scalar_kind_of_expression(*init_handle, 0) {
            Some(naga::ScalarKind::Bool) => self.types.bool_ty,
            Some(naga::ScalarKind::Sint) => self.types.i32_ty,
            Some(naga::ScalarKind::Float) => self.types.f32_ty,
            Some(naga::ScalarKind::Uint) => self.types.u32_ty,
            _ => value_types_scalar.unwrap_or(self.types.u32_ty),
        };
        // Reuse the cached local ONLY when its type matches the type required
        // now. Lowering reuses an SSA id across sibling blocks for values of a
        // *different* scalar kind (e.g. a Bool comparison result, then later a
        // u32 state word in an NFA-scan shader). Returning the stale-typed local
        // makes the caller's coerced Store validate as `InvalidStoreTypes`
        // whenever the scalar-kind heuristic can't re-derive the value's kind
        // (coerce_value_to_type then no-ops, leaving a u32 value stored into a
        // bool local). This mirrors `allocate_carrier_local`'s re-typing guard.
        // Repro: a 2-byte NFA literal (`num_states == 4`) before this guard.
        if let Some(local) = self.block_scoped_locals.get(&id).copied() {
            if self.function.local_variables[local].ty == ty {
                return local;
            }
        }
        let local = self.function.local_variables.append(
            LocalVariable {
                name: Some(format!("vyre_block_scope_{id}")),
                ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        self.block_scoped_locals.insert(id, local);
        local
    }
}

impl BodyBuilder<'_> {
    /// Allocate (idempotent) the function-scope LocalVariable that backs
    /// the source-level loop carrier `name`, type-decided from the seed
    /// expression's scalar kind.
    fn ensure_named_carrier_local(
        &mut self,
        name: &vyre_lower::descriptor::Name,
        seed_handle: naga::Handle<Expression>,
    ) -> naga::Handle<LocalVariable> {
        if let Some(existing) = self.named_carrier_locals.get(name).copied() {
            return existing;
        }
        let ty = match self.scalar_kind_of_expression(seed_handle, 0) {
            Some(ScalarKind::Bool) => self.types.bool_ty,
            Some(ScalarKind::Sint) => self.types.i32_ty,
            Some(ScalarKind::Float) => self.types.f32_ty,
            Some(ScalarKind::Uint) => self.types.u32_ty,
            _ => self.types.u32_ty,
        };
        let local = self.function.local_variables.append(
            LocalVariable {
                name: Some(format!("vyre_named_carry_{name}")),
                ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        self.named_carrier_locals.insert(name.clone(), local);
        self.named_carrier_types.insert(name.clone(), ty);
        local
    }

    pub(super) fn emit_loop_carrier_init(
        &mut self,
        op: &KernelOp,
        name: &vyre_lower::descriptor::Name,
    ) -> Result<(), EmitError> {
        let seed = self.value_operand(op, 0)?;
        let local = self.ensure_named_carrier_local(name, seed);
        let local_ty = self.function.local_variables[local].ty;
        let value = self.coerce_value_to_type(seed, local_ty);
        let pointer = self.append_expr(Expression::LocalVariable(local));
        self.function
            .body
            .push(Statement::Store { pointer, value }, Span::UNDEFINED);
        Ok(())
    }

    pub(super) fn emit_loop_carrier_read(
        &mut self,
        op: &KernelOp,
        name: &vyre_lower::descriptor::Name,
    ) -> Result<(), EmitError> {
        let local = *self.named_carrier_locals.get(name).ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "LoopCarrier `{name}` read before any LoopCarrierInit allocated its slot. \
                 Fix: lower a LoopCarrierInit op into the parent body before any \
                 LoopCarrier/LoopCarrierEnd op for this name."
            ))
        })?;
        let ty = self.function.local_variables[local].ty;
        let carrier_pointer = self.append_expr(Expression::LocalVariable(local));
        let value = self.append_expr(Expression::Load {
            pointer: carrier_pointer,
        });
        let snapshot = self.function.local_variables.append(
            LocalVariable {
                name: op
                    .result
                    .map(|id| format!("vyre_named_carry_snapshot_{id}")),
                ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        let snapshot_pointer = self.append_expr(Expression::LocalVariable(snapshot));
        self.function.body.push(
            Statement::Store {
                pointer: snapshot_pointer,
                value,
            },
            Span::UNDEFINED,
        );
        let snapshot_pointer = self.append_expr(Expression::LocalVariable(snapshot));
        let snapshot_value = self.append_expr(Expression::Load {
            pointer: snapshot_pointer,
        });
        self.bind_result_typed(op, snapshot_value, ty)
    }

    pub(super) fn emit_loop_carrier_end(
        &mut self,
        op: &KernelOp,
        name: &vyre_lower::descriptor::Name,
    ) -> Result<(), EmitError> {
        let value = self.value_operand(op, 0)?;
        let local = *self.named_carrier_locals.get(name).ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "LoopCarrierEnd `{name}` writes before any LoopCarrierInit allocated its slot."
            ))
        })?;
        let local_ty = self.function.local_variables[local].ty;
        let value = self.coerce_value_to_type(value, local_ty);
        let pointer = self.append_expr(Expression::LocalVariable(local));
        self.function
            .body
            .push(Statement::Store { pointer, value }, Span::UNDEFINED);
        Ok(())
    }
}

fn collect_op_referenced_ids(op: &KernelOp, parent: &KernelBody, out: &mut FxHashSet<u32>) {
    match &op.kind {
        KernelOpKind::StructuredIfThen | KernelOpKind::StructuredBlock => {
            if let Some(&cond) = op.operands.first() {
                out.insert(cond);
            }
            if let Some(&child_idx) = op.operands.get(1) {
                if let Some(child) = parent.child_bodies.get(child_idx as usize) {
                    collect_body_referenced_ids(child, out);
                }
            }
        }
        KernelOpKind::StructuredIfThenElse => {
            if let Some(&cond) = op.operands.first() {
                out.insert(cond);
            }
            for child_idx in op.operands.iter().skip(1).copied() {
                if let Some(child) = parent.child_bodies.get(child_idx as usize) {
                    collect_body_referenced_ids(child, out);
                }
            }
        }
        KernelOpKind::StructuredForLoop { .. } => {
            if let Some(&from) = op.operands.first() {
                out.insert(from);
            }
            if let Some(&to) = op.operands.get(1) {
                out.insert(to);
            }
            if let Some(&child_idx) = op.operands.get(2) {
                if let Some(child) = parent.child_bodies.get(child_idx as usize) {
                    collect_body_referenced_ids(child, out);
                }
            }
        }
        KernelOpKind::Region { .. } => {
            if let Some(&child_idx) = op.operands.first() {
                if let Some(child) = parent.child_bodies.get(child_idx as usize) {
                    collect_body_referenced_ids(child, out);
                }
            }
        }
        _ => {
            for &operand in &op.operands {
                out.insert(operand);
            }
        }
    }
}

fn collect_body_referenced_ids(body: &KernelBody, out: &mut FxHashSet<u32>) {
    for op in &body.ops {
        collect_op_referenced_ids(op, body, out);
    }
}

fn collect_produced_ids(body: &KernelBody, out: &mut FxHashSet<u32>) {
    for op in &body.ops {
        if let Some(result) = op.result {
            out.insert(result);
        }
    }
    for child in &body.child_bodies {
        collect_produced_ids(child, out);
    }
}
