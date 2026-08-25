use std::fmt::Write as _;

use crate::reg::{PtxType, Reg};
use crate::{EmitError, PtxEmitOptions};
use rustc_hash::{FxHashMap, FxHashSet};
use vyre_lower::descriptor::Name;
use vyre_lower::verify::{classify_operand, OperandClass};
use vyre_lower::{
    BindingSlot, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

mod async_copy;
mod atomic;
mod body;
mod context;
mod control;
mod facts;
mod format;
mod memory;
mod mma;
mod module;
mod names;
mod operands;
mod results;
mod scalar;
mod schedule;
mod sizing;
mod vector;

use memory::AsyncCopyDirection;
use module::ModuleBuilder;
use operands::{read_store_operands, read_two_operands};
use sizing::estimated_module_text_capacity;

pub(crate) fn emit_text(
    desc: &KernelDescriptor,
    options: PtxEmitOptions,
) -> Result<String, EmitError> {
    let mut module = ModuleBuilder::new(options, estimated_module_text_capacity(desc));
    module.write_preamble();
    module.write_entry_point(desc)?;
    Ok(module.finish())
}

struct BodyCtx<'a> {
    options: PtxEmitOptions,
    text: String,
    /// Per-class next register index. Pre-decremented `<N>` register
    /// declarations are sized from these counters.
    next_pred: u32,
    next_b16: u32,
    next_u32: u32,
    next_i32: u32,
    next_f32: u32,
    next_u64: u32,
    /// Per-body next label index for branch targets.
    next_label: u32,
    /// Map from descriptor operand id → PTX register holding its value.
    operand_to_reg: FxHashMap<u32, Reg>,
    /// Descriptor result ids known to be literal u32 values. Used to
    /// fold constant buffer indices into immediate address offsets.
    u32_literals: FxHashMap<u32, u32>,
    /// Map from binding slot → its loaded global pointer register.
    slot_to_ptr: FxHashMap<u32, Reg>,
    /// Map from shared-memory binding slot → PTX shared symbol.
    slot_to_shared_symbol: FxHashMap<u32, String>,
    /// Read-only global bindings with enough spatial reuse to route loads
    /// through CUDA's non-coherent/read-only cache path (`ld.global.nc`).
    read_only_cache_slots: FxHashSet<u32>,
    /// Tags whose native cp.async groups have been committed but not yet
    /// waited. Keeping the wait at AsyncWait, instead of immediately after
    /// AsyncLoad, lets independent compute overlap with global-to-shared DMA.
    pending_cp_async_tags: FxHashSet<Name>,
    /// Active structured-loop induction values keyed by loop variable.
    loop_indices: FxHashMap<Name, Reg>,
    /// Per source-level loop-carrier name: the PTX register that
    /// carries the current value across iterations. Allocated by
    /// `LoopCarrierInit`, written by `LoopCarrierEnd`, read by
    /// `LoopCarrier`. Persists for the life of the kernel emission so
    /// post-loop reads pick up the loop's final value.
    named_carriers: FxHashMap<Name, Reg>,
    /// Result-id of every `LoopCarrier` op, mapped back to its
    /// carrier name so the binder can return the carrier register
    /// directly to consumers.
    named_carrier_result_ids: FxHashMap<u32, Name>,
    /// Per-slot cached register holding the buffer's element count.
    /// Preloaded from params metadata at entry so all branch arms see
    /// dominated length registers. Without this clamp PTX speculative
    /// loads from `Expr::select` arms can read past the buffer end
    /// (WGSL clamps automatically; PTX does not).
    slot_to_length_reg: FxHashMap<u32, Reg>,
    /// Binding slot lookup table. Emission performs this lookup for
    /// every memory op; keep it O(1) instead of scanning the layout.
    slot_to_binding: FxHashMap<u32, &'a BindingSlot>,
    /// True when the descriptor contains barriers or shared memory and
    /// every lane in the launched workgroup must remain live through the
    /// preamble. In this mode memory side effects use per-op bounds
    /// predicates instead of relying on an entry-wide element-count exit.
    full_workgroup_entry: bool,
    /// Sequential index of the next `MemoryOrdering::GridSync` barrier
    /// emitted in this kernel. Each whole-grid barrier `i` waits on the
    /// module-scope counter reaching `(i+1) * gridSize` (see
    /// [`Self::emit_grid_sync_barrier`]); the indices must be assigned in
    /// emission order so every CTA agrees on each barrier's release target.
    grid_barrier_index: u32,
}

