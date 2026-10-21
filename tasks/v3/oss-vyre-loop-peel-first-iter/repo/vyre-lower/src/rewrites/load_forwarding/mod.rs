//! Store-to-load and load-to-load forwarding.
//!
//! When a `Load(slot, idx)` follows a `Store(slot, idx, val)` (or an
//! earlier `Load(slot, idx) -> r`) with no intervening op that could
//! invalidate `(slot, idx)`, the second load's result-id is rewritten
//! to point at the prior value-id. The Load op itself is left in place;
//! `descriptor_dce` removes it if it becomes unused.
//!
//! ## Invalidation
//!
//! Per-slot tracking. Any of these intervening ops invalidate mutable-cache
//! entries for the relevant slot (or all mutable slots, when scope is unknown):
//! - `Store*` to the same slot at a different index  -  may alias the
//!   tracked index, so the cached value is no longer guaranteed.
//! - `Atomic`  -  read-modify-writes can change the value.
//! - `Barrier`  -  re-publishes other threads' writes.
//! - `Async{Load,Store,Wait}`  -  staged writes may land asynchronously.
//! - Structured control flow (`If`/`ForLoop`/`Block`/`Region`)  -  body
//!   could write anywhere.
//! - `Trap`/`Resume`/`Return`  -  terminator.
//! - `Call`/`OpaqueExpr`/`OpaqueNode`  -  opaque side effects.
//!
//! ## Why two flavors at once
//!
//! Both store-to-load (`store; load → forward`) and load-to-load
//! (`load r1; load r2 → r2 := r1`) share the same per-slot cache; doing
//! one without the other would leave easy redundancy on the table.

use super::dataflow_facts::resolve_remapped_reaching_def_id as resolve;
use super::literal::u32_literals_by_result;
use super::memory_address::{
    address_key, locations_may_alias, AddressKey, MemoryLocation, MemorySpace, MemoryTarget,
    SlotAliasPolicy,
};
use crate::operand_semantics::operand_is_result_reference;
use crate::{KernelBody, KernelDescriptor, KernelOpKind};
use rustc_hash::FxHashMap;

#[must_use]
pub fn load_forwarding(desc: &KernelDescriptor) -> KernelDescriptor {
    load_forwarding_with_optional_dataflow_facts(desc, None, None)
}

#[must_use]
pub fn load_forwarding_with_alias_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::alias_facts::AliasFactSet,
) -> KernelDescriptor {
    load_forwarding_with_optional_dataflow_facts(desc, Some(alias_facts), None)
}

#[must_use]
pub fn load_forwarding_with_weir_alias_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::weir_alias::AliasFactSet,
) -> KernelDescriptor {
    load_forwarding_with_alias_facts(desc, alias_facts)
}

#[must_use]
pub fn load_forwarding_with_dataflow_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::alias_facts::AliasFactSet,
    reaching_defs: &crate::analyses::reaching_def_facts::ReachingDefFactSet,
) -> KernelDescriptor {
    load_forwarding_with_optional_dataflow_facts(desc, Some(alias_facts), Some(reaching_defs))
}

#[must_use]
pub fn load_forwarding_with_dataflow_analysis_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::weir_alias::AliasFactSet,
    reaching_defs: &crate::analyses::weir_reaching_def::ReachingDefFactSet,
) -> KernelDescriptor {
    load_forwarding_with_dataflow_facts(desc, alias_facts, reaching_defs)
}

fn load_forwarding_with_optional_dataflow_facts(
    desc: &KernelDescriptor,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = load_forwarding_body(out.body, alias_facts, reaching_defs);
    out
}

