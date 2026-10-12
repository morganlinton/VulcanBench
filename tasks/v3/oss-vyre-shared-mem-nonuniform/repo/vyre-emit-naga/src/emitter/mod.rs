//! Per-function emit state. Everything below the `BodyBuilder`
//! boundary lives in topic-specific files (op_dispatch, coercion,
//! binding_helpers, scalar_ops, async_op, atomic, loops, binop,
//! op_lookup, setup). This module just defines `BodyBuilder` and
//! re-exports the entry point.

mod async_op;
mod atomic;
mod binding_helpers;
mod binop;
mod coercion;
mod loop_carriers;
mod loops;
mod op_dispatch;
mod op_lookup;
mod scalar_ops;
mod setup;
mod subgroup;

#[cfg(test)]
pub(crate) use op_dispatch::op_dispatch_route_cache_probe;
pub(crate) use setup::emit_uncached;
use setup::{Builtins, TypeHandles};

use rustc_hash::{FxHashMap, FxHashSet};

use naga::{Expression, Function, GlobalVariable, LocalVariable, Type};
use vyre_foundation::ir::DataType;

struct BodyBuilder<'a> {
    function: &'a mut Function,
    values: FxHashMap<u32, naga::Handle<Expression>>,
    value_types: FxHashMap<u32, naga::Handle<Type>>,
    globals: &'a FxHashMap<u32, naga::Handle<GlobalVariable>>,
    binding_types: &'a FxHashMap<u32, naga::Handle<Type>>,
    binding_counts: &'a FxHashMap<u32, Option<u32>>,
    /// Source-level `DataType` per binding slot. Distinguishes
    /// byte-element bindings (U8/I8) from word-element bindings (U32/I32)
    /// at op-emit time so packed-byte loads honor the byte-addressed
    /// semantics the reference evaluator uses. Without this, a
    /// `Load("source": U8, addr)` returns the u32 word at index `addr`
    /// (since WGSL has no native byte storage), not the byte at byte
    /// address `addr`.
    binding_data_types: &'a FxHashMap<u32, DataType>,
    builtins: Builtins,
    types: TypeHandles,
    loop_locals: FxHashMap<vyre_lower::descriptor::Name, naga::Handle<LocalVariable>>,
    loop_types: FxHashMap<vyre_lower::descriptor::Name, naga::Handle<Type>>,
    /// Q7 fix: result ids that need a function-scope `LocalVariable`
    /// carrier because they are produced inside a `StructuredForLoop`
    /// child body and referenced after the loop in the parent body.
    /// Naga's expression-scoping rejects the dangling SSA reference
    /// otherwise (`no definition in scope for identifier _e37`).
    /// Populated by `emit_structured_for_loop` before entering the
    /// loop, drained when the loop emit completes.
    loop_carrier_targets: FxHashSet<u32>,
    /// Lazily-allocated `LocalVariable` per loop-carried id.
    loop_carrier_locals: FxHashMap<u32, naga::Handle<LocalVariable>>,
    /// Depth of nested `child_block` swaps. > 0 means the current
    /// `self.function.body` is a child block (loop body, if-then arm,
    /// continuing block, etc.)  -  values bound here have their
    /// `Statement::Emit` in a closed scope from the perspective of
    /// outer-block readers. `bind_result` publishes via a function-
    /// scope `LocalVariable` whenever depth > 0 so `value_handle_for_id`
    /// can re-Load the value in the consumer's current block.
    pub(super) child_body_depth: usize,
    /// Block-scoped locals: for ANY value produced inside a child block,
    /// store to a function-local so it can be re-Loaded in a different
    /// block. This is more conservative than loop_carriers (which only
    /// covers values explicitly identified as loop-carried) and catches
    /// cases where the carrier analysis misses a value that escapes its
    /// birth block.
    block_scoped_locals: FxHashMap<u32, naga::Handle<LocalVariable>>,
    /// Function-scope `LocalVariable` per source-level loop-carrier
    /// name. Allocated by `LoopCarrierInit`, written by
    /// `LoopCarrierEnd`, read by `LoopCarrier`. Survives across loop
    /// iterations and post-loop reads.
    named_carrier_locals: FxHashMap<vyre_lower::descriptor::Name, naga::Handle<LocalVariable>>,
    /// Recorded scalar type per named carrier (decided at init time
    /// from the seed expression). Subsequent reads coerce loaded values
    /// to this type so consumers do not see Bool-vs-u32 mismatches.
    named_carrier_types: FxHashMap<vyre_lower::descriptor::Name, naga::Handle<Type>>,
    /// Result-id → carrier name mapping for `LoopCarrier` reads. When
    /// a downstream op references one of these ids, `value_handle_for_id`
    /// emits a fresh `Load` from the named carrier local directly in
    /// the consumer's current block, bypassing the block-scoped local
    /// publish path. This is required because wgpu/naga's downstream
    /// optimizers can hoist a "load-once-per-iteration into a
    /// block-scoped local" pattern out of the loop on shaders where the
    /// block-scoped local has a single writer; routing each read
    /// directly to the carrier local forces a per-consumer-site load.
    named_carrier_result_ids: FxHashMap<u32, vyre_lower::descriptor::Name>,
    trap_sidecar_slot: Option<u32>,
    trap_tag_codes: FxHashMap<vyre_lower::descriptor::Name, u32>,
    op_dispatch_routes: op_dispatch::OpDispatchRouteCache,
}