impl BodyCtx<'_> {
    fn emit_predicated_store(
        &mut self,
        op: &KernelOp,
        pred: Reg,
        negate: bool,
    ) -> Result<bool, EmitError> {
        if !matches!(
            op.kind,
            KernelOpKind::StoreGlobal | KernelOpKind::StoreShared
        ) {
            return Ok(false);
        }
        let (binding_slot, index_op_id, value_op_id) = read_store_operands(op)?;
        let binding = self.binding_for_slot(binding_slot)?;
        let element_type = binding.element_type.clone();
        let memory_class = binding.memory_class;
        let elem_ty = PtxType::from_dtype(&element_type)?;
        let value_reg = self.coerce_for_store(self.lookup_operand(value_op_id)?, elem_ty);
        let address =
            self.emit_memory_address(binding_slot, index_op_id, &element_type, memory_class)?;
        let guard = self.store_guard_for_index(
            binding_slot,
            index_op_id,
            memory_class,
            Some((if negate { "@!" } else { "@" }, pred)),
        )?;
        self.emit_store_value(guard, address, &element_type, value_reg)?;
        Ok(true)
    }

    fn store_guard_for_index(
        &mut self,
        binding_slot: u32,
        index_op_id: u32,
        memory_class: MemoryClass,
        existing: Option<(&str, Reg)>,
    ) -> Result<Option<(String, Reg)>, EmitError> {
        let existing = existing.map(|(prefix, pred)| (prefix.to_string(), pred));
        if !self.full_workgroup_entry || !matches!(memory_class, MemoryClass::Global) {
            return Ok(existing);
        }

        let in_bounds = self.emit_index_in_bounds_pred(binding_slot, index_op_id)?;
        let Some((prefix, pred)) = existing else {
            return Ok(Some(("@".to_string(), in_bounds)));
        };
        let branch_live = match prefix.as_str() {
            "@" => pred,
            "@!" => {
                let not_pred = self.alloc(PtxType::Bool);
                let _ = writeln!(self.text, "    not.pred    {not_pred}, {pred};");
                not_pred
            }
            other => {
                return Err(EmitError::InvalidDescriptor(format!(
                    "unsupported PTX store guard prefix {other:?}. Fix: use @ or @! predication."
                )));
            }
        };
        let combined = self.alloc(PtxType::Bool);
        let _ = writeln!(
            self.text,
            "    and.pred    {combined}, {branch_live}, {in_bounds};"
        );
        Ok(Some(("@".to_string(), combined)))
    }

    /// Emit a whole-grid barrier (`MemoryOrdering::GridSync`) as a
    /// monotonic-counter cooperative barrier.
    ///
    /// The kernel is launched cooperatively (every CTA co-resident), so the
    /// `i`-th grid barrier releases once the module-scope counter
    /// `_vyre_grid_barrier` reaches `(i+1) * gridSize` — i.e. every CTA has
    /// arrived at barrier `i`. The counter only grows within a launch, so the
    /// strictly increasing per-barrier release targets are race-robust: a CTA
    /// that races ahead only pushes the counter higher, never below an earlier
    /// barrier's target, so no CTA is ever falsely released. The host zeroes
    /// the counter before each cooperative launch (the fixpoint loop re-launches
    /// the kernel on one serialized stream).
    ///
    /// Per-CTA `bar.sync` brackets the global phase: the first publishes the
    /// CTA's prior global writes (`membar.gl`) and converges all lanes; the
    /// per-CTA leader records arrival and spins on the counter; the second
    /// releases the CTA once the leader observes the target, and a trailing
    /// `membar.gl` acquires the other CTAs' published writes. This is the
    /// standard sense-free grid barrier and is correct only when every CTA
    /// executes the same barrier sequence — guaranteed here because GridSync
    /// barriers are top-level (never under divergent control flow) and the
    /// full-workgroup entry keeps every lane live through the barrier.
    fn emit_grid_sync_barrier(&mut self) -> Result<(), EmitError> {
        let index = self.grid_barrier_index;
        self.grid_barrier_index = index
            .checked_add(1)
            .ok_or_else(|| EmitError::InvalidDescriptor("grid-sync barrier index overflow".into()))?;
        // Release target multiplier for this barrier: (index + 1).
        let multiplier = index
            .checked_add(1)
            .ok_or_else(|| EmitError::InvalidDescriptor("grid-sync barrier target overflow".into()))?;

        let sum = self.alloc(PtxType::U32);
        let tmp = self.alloc(PtxType::U32);
        let grid = self.alloc(PtxType::U32);
        let dim = self.alloc(PtxType::U32);
        let target = self.alloc(PtxType::U32);
        let old = self.alloc(PtxType::U32);
        let cur = self.alloc(PtxType::U32);
        let bar_addr = self.alloc(PtxType::U64);
        let not_leader = self.alloc(PtxType::Bool);
        let waiting = self.alloc(PtxType::Bool);
        let spin = self.alloc_label("grid_sync_spin");
        let skip = self.alloc_label("grid_sync_skip");

        let _ = writeln!(
            self.text,
            "    // --- grid.sync barrier #{index} (monotonic counter, release at {multiplier}*gridSize) ---"
        );
        // Publish this CTA's prior global writes, then converge every lane so
        // all writes have flushed before the leader records arrival.
        let _ = writeln!(self.text, "    membar.gl;");
        let _ = writeln!(self.text, "    bar.sync 0;");
        // Per-CTA leader = (tid.x | tid.y | tid.z) == 0.
        let _ = writeln!(self.text, "    mov.u32 {sum}, %tid.x;");
        let _ = writeln!(self.text, "    mov.u32 {tmp}, %tid.y;");
        let _ = writeln!(self.text, "    or.b32 {sum}, {sum}, {tmp};");
        let _ = writeln!(self.text, "    mov.u32 {tmp}, %tid.z;");
        let _ = writeln!(self.text, "    or.b32 {sum}, {sum}, {tmp};");
        let _ = writeln!(self.text, "    setp.ne.u32 {not_leader}, {sum}, 0;");
        let _ = writeln!(self.text, "    @{not_leader} bra {skip};");
        // gridSize = nctaid.x * nctaid.y * nctaid.z.
        let _ = writeln!(self.text, "    mov.u32 {grid}, %nctaid.x;");
        let _ = writeln!(self.text, "    mov.u32 {dim}, %nctaid.y;");
        let _ = writeln!(self.text, "    mul.lo.u32 {grid}, {grid}, {dim};");
        let _ = writeln!(self.text, "    mov.u32 {dim}, %nctaid.z;");
        let _ = writeln!(self.text, "    mul.lo.u32 {grid}, {grid}, {dim};");
        let _ = writeln!(self.text, "    mul.lo.u32 {target}, {grid}, {multiplier};");
        let _ = writeln!(self.text, "    mov.u64 {bar_addr}, _vyre_grid_barrier;");
        let _ = writeln!(self.text, "    atom.global.add.u32 {old}, [{bar_addr}], 1;");
        let _ = writeln!(self.text, "{spin}:");
        let _ = writeln!(self.text, "    ld.volatile.global.u32 {cur}, [{bar_addr}];");
        let _ = writeln!(self.text, "    setp.lt.u32 {waiting}, {cur}, {target};");
        let _ = writeln!(self.text, "    @{waiting} bra {spin};");
        let _ = writeln!(self.text, "{skip}:");
        // Release the CTA once the leader has seen every CTA arrive, then
        // acquire the other CTAs' freshly published global writes.
        let _ = writeln!(self.text, "    bar.sync 0;");
        let _ = writeln!(self.text, "    membar.gl;");
        Ok(())
    }

    fn emit_op(&mut self, body: &KernelBody, op: &KernelOp) -> Result<(), EmitError> {
        use KernelOpKind::*;
        match &op.kind {
            Literal => {
                let pool_idx = *op.operands.first().ok_or_else(|| {
                    EmitError::InvalidDescriptor("Literal op missing pool index".into())
                })? as usize;
                let lit = body.literals.get(pool_idx).ok_or_else(|| {
                    EmitError::InvalidDescriptor(format!("Literal pool index {pool_idx} OOR"))
                })?;
                let (reg, lit_text) = self.alloc_literal(lit);
                let _ = writeln!(
                    self.text,
                    "    mov.{}    {reg}, {lit_text};",
                    reg.0.ptx_type_str()
                );
                self.bind_result(op, reg)?;
                if let (Some(result), LiteralValue::U32(value)) = (op.result, lit) {
                    self.u32_literals.insert(result, *value);
                }
            }
            Copy => {
                let operand_id = *op
                    .operands
                    .first()
                    .ok_or_else(|| EmitError::InvalidDescriptor("Copy missing operand".into()))?;
                let source = self.lookup_operand(operand_id)?;
                let snapshot = self.alloc(source.0);
                let _ = writeln!(
                    self.text,
                    "    mov.{}    {snapshot}, {source};",
                    source.0.ptx_type_str()
                );
                self.bind_result(op, snapshot)?;
            }
            LocalInvocationId | GlobalInvocationId => {
                let axis_idx = *op.operands.first().unwrap_or(&0);
                let reg = if matches!(op.kind, LocalInvocationId) {
                    match axis_idx {
                        0 => Reg(PtxType::U32, 2),
                        1 => Reg(PtxType::U32, 6),
                        _ => Reg(PtxType::U32, 24),
                    }
                } else {
                    match axis_idx {
                        0 => Reg(PtxType::U32, 3),
                        1 => Reg(PtxType::U32, 7),
                        _ => Reg(PtxType::U32, 25),
                    }
                };
                self.bind_result(op, reg)?;
            }
            WorkgroupId => {
                let axis_idx = *op.operands.first().unwrap_or(&0);
                let reg = match axis_idx {
                    0 => Reg(PtxType::U32, 0),
                    1 => Reg(PtxType::U32, 4),
                    _ => Reg(PtxType::U32, 8),
                };
                self.bind_result(op, reg)?;
            }
            LoadGlobal | LoadShared | LoadConstant => {
                let (binding_slot, index_op_id) = read_two_operands(op, "Load")?;
                let binding = self.binding_for_slot(binding_slot)?;
                let element_type = binding.element_type.clone();
                let memory_class = binding.memory_class;
                let elem_ty = PtxType::from_dtype(&element_type)?;
                let address = self.emit_memory_address(
                    binding_slot,
                    index_op_id,
                    &element_type,
                    memory_class,
                )?;
                let load_space = self.load_space_for(binding_slot, memory_class);
                let val_reg = self.emit_load_value(address, load_space, &element_type, elem_ty)?;
                self.bind_result(op, val_reg)?;
            }
            BufferLength => {
                let binding_slot = *op.operands.first().ok_or_else(|| {
                    EmitError::InvalidDescriptor("BufferLength missing slot".into())
                })?;
                // Checked arithmetic: slot byte offset in the params buffer is
                // `4 + slot * 4`. An overflowing slot would wrap silently and
                // produce a wrong PTX address; reject it with a structured error.
                let byte_offset = binding_slot
                    .checked_mul(4)
                    .and_then(|v| v.checked_add(4))
                    .ok_or_else(|| EmitError::InvalidBinding {
                        slot: binding_slot,
                        reason: "BufferLength slot byte offset (4 + slot * 4) overflows u32".into(),
                    })?;
                let reg = self.alloc(PtxType::U32);
                let _ = writeln!(
                    self.text,
                    "    ld.global.u32    {reg}, [%rd0 + {byte_offset}];"
                );
                self.bind_result(op, reg)?;
            }
            StoreGlobal | StoreShared => {
                let (binding_slot, index_op_id, value_op_id) = read_store_operands(op)?;
                let binding = self.binding_for_slot(binding_slot)?;
                let element_type = binding.element_type.clone();
                let memory_class = binding.memory_class;
                let elem_ty = PtxType::from_dtype(&element_type)?;
                let value_reg = self.coerce_for_store(self.lookup_operand(value_op_id)?, elem_ty);
                let address = self.emit_memory_address(
                    binding_slot,
                    index_op_id,
                    &element_type,
                    memory_class,
                )?;
                let guard =
                    self.store_guard_for_index(binding_slot, index_op_id, memory_class, None)?;
                self.emit_store_value(guard, address, &element_type, value_reg)?;
            }
            BinOpKind(bin_op) => {
                let left_id = *op
                    .operands
                    .first()
                    .ok_or_else(|| EmitError::InvalidDescriptor("BinOp missing left".into()))?;
                let right_id = *op
                    .operands
                    .get(1)
                    .ok_or_else(|| EmitError::InvalidDescriptor("BinOp missing right".into()))?;
                let left = self.lookup_operand(left_id)?;
                let right = self.lookup_operand(right_id)?;
                if matches!(bin_op, vyre_foundation::ir::BinOp::Mul) {
                    if let Some(constant) = self.u32_literals.get(&right_id).copied() {
                        if let Some(result) = self.emit_small_u32_const_mul(left, constant) {
                            self.bind_result(op, result)?;
                            return Ok(());
                        }
                    }
                    if let Some(constant) = self.u32_literals.get(&left_id).copied() {
                        if let Some(result) = self.emit_small_u32_const_mul(right, constant) {
                            self.bind_result(op, result)?;
                            return Ok(());
                        }
                    }
                }
                if matches!(bin_op, vyre_foundation::ir::BinOp::Div) {
                    if let Some(divisor) = self.u32_literals.get(&right_id).copied() {
                        if let Some(result) = self.emit_fast_u32_const_div(left, divisor) {
                            self.bind_result(op, result)?;
                            return Ok(());
                        }
                    }
                }
                if matches!(bin_op, vyre_foundation::ir::BinOp::Mod) {
                    if let Some(divisor) = self.u32_literals.get(&right_id).copied() {
                        if let Some(result) = self.emit_fast_u32_const_mod(left, divisor) {
                            self.bind_result(op, result)?;
                            return Ok(());
                        }
                    }
                }
                let (result, _result_ty) = self.emit_binop(*bin_op, left, right)?;
                self.bind_result(op, result)?;
            }
            UnOpKind(un_op) => {
                let operand_id = *op
                    .operands
                    .first()
                    .ok_or_else(|| EmitError::InvalidDescriptor("UnOp missing operand".into()))?;
                let operand = self.lookup_operand(operand_id)?;
                let result = self.emit_unop(un_op, operand)?;
                self.bind_result(op, result)?;
            }
            Return => {
                // Handled by finish_with_return; per-op Return is a no-op here.
            }
            Barrier { ordering } => {
                if ordering.requires_grid_sync() {
                    if self.options.cooperative_grid_sync {
                        self.emit_grid_sync_barrier()?;
                    } else {
                        return Err(EmitError::InvalidDescriptor(
                            "MemoryOrdering::GridSync cannot be lowered to a CTA-scope `bar.sync 0` \
                             barrier: that would silently downgrade whole-grid synchronization to \
                             one workgroup and lose cross-block visibility. Enable \
                             PtxEmitOptions::cooperative_grid_sync (only when the consumer guarantees \
                             a cooperative launch) to emit a native cooperative grid barrier, or split \
                             the program at the GridSync barrier."
                                .to_string(),
                        ));
                    }
                } else {
                    let _ = writeln!(self.text, "    bar.sync 0;");
                }
            }
            Region { generator } => {
                self.emit_region(body, op, generator)?;
            }
            StructuredBlock => {
                self.emit_structured_block(body, op)?;
            }
            StructuredIfThen => {
                self.emit_structured_if_then(body, op)?;
            }
            StructuredIfThenElse => {
                self.emit_structured_if_then_else(body, op)?;
            }
            StructuredForLoop { loop_var } => {
                self.emit_structured_for_loop(body, op, loop_var)?;
            }
            LoopIndex { loop_var } => {
                self.emit_loop_index(op, loop_var)?;
            }
            LoopCarrierInit { name } => {
                let seed_id = *op.operands.first().ok_or_else(|| {
                    EmitError::InvalidDescriptor(format!(
                        "LoopCarrierInit `{name}` missing seed operand"
                    ))
                })?;
                let seed_reg = self.lookup_operand(seed_id)?;
                let carrier = self
                    .named_carriers
                    .get(name)
                    .copied()
                    .unwrap_or_else(|| self.alloc(seed_reg.0));
                self.named_carriers.insert(name.clone(), carrier);
                let _ = writeln!(
                    self.text,
                    "    mov.{}    {carrier}, {seed_reg};",
                    carrier.0.ptx_type_str()
                );
            }
            LoopCarrier { name } => {
                let carrier = *self.named_carriers.get(name).ok_or_else(|| {
                    EmitError::InvalidDescriptor(format!(
                        "LoopCarrier `{name}` read before LoopCarrierInit allocated its register"
                    ))
                })?;
                if let Some(result_id) = op.result {
                    self.named_carrier_result_ids
                        .insert(result_id, name.clone());
                }
                let snapshot = self.alloc(carrier.0);
                let _ = writeln!(
                    self.text,
                    "    mov.{}    {snapshot}, {carrier};",
                    carrier.0.ptx_type_str()
                );
                self.bind_result(op, snapshot)?;
            }
            LoopCarrierEnd { name } => {
                let value_id = *op.operands.first().ok_or_else(|| {
                    EmitError::InvalidDescriptor(format!(
                        "LoopCarrierEnd `{name}` missing value operand"
                    ))
                })?;
                let value = self.lookup_operand(value_id)?;
                let carrier = *self.named_carriers.get(name).ok_or_else(|| {
                    EmitError::InvalidDescriptor(format!(
                        "LoopCarrierEnd `{name}` written before LoopCarrierInit allocated its register"
                    ))
                })?;
                let _ = writeln!(
                    self.text,
                    "    mov.{}    {carrier}, {value};",
                    carrier.0.ptx_type_str()
                );
            }
            Cast { target } => {
                let operand_id = *op
                    .operands
                    .first()
                    .ok_or_else(|| EmitError::InvalidDescriptor("Cast missing operand".into()))?;
                let src = self.lookup_operand(operand_id)?;
                let dst = self.emit_cast(src, target)?;
                self.bind_result(op, dst)?;
            }
            Select => {
                let cond_id = *op
                    .operands
                    .first()
                    .ok_or_else(|| EmitError::InvalidDescriptor("Select missing cond".into()))?;
                let true_id = *op.operands.get(1).ok_or_else(|| {
                    EmitError::InvalidDescriptor("Select missing true_val".into())
                })?;
                let false_id = *op.operands.get(2).ok_or_else(|| {
                    EmitError::InvalidDescriptor("Select missing false_val".into())
                })?;
                let cond = self.pred_from_boolish(self.lookup_operand(cond_id)?);
                let t = self.lookup_operand(true_id)?;
                let f = self.lookup_operand(false_id)?;
                let dst = self.alloc(t.0);
                if matches!(t.0, crate::reg::PtxType::Bool) {
                    // PTX `selp` does not accept `.pred`. Lower predicate
                    // select as: dst = (cond AND t) OR ((NOT cond) AND f).
                    let not_cond = self.alloc(crate::reg::PtxType::Bool);
                    let pick_t = self.alloc(crate::reg::PtxType::Bool);
                    let pick_f = self.alloc(crate::reg::PtxType::Bool);
                    let _ = writeln!(self.text, "    not.pred    {not_cond}, {cond};");
                    let _ = writeln!(self.text, "    and.pred    {pick_t}, {cond}, {t};");
                    let _ = writeln!(self.text, "    and.pred    {pick_f}, {not_cond}, {f};");
                    let _ = writeln!(self.text, "    or.pred    {dst}, {pick_t}, {pick_f};");
                } else {
                    let _ = writeln!(
                        self.text,
                        "    selp.{}    {dst}, {t}, {f}, {cond};",
                        t.0.ptx_type_str()
                    );
                }
                self.bind_result(op, dst)?;
            }
            AsyncLoad { tag } => {
                // Operands: [src_binding, dst_binding, offset_op_id, size_op_id]
                let src_slot = *op.operands.first().ok_or_else(|| {
                    EmitError::InvalidDescriptor("AsyncLoad missing src slot".into())
                })?;
                let dst_slot = *op.operands.get(1).ok_or_else(|| {
                    EmitError::InvalidDescriptor("AsyncLoad missing dst slot".into())
                })?;
                let offset_id = *op.operands.get(2).ok_or_else(|| {
                    EmitError::InvalidDescriptor("AsyncLoad missing offset".into())
                })?;
                let size_id = *op
                    .operands
                    .get(3)
                    .ok_or_else(|| EmitError::InvalidDescriptor("AsyncLoad missing size".into()))?;
                let _ = writeln!(
                    self.text,
                    "    // async_load tag={tag} src=slot{src_slot} dst=slot{dst_slot}"
                );
                if !self.emit_cp_async_load_loop(tag, src_slot, dst_slot, offset_id, size_id)? {
                    self.emit_async_copy_loop(
                        tag,
                        src_slot,
                        dst_slot,
                        offset_id,
                        size_id,
                        AsyncCopyDirection::Load,
                    )?;
                }
            }
            AsyncStore { tag } => {
                let src_slot = *op.operands.first().ok_or_else(|| {
                    EmitError::InvalidDescriptor("AsyncStore missing src slot".into())
                })?;
                let dst_slot = *op.operands.get(1).ok_or_else(|| {
                    EmitError::InvalidDescriptor("AsyncStore missing dst slot".into())
                })?;
                let offset_id = *op.operands.get(2).ok_or_else(|| {
                    EmitError::InvalidDescriptor("AsyncStore missing offset".into())
                })?;
                let size_id = *op.operands.get(3).ok_or_else(|| {
                    EmitError::InvalidDescriptor("AsyncStore missing size".into())
                })?;
                let _ = writeln!(
                    self.text,
                    "    // async_store tag={tag} src=slot{src_slot} dst=slot{dst_slot}"
                );
                self.emit_async_copy_loop(
                    tag,
                    src_slot,
                    dst_slot,
                    offset_id,
                    size_id,
                    AsyncCopyDirection::Store,
                )?;
            }
            AsyncWait { tag } => {
                let _ = writeln!(self.text, "    // async_wait tag={tag}");
                if !self.emit_cp_async_wait_for_tag(tag) {
                    let _ = writeln!(self.text, "    membar.cta;");
                }
            }
            SubgroupBallot => {
                let cond_id = *op.operands.first().ok_or_else(|| {
                    EmitError::InvalidDescriptor("SubgroupBallot missing cond".into())
                })?;
                let cond = self.lookup_operand(cond_id)?;
                let result = self.alloc(PtxType::U32);
                let pred = self.pred_from_boolish(cond);
                let mask = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    activemask.b32    {mask};");
                let _ = writeln!(
                    self.text,
                    "    vote.sync.ballot.b32    {result}, {pred}, {mask};"
                );
                self.bind_result(op, result)?;
            }
            // Shuffle and Broadcast both lower to `shfl.sync.idx.b32` reading
            // `value` from the lane named by `lane`. They differ only in operand
            // uniformity (broadcast's lane is uniform across the wave), which is
            // the caller's contract, not the instruction's — so the PTX is
            // identical.
            SubgroupShuffle | SubgroupBroadcast => {
                let value_id = *op.operands.first().ok_or_else(|| {
                    EmitError::InvalidDescriptor("subgroup shuffle/broadcast missing value".into())
                })?;
                let lane_id = *op.operands.get(1).ok_or_else(|| {
                    EmitError::InvalidDescriptor("subgroup shuffle/broadcast missing lane".into())
                })?;
                let value = self.lookup_operand(value_id)?;
                let lane = self.lookup_operand(lane_id)?;
                let result = self.alloc(value.0);
                let mask = self.alloc(PtxType::U32);
                let lane_mask = self.subgroup_lane_mask();
                let _ = writeln!(self.text, "    activemask.b32    {mask};");
                if value.0 == PtxType::F32 {
                    let bits = self.alloc(PtxType::U32);
                    let shuffled_bits = self.alloc(PtxType::U32);
                    let _ = writeln!(self.text, "    mov.b32    {bits}, {value};");
                    let _ = writeln!(
                        self.text,
                        "    shfl.sync.idx.b32    {shuffled_bits}, {bits}, {lane}, 0x{lane_mask:x}, {mask};"
                    );
                    let _ = writeln!(self.text, "    mov.b32    {result}, {shuffled_bits};");
                } else {
                    let _ = writeln!(
                        self.text,
                        "    shfl.sync.idx.b32    {result}, {value}, {lane}, 0x{lane_mask:x}, {mask};"
                    );
                }
                self.bind_result(op, result)?;
            }
            SubgroupReduce { op: reduce_op } => {
                let value_id = *op.operands.first().ok_or_else(|| {
                    EmitError::InvalidDescriptor("SubgroupReduce missing value".into())
                })?;
                let value = self.lookup_operand(value_id)?;
                let result = self.emit_subgroup_reduce(*reduce_op, value)?;
                self.bind_result(op, result)?;
            }
            SubgroupLocalId => {
                let result = self.alloc(PtxType::U32);
                let _ = writeln!(self.text, "    mov.u32    {result}, %laneid;");
                self.bind_result(op, result)?;
            }
            SubgroupSize => {
                let result = self.alloc(PtxType::U32);
                let subgroup_size = self.options.subgroup_size;
                let _ = writeln!(self.text, "    mov.u32    {result}, {subgroup_size};");
                self.bind_result(op, result)?;
            }
            Atomic {
                op: atomic_op,
                ordering: _,
            } => {
                self.emit_atomic(op, *atomic_op)?;
            }
            Fma => {
                let a_id = *op
                    .operands
                    .first()
                    .ok_or_else(|| EmitError::InvalidDescriptor("Fma missing a".into()))?;
                let b_id = *op
                    .operands
                    .get(1)
                    .ok_or_else(|| EmitError::InvalidDescriptor("Fma missing b".into()))?;
                let c_id = *op
                    .operands
                    .get(2)
                    .ok_or_else(|| EmitError::InvalidDescriptor("Fma missing c".into()))?;
                let a = self.lookup_operand(a_id)?;
                let b = self.lookup_operand(b_id)?;
                let c = self.lookup_operand(c_id)?;
                let dst = self.alloc(a.0);
                let _ = writeln!(
                    self.text,
                    "    fma.rn.{}    {dst}, {a}, {b}, {c};",
                    a.0.ptx_type_str()
                );
                self.bind_result(op, dst)?;
            }
            MatrixMma {
                shape,
                a_layout,
                b_layout,
                a_type,
                b_type,
                accum_type,
            } => {
                let outputs = self.emit_matrix_mma(
                    op,
                    *shape,
                    *a_layout,
                    *b_layout,
                    *a_type,
                    *b_type,
                    *accum_type,
                )?;
                self.bind_consecutive_results(op, &outputs)?;
            }
            Trap { tag } => {
                let address_id = *op
                    .operands
                    .first()
                    .ok_or_else(|| EmitError::InvalidDescriptor("Trap missing address".into()))?;
                let _address = self.lookup_operand(address_id)?;
                let _ = writeln!(self.text, "    // trap tag: {tag}");
                let _ = writeln!(self.text, "    bra $L_exit;");
            }
            Resume { tag } => {
                let _ = writeln!(self.text, "    // resume tag: {tag}");
            }
            IndirectDispatch { .. } => {
                return Err(EmitError::UnsupportedOp(KernelOp {
                    kind: op.kind.clone(),
                    operands: op.operands.clone(),
                    result: op.result,
                }));
            }
            other => {
                return Err(EmitError::UnsupportedOp(KernelOp {
                    kind: other.clone(),
                    operands: op.operands.clone(),
                    result: op.result,
                }));
            }
        }
        Ok(())
    }
}

pub(super) fn body_descendants_read_operand(body: &KernelBody, result_id: u32) -> bool {
    body.child_bodies
        .iter()
        .any(|child| body_reads_operand_recursive(child, result_id))
}

fn body_reads_operand_recursive(body: &KernelBody, result_id: u32) -> bool {
    body.ops.iter().any(|op| {
        op.operands.iter().enumerate().any(|(pos, &operand)| {
            operand == result_id && classify_operand(&op.kind, pos) == OperandClass::ResultRef
        })
    }) || body
        .child_bodies
        .iter()
        .any(|child| body_reads_operand_recursive(child, result_id))
}