fn load_forwarding_body(
    mut body: KernelBody,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> KernelBody {
    let literal_values = u32_literals_by_result(&body);
    // Per memory-space + slot map: (space, slot_id) → (address → val_id).
    let mut cache: FxHashMap<MemoryTarget, FxHashMap<AddressKey, CachedValue>> =
        FxHashMap::default();
    let mut id_remap: FxHashMap<u32, u32> = FxHashMap::default();

    for op in &body.ops {
        match &op.kind {
            KernelOpKind::StoreGlobal | KernelOpKind::StoreShared => {
                if op.operands.len() < 3 {
                    continue;
                }
                let target = match &op.kind {
                    KernelOpKind::StoreGlobal => MemoryTarget::global(op.operands[0]),
                    KernelOpKind::StoreShared => MemoryTarget::shared(op.operands[0]),
                    _ => continue,
                };
                let idx = resolve(op.operands[1], &id_remap, reaching_defs);
                let val = resolve(op.operands[2], &id_remap, reaching_defs);
                let address = address_key(idx, &literal_values);
                invalidate_cache_for_write(&mut cache, target, idx, address, alias_facts);
                cache.entry(target).or_default().insert(
                    address,
                    CachedValue {
                        index_operand: idx,
                        value_id: val,
                    },
                );
            }
            KernelOpKind::LoadGlobal | KernelOpKind::LoadShared => {
                if op.operands.len() < 2 {
                    continue;
                }
                let target = match &op.kind {
                    KernelOpKind::LoadGlobal => MemoryTarget::global(op.operands[0]),
                    KernelOpKind::LoadShared => MemoryTarget::shared(op.operands[0]),
                    _ => continue,
                };
                let idx = resolve(op.operands[1], &id_remap, reaching_defs);
                let address = address_key(idx, &literal_values);
                let Some(load_result) = op.result else {
                    continue;
                };
                if let Some(entry) = cache.get_mut(&target) {
                    if let Some(cached) = entry.get(&address) {
                        // Forward: rewrite this load's result-id refs to point
                        // at the cached value.
                        id_remap.insert(load_result, cached.value_id);
                        // Don't update the cache  -  the cached value is the
                        // canonical id; this load is now redundant.
                        continue;
                    }
                    entry.insert(
                        address,
                        CachedValue {
                            index_operand: idx,
                            value_id: load_result,
                        },
                    );
                } else {
                    let mut entry = FxHashMap::default();
                    entry.insert(
                        address,
                        CachedValue {
                            index_operand: idx,
                            value_id: load_result,
                        },
                    );
                    cache.insert(target, entry);
                }
            }
            KernelOpKind::LoadConstant => {
                // Constants are immutable  -  perfectly safe to forward.
                if op.operands.len() < 2 {
                    continue;
                }
                let target = MemoryTarget::constant(op.operands[0]);
                let idx = resolve(op.operands[1], &id_remap, reaching_defs);
                let address = address_key(idx, &literal_values);
                let Some(load_result) = op.result else {
                    continue;
                };
                if let Some(entry) = cache.get_mut(&target) {
                    if let Some(cached) = entry.get(&address) {
                        id_remap.insert(load_result, cached.value_id);
                        continue;
                    }
                    entry.insert(
                        address,
                        CachedValue {
                            index_operand: idx,
                            value_id: load_result,
                        },
                    );
                } else {
                    let mut entry = FxHashMap::default();
                    entry.insert(
                        address,
                        CachedValue {
                            index_operand: idx,
                            value_id: load_result,
                        },
                    );
                    cache.insert(target, entry);
                }
            }
            KernelOpKind::Atomic { .. } => {
                if let (Some(&slot), Some(&idx)) = (op.operands.first(), op.operands.get(1)) {
                    let resolved_idx = resolve(idx, &id_remap, reaching_defs);
                    let address = address_key(resolved_idx, &literal_values);
                    invalidate_cache_for_write(
                        &mut cache,
                        MemoryTarget::global(slot),
                        resolved_idx,
                        address,
                        alias_facts,
                    );
                } else if let Some(&slot) = op.operands.first() {
                    cache.remove(&MemoryTarget::global(slot));
                } else {
                    clear_mutable_cache(&mut cache);
                }
            }
            KernelOpKind::Barrier { .. } => {
                // Barrier republishes other threads' writes  -  keep constant forwarding.
                clear_mutable_cache(&mut cache);
            }
            KernelOpKind::AsyncLoad { .. }
            | KernelOpKind::AsyncStore { .. }
            | KernelOpKind::AsyncWait { .. } => {
                clear_mutable_cache(&mut cache);
            }
            KernelOpKind::StructuredIfThen
            | KernelOpKind::StructuredIfThenElse
            | KernelOpKind::StructuredForLoop { .. }
            | KernelOpKind::StructuredBlock
            | KernelOpKind::Region { .. } => {
                clear_mutable_cache(&mut cache);
            }
            KernelOpKind::Trap { .. } | KernelOpKind::Resume { .. } | KernelOpKind::Return => {
                clear_mutable_cache(&mut cache);
            }
            KernelOpKind::Call { .. }
            | KernelOpKind::OpaqueExpr(..)
            | KernelOpKind::OpaqueNode(..) => {
                clear_mutable_cache(&mut cache);
            }
            // IndirectDispatch READS count_buffer[count_offset] but writes no
            // buffer memory in this kernel, so cached load values stay valid: a
            // read never makes a forwarded value stale. (Contrast dead-store
            // elimination, where this op's READ keeps a store live.)
            KernelOpKind::IndirectDispatch { .. } => {}
            // Pure value/builtin/arith ops and function-local loop-carrier
            // slots: no global/shared/constant buffer WRITE, so they cannot
            // stale a cached load. EXHAUSTIVE ON PURPOSE (no `_` wildcard): a
            // future `KernelOpKind` that writes buffer memory must be added here
            // as an invalidation rather than silently defaulting to "no effect"
            // and forwarding a stale value past it (a miscompile).
            KernelOpKind::Literal
            | KernelOpKind::Copy
            | KernelOpKind::LocalInvocationId
            | KernelOpKind::GlobalInvocationId
            | KernelOpKind::WorkgroupId
            | KernelOpKind::SubgroupLocalId
            | KernelOpKind::SubgroupSize
            | KernelOpKind::LoopIndex { .. }
            | KernelOpKind::LoopCarrierInit { .. }
            | KernelOpKind::LoopCarrier { .. }
            | KernelOpKind::LoopCarrierEnd { .. }
            | KernelOpKind::BufferLength
            | KernelOpKind::BinOpKind(_)
            | KernelOpKind::UnOpKind(_)
            | KernelOpKind::Fma
            | KernelOpKind::MatrixMma { .. }
            | KernelOpKind::Select
            | KernelOpKind::Cast { .. }
            | KernelOpKind::SubgroupBallot
            | KernelOpKind::SubgroupShuffle
            | KernelOpKind::SubgroupBroadcast
            | KernelOpKind::SubgroupReduce { .. } => {}
        }
    }

    if !id_remap.is_empty() {
        for op in &mut body.ops {
            for pos in 0..op.operands.len() {
                if operand_is_result_reference(&op.kind, pos) {
                    if let Some(&new) = id_remap.get(&op.operands[pos]) {
                        op.operands[pos] = new;
                    }
                }
            }
        }
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| load_forwarding_body(child, alias_facts, reaching_defs))
        .collect();
    body
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CachedValue {
    index_operand: u32,
    value_id: u32,
}

fn invalidate_cache_for_write(
    cache: &mut FxHashMap<MemoryTarget, FxHashMap<AddressKey, CachedValue>>,
    write_target: MemoryTarget,
    write_index: u32,
    write_address: AddressKey,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
) {
    let Some(entries) = cache.get_mut(&write_target) else {
        return;
    };

    entries.retain(|cached_address, cached| {
        let cached_location =
            MemoryLocation::new(write_target, cached.index_operand, *cached_address);
        let write_location = MemoryLocation::new(write_target, write_index, write_address);
        !locations_may_alias(
            cached_location,
            write_location,
            alias_facts,
            SlotAliasPolicy::DistinctSlotsNeverAlias,
        )
    });

    if entries.is_empty() {
        cache.remove(&write_target);
    }
}

fn clear_mutable_cache(cache: &mut FxHashMap<MemoryTarget, FxHashMap<AddressKey, CachedValue>>) {
    cache.retain(|target, _| matches!(target.space, MemorySpace::Constant));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };
    use vyre_foundation::ir::DataType;

    fn rw_slot() -> BindingSlot {
        rw_slot_numbered(0)
    }

    fn rw_slot_numbered(slot: u32) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: format!("buf{slot}"),
        }
    }

    #[test]
    fn store_then_load_forwards_value() {
        // r0=Lit(0) (idx), r1=Lit(7) (val), Store(buf, r0, r1), r2=Load(buf, r0),
        // Store(buf, r0, r2). The Load should forward r2 := r1.
        let desc = KernelDescriptor {
            id: "stl".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2], // value should now resolve to r1
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let out = load_forwarding(&desc);
        // The trailing store's value-operand was 2 → should now be 1.
        assert_eq!(out.body.ops[4].operands, vec![0, 0, 1]);
    }

    #[test]
    fn external_no_alias_fact_preserves_forwarding_across_unrelated_store() {
        let desc = KernelDescriptor {
            id: "alias_aware_stlf".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 3],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(4),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 4],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7), LiteralValue::U32(9)],
            },
        };
        let conservative = load_forwarding(&desc);
        assert_eq!(conservative.body.ops[7].operands, vec![0, 0, 4]);

        let mut facts = crate::analyses::alias_facts::AliasFactSet::default();
        facts.insert_no_alias(crate::analyses::alias_facts::NoAliasFact {
            left_binding: 0,
            left_index: 0,
            right_binding: 0,
            right_index: 1,
        });
        let alias_aware = load_forwarding_with_alias_facts(&desc, &facts);
        assert_eq!(alias_aware.body.ops[7].operands, vec![0, 0, 2]);
    }

    #[test]
    fn different_binding_store_invalidates_forwarding_without_external_no_alias_fact() {
        let desc = KernelDescriptor {
            id: "cross_binding_alias_conservative_stlf".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot_numbered(0), rw_slot_numbered(1)],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],

                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![1, 1, 2],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(4),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 4],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(0),
                    LiteralValue::U32(1),
                    LiteralValue::U32(9),
                ],
            },
        };

        // Slot 0 and slot 1 are distinct GPU buffer bindings  -  a store
        // to slot 1 cannot alias slot 0. The load at ops[5] (LoadGlobal
        // slot 0, idx 0) should forward to the earlier Load result r3,
        // making ops[6] use r3 instead of r4.
        let conservative = load_forwarding(&desc);
        assert_eq!(conservative.body.ops[6].operands, vec![0, 0, 3]);

        let mut facts = crate::analyses::alias_facts::AliasFactSet::default();
        facts.insert_no_alias(crate::analyses::alias_facts::NoAliasFact {
            left_binding: 0,
            left_index: 0,
            right_binding: 1,
            right_index: 1,
        });
        let alias_aware = load_forwarding_with_alias_facts(&desc, &facts);
        assert_eq!(alias_aware.body.ops[6].operands, vec![0, 0, 3]);
    }

    #[test]
    fn global_and_shared_slots_do_not_forward_across_memory_spaces() {
        let desc = KernelDescriptor {
            id: "stlf_memory_space_separation".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadShared,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreShared,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };

        let out = load_forwarding(&desc);
        assert_eq!(
            out.body.ops[3].operands,
            vec![0, 0, 2],
            "LoadShared must not forward from a same-slot LoadGlobal cache entry"
        );
    }

    #[test]
    fn load_then_load_same_idx_forwards() {
        let desc = KernelDescriptor {
            id: "ll".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let out = load_forwarding(&desc);
        // Store value should resolve to r1 (the first load), not r2.
        assert_eq!(out.body.ops[3].operands, vec![0, 0, 1]);
    }

    #[test]
    fn store_to_different_const_index_does_not_flush_forwardable_load() {
        let desc = KernelDescriptor {
            id: "different_const_store_keeps_load_cache".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 2],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(4),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 4],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(0),
                    LiteralValue::U32(1),
                    LiteralValue::U32(9),
                ],
            },
        };

        let out = load_forwarding(&desc);
        assert_eq!(out.body.ops[6].operands, vec![0, 1, 3]);
    }

    #[test]
    fn cache_keys_resolve_through_prior_load_forwarding() {
        let desc = KernelDescriptor {
            id: "resolved_idx".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 3],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 2],
                        result: Some(4),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 4],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(9)],
            },
        };

        let out = load_forwarding(&desc);
        assert_eq!(
            out.body.ops[6].operands,
            vec![0, 1, 3],
            "load at a remapped-equivalent index should forward from the earlier store"
        );
    }

    #[test]
    fn pure_op_between_store_and_load_does_not_invalidate_cache() {
        // A pure op (here `Copy`) between a store and a load of the SAME address
        // must NOT clear the forward cache — pure ops write no buffer memory.
        // The exhaustive-match change lists pure ops explicitly as no-effect
        // (replacing the optimistic `_ => {}` wildcard); this pins that contract
        // so a future op that wrongly clears the cache (blocking a sound
        // forward) — or worse, a memory-writer silently treated as no-effect —
        // is caught.
        let desc = KernelDescriptor {
            id: "pure_between_store_load".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    }, // index
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    }, // stored value
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1], // slot0[idx0] = val1
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::Copy,
                        operands: vec![1], // pure: snapshot val1
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0], // load slot0[idx0]
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 3], // uses the load result so the forward is observable
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(5), LiteralValue::U32(7)],
            },
        };
        let out = load_forwarding(&desc);
        // The load (result 3) forwards the stored value (id 1) across the pure
        // Copy, so the final store's value operand is remapped 3 -> 1.
        assert_eq!(
            out.body.ops.last().unwrap().operands,
            vec![0, 0, 1],
            "pure op must not invalidate the cache; the load must forward the stored value across it"
        );
    }

    #[test]
    fn intervening_store_to_different_idx_invalidates_cache() {
        let desc = KernelDescriptor {
            id: "intervening".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    }, // idx_a
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    }, // val
                    KernelOp {
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![0],
                        result: Some(2),
                    }, // idx_b is dynamic and may alias idx_a
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![3],
                        result: Some(3),
                    }, // val_b
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1], // store val at idx_a
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 2, 3], // dynamic idx_b may alias idx_a
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0], // load idx_a
                        result: Some(4),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 4],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(0),
                    LiteralValue::U32(7),
                    LiteralValue::U32(8),
                ],
            },
        };
        let out = load_forwarding(&desc);
        // The dynamic second store may alias idx_a, so it must wipe the cached idx_a load.
        // The load(idx_a) is no longer in cache → no forwarding → trailing store keeps r4.
        assert_eq!(out.body.ops[7].operands, vec![0, 0, 4]);
    }

    #[test]
    fn barrier_invalidates_cache() {
        let desc = KernelDescriptor {
            id: "barrier".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::Barrier {
                            ordering:
                                vyre_foundation::runtime::memory_model::MemoryOrdering::SeqCst,
                        },
                        operands: vec![],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let out = load_forwarding(&desc);
        // Barrier wiped cache; trailing store still references r2.
        assert_eq!(out.body.ops[5].operands, vec![0, 0, 2]);
    }

    #[test]
    fn different_slots_dont_interfere() {
        let mut s1 = rw_slot();
        s1.slot = 1;
        s1.name = "buf2".into();
        let desc = KernelDescriptor {
            id: "twoslots".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot(), s1],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    }, // idx
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    }, // val_a
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(2),
                    }, // val_b
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1], // slot 0
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![1, 0, 2], // slot 1  -  different slot, doesn't invalidate slot 0
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0], // slot 0, idx 0  -  should forward to val_a (r1)
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 3],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(0),
                    LiteralValue::U32(7),
                    LiteralValue::U32(99),
                ],
            },
        };
        let out = load_forwarding(&desc);
        assert_eq!(out.body.ops[6].operands, vec![0, 0, 1]);
    }

    #[test]
    fn atomic_invalidates_only_target_slot() {
        let desc = KernelDescriptor {
            id: "atomic_slot".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::Atomic {
                            op: vyre_foundation::ir::AtomicOp::Add,
                            ordering:
                                vyre_foundation::runtime::memory_model::MemoryOrdering::SeqCst,
                        },
                        operands: vec![0, 0, 1], // slot 0
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 3],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let out = load_forwarding(&desc);
        // Atomic on slot 0 invalidated cache; load can't forward.
        assert_eq!(out.body.ops[5].operands, vec![0, 0, 3]);
    }

    #[test]
    fn structured_if_invalidates_cache() {
        let desc = KernelDescriptor {
            id: "if_invalidates".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                    // condition operand
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![2, 0], // cond=r2, body-child-idx=0
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 3],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let out = load_forwarding(&desc);
        assert_eq!(out.body.ops[6].operands, vec![0, 0, 3]);
    }

    #[test]
    fn nothing_to_forward_is_noop() {
        let desc = KernelDescriptor {
            id: "noop".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let out = load_forwarding(&desc);
        assert_eq!(out.body.ops.len(), 3);
        assert_eq!(out.body.ops[2].operands, vec![0, 0, 1]);
    }

    #[test]
    fn loadconstant_is_forwardable_across_arbitrary_ops() {
        let desc = KernelDescriptor {
            id: "lc".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadConstant,
                        operands: vec![0, 0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadConstant,
                        operands: vec![0, 0], // same (slot, idx)  -  should forward
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let out = load_forwarding(&desc);
        assert_eq!(out.body.ops[3].operands, vec![0, 0, 1]);
    }

    #[test]
    fn loadconstant_survives_barrier_and_reuses_cache() {
        let desc = KernelDescriptor {
            id: "lc_barrier".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadConstant,
                        operands: vec![0, 0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::Barrier {
                            ordering:
                                vyre_foundation::runtime::memory_model::MemoryOrdering::SeqCst,
                        },
                        operands: vec![],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadConstant,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let out = load_forwarding(&desc);
        assert_eq!(out.body.ops[4].operands, vec![0, 0, 1]);
    }

    #[test]
    fn idempotent() {
        let desc = KernelDescriptor {
            id: "idemp".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let once = load_forwarding(&desc);
        let twice = load_forwarding(&once);
        assert_eq!(once.body.ops, twice.body.ops);
    }

    #[test]
    fn empty_kernel_is_noop() {
        let desc = KernelDescriptor {
            id: "empty".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let out = load_forwarding(&desc);
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn reaching_def_facts_forward_equivalent_index_results() {
        let desc = KernelDescriptor {
            id: "reaching_def_forward".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::Copy,
                        operands: vec![0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 2],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 3],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7)],
            },
        };
        let mut reaching = crate::analyses::reaching_def_facts::ReachingDefFactSet::default();
        reaching.set_reaching_defs(2, vec![0]);
        let aliases = crate::analyses::alias_facts::AliasFactSet::default();
        let out = load_forwarding_with_dataflow_facts(&desc, &aliases, &reaching);
        assert_eq!(out.body.ops[5].operands, vec![0, 0, 1]);
    }

    #[test]
    fn dataflow_analysis_facts_forward_equivalent_index_results() {
        let desc = KernelDescriptor {
            id: "dataflow_analysis_forward".into(),
            bindings: BindingLayout {
                slots: vec![rw_slot()],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::Copy,
                        operands: vec![0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 2],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 3],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7)],
            },
        };
        let mut reaching = crate::analyses::weir_reaching_def::ReachingDefFactSet::default();
        reaching.set_reaching_defs(2, vec![0]);
        let aliases = crate::analyses::weir_alias::AliasFactSet::default();
        let out = load_forwarding_with_dataflow_analysis_facts(&desc, &aliases, &reaching);
        assert_eq!(out.body.ops[5].operands, vec![0, 0, 1]);
    }
}
