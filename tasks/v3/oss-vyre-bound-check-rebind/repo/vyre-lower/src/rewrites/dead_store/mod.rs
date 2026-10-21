//! Dead-store elimination.
//!
//! When two `StoreGlobal` (or `StoreShared`) ops in sequence write to
//! the same binding-slot AND the same index-operand-id, AND no
//! intervening op reads that location (or any aliased location), the
//! first store is dead  -  its value gets immediately overwritten and
//! never observed.
//!
//! Phase-1 conservative rules:
//! - Same binding slot for overwrite proof.
//! - Same index operand id (textual equality  -  no commutative-index
//!   normalization).
//! - No intervening LoadGlobal/LoadShared from the same binding.
//! - No intervening structured-control-flow op (an if/loop body might
//!   read the location; phase 1 stays out of those).
//! - No intervening Barrier or Atomic on the same slot.
//!
//! External alias and reaching-definition facts make the pass
//! more aggressive without weakening safety: no-alias facts prevent
//! unrelated reads from invalidating pending stores, and single
//! reaching-def facts canonicalize equivalent descriptor index ids.

use super::dataflow_facts::resolve_reaching_def_id as resolve;
use super::literal::u32_literals_by_result;
use super::memory_address::{
    address_key, locations_may_alias, MemoryLocation, MemorySpace, MemoryTarget, SlotAliasPolicy,
};
use crate::{KernelBody, KernelDescriptor, KernelOp, KernelOpKind};
use rustc_hash::FxHashMap;

#[must_use]
pub fn dead_store(desc: &KernelDescriptor) -> KernelDescriptor {
    dead_store_with_optional_dataflow_facts(desc, None, None)
}

#[must_use]
pub fn dead_store_with_alias_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::alias_facts::AliasFactSet,
) -> KernelDescriptor {
    dead_store_with_optional_dataflow_facts(desc, Some(alias_facts), None)
}

#[must_use]
pub fn dead_store_with_weir_alias_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::weir_alias::AliasFactSet,
) -> KernelDescriptor {
    dead_store_with_alias_facts(desc, alias_facts)
}

#[must_use]
pub fn dead_store_with_dataflow_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::alias_facts::AliasFactSet,
    reaching_defs: &crate::analyses::reaching_def_facts::ReachingDefFactSet,
) -> KernelDescriptor {
    dead_store_with_optional_dataflow_facts(desc, Some(alias_facts), Some(reaching_defs))
}

#[must_use]
pub fn dead_store_with_dataflow_analysis_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::weir_alias::AliasFactSet,
    reaching_defs: &crate::analyses::weir_reaching_def::ReachingDefFactSet,
) -> KernelDescriptor {
    dead_store_with_dataflow_facts(desc, alias_facts, reaching_defs)
}

fn dead_store_with_optional_dataflow_facts(
    desc: &KernelDescriptor,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = dead_store_body(out.body, alias_facts, reaching_defs);
    out
}

fn dead_store_body(
    mut body: KernelBody,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> KernelBody {
    let mut keep = vec![true; body.ops.len()];
    let mut pending_stores = FxHashMap::<MemoryLocation, usize>::default();
    let literal_values = u32_literals_by_result(&body);

    for (op_index, op) in body.ops.iter().enumerate() {
        match store_key(op, &literal_values, reaching_defs) {
            Some(key) => {
                if let Some(previous_store) = pending_stores.insert(key, op_index) {
                    keep[previous_store] = false;
                }
            }
            None => invalidate_pending_stores(
                &mut pending_stores,
                op,
                &literal_values,
                alias_facts,
                reaching_defs,
            ),
        }
    }

    let old_ops = std::mem::take(&mut body.ops);
    body.ops = old_ops
        .into_iter()
        .enumerate()
        .filter_map(|(i, op)| keep[i].then_some(op))
        .collect();

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| dead_store_body(child, alias_facts, reaching_defs))
        .collect();

    body
}

fn store_key(
    op: &KernelOp,
    literal_values: &FxHashMap<u32, u32>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> Option<MemoryLocation> {
    let target = match op.kind {
        KernelOpKind::StoreGlobal => MemoryTarget::global(*op.operands.first()?),
        KernelOpKind::StoreShared => MemoryTarget::shared(*op.operands.first()?),
        _ => return None,
    };
    let index = resolve(*op.operands.get(1)?, reaching_defs);
    Some(MemoryLocation::new(
        target,
        index,
        address_key(index, literal_values),
    ))
}

fn invalidate_pending_stores(
    pending_stores: &mut FxHashMap<MemoryLocation, usize>,
    op: &KernelOp,
    literal_values: &FxHashMap<u32, u32>,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) {
    match invalidated_store_scope(op, literal_values, reaching_defs) {
        Invalidation::None => {}
        Invalidation::OnlySpace(space) => {
            pending_stores.retain(|key, _| key.target.space != space);
        }
        Invalidation::OnlyAddress(probe) => {
            pending_stores.retain(|key, _| {
                !locations_may_alias(
                    *key,
                    probe,
                    alias_facts,
                    SlotAliasPolicy::DistinctSlotsMayAlias,
                )
            });
        }
        Invalidation::All => pending_stores.clear(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Invalidation {
    None,
    OnlySpace(MemorySpace),
    OnlyAddress(MemoryLocation),
    All,
}

fn invalidated_store_scope(
    op: &KernelOp,
    literal_values: &FxHashMap<u32, u32>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> Invalidation {
    use KernelOpKind::*;
    match &op.kind {
        LoadGlobal => memory_probe(MemorySpace::Global, op, literal_values, reaching_defs)
            .map(Invalidation::OnlyAddress)
            .unwrap_or(Invalidation::OnlySpace(MemorySpace::Global)),
        LoadShared => memory_probe(MemorySpace::Shared, op, literal_values, reaching_defs)
            .map(Invalidation::OnlyAddress)
            .unwrap_or(Invalidation::OnlySpace(MemorySpace::Shared)),
        // Atomics on the same slot also use/may-use the value.
        Atomic { .. } => op
            .operands
            .get(1)
            .and_then(|_| memory_probe(MemorySpace::Global, op, literal_values, reaching_defs))
            .map(Invalidation::OnlyAddress)
            .unwrap_or(Invalidation::All),
        // Structured control flow may read.
        StructuredIfThen
        | StructuredIfThenElse
        | StructuredForLoop { .. }
        | StructuredBlock
        | Region { .. } => Invalidation::All,
        // Barriers and async serialize visibility.
        Barrier { .. } | AsyncLoad { .. } | AsyncStore { .. } | AsyncWait { .. } => {
            Invalidation::All
        }
        // Trap/Resume/Return end the kernel; preceding store is observable
        // by the trap handler, so don't elide.
        Trap { .. } | Resume { .. } | Return => Invalidation::All,
        // Calls: opaque side effects.
        Call { .. } | OpaqueExpr(..) | OpaqueNode(..) => Invalidation::All,
        // Indirect dispatch READS `count_buffer[count_offset]` (a global
        // binding, operand 0) to pick the dispatch shape, so any pending global
        // store it may read must stay live. It touches only global memory,
        // never shared.
        IndirectDispatch { .. } => Invalidation::OnlySpace(MemorySpace::Global),
        // Everything below reads no writable global/shared buffer memory, so it
        // can never make a pending store live: pure value/builtin/arith ops,
        // function-local loop-carrier slots (a distinct address space that
        // cannot alias a tracked buffer store), read-only `LoadConstant`, and
        // the metadata-only `BufferLength`. `StoreGlobal`/`StoreShared` reach
        // this arm only for exhaustiveness — a store is keyed by `store_key`,
        // never classified as a reader here. EXHAUSTIVE ON PURPOSE (no `_`
        // wildcard): a future `KernelOpKind` that reads memory must be
        // classified by hand rather than silently defaulting to `None` and
        // eliminating a store it actually reads (a miscompile).
        Literal
        | Copy
        | LocalInvocationId
        | GlobalInvocationId
        | WorkgroupId
        | SubgroupLocalId
        | SubgroupSize
        | LoopIndex { .. }
        | LoopCarrierInit { .. }
        | LoopCarrier { .. }
        | LoopCarrierEnd { .. }
        | LoadConstant
        | BufferLength
        | BinOpKind(_)
        | UnOpKind(_)
        | Fma
        | MatrixMma { .. }
        | Select
        | Cast { .. }
        | SubgroupBallot
        | SubgroupShuffle
        | SubgroupBroadcast
        | SubgroupReduce { .. }
        | StoreGlobal
        | StoreShared => Invalidation::None,
    }
}

fn memory_probe(
    space: MemorySpace,
    op: &KernelOp,
    literal_values: &FxHashMap<u32, u32>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> Option<MemoryLocation> {
    let slot = *op.operands.first()?;
    let index = resolve(*op.operands.get(1)?, reaching_defs);
    let target = match space {
        MemorySpace::Global => MemoryTarget::global(slot),
        MemorySpace::Shared => MemoryTarget::shared(slot),
        MemorySpace::Constant => MemoryTarget::constant(slot),
    };
    Some(MemoryLocation::new(
        target,
        index,
        address_key(index, literal_values),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };
    use vyre_foundation::ir::DataType;

    fn out_binding() -> BindingSlot {
        out_binding_numbered(0)
    }

    fn out_binding_numbered(slot: u32) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::WriteOnly,
            name: format!("out{slot}"),
        }
    }

    #[test]
    fn dead_store_on_empty_kernel_is_noop() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let out = dead_store(&desc);
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn two_back_to_back_stores_to_same_address_keep_only_last() {
        let desc = KernelDescriptor {
            id: "double_store".into(),
            bindings: BindingLayout {
                slots: vec![out_binding()],
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
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1], // store value 1 at index 0
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2], // store value 2 at index 0  -  kills the prev store
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
        let out = dead_store(&desc);
        // Started with 5 ops; first store is dead → 4 ops survive.
        assert_eq!(out.body.ops.len(), 4);
        // Confirm the surviving store stores value 2.
        let store = out
            .body
            .ops
            .iter()
            .find(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .unwrap();
        assert_eq!(store.operands, vec![0, 0, 2]);
    }

    #[test]
    fn intervening_load_keeps_first_store_alive() {
        let mut binding = out_binding();
        binding.visibility = BindingVisibility::ReadWrite;
        let desc = KernelDescriptor {
            id: "store_load_store".into(),
            bindings: BindingLayout {
                slots: vec![binding],
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
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
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
        let out = dead_store(&desc);
        // The intervening load reads what the first store wrote → both stores stay.
        let store_count = out
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .count();
        assert_eq!(store_count, 2);
    }

    #[test]
    fn intervening_indirect_dispatch_keeps_first_store_alive() {
        // Regression: `IndirectDispatch` READS `count_buffer[count_offset]` (a
        // global binding) to choose the dispatch shape, so a `StoreGlobal` it
        // may read must NOT be eliminated by a later same-address store. Before
        // the exhaustive-match fix, `IndirectDispatch` fell into the optimistic
        // `_ => Invalidation::None` wildcard and the first store was wrongly
        // elided — a miscompile. Mirrors `intervening_load_keeps_first_store_alive`.
        let mut binding = out_binding();
        binding.visibility = BindingVisibility::ReadWrite;
        let desc = KernelDescriptor {
            id: "store_indirectdispatch_store".into(),
            bindings: BindingLayout {
                slots: vec![binding],
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
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                    // Reads count_buffer = global binding slot 0.
                    KernelOp {
                        kind: KernelOpKind::IndirectDispatch { count_offset: 0 },
                        operands: vec![0],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
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
        let out = dead_store(&desc);
        let store_count = out
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .count();
        assert_eq!(
            store_count, 2,
            "IndirectDispatch reads count_buffer; the first store it may read must stay live"
        );
        // The IndirectDispatch op itself is preserved (the pass never elides it).
        assert!(out
            .body
            .ops
            .iter()
            .any(|o| matches!(o.kind, KernelOpKind::IndirectDispatch { .. })));
    }

    #[test]
    fn external_no_alias_fact_preserves_unrelated_load_and_drops_dead_store() {
        let mut binding = out_binding();
        binding.visibility = BindingVisibility::ReadWrite;
        let desc = KernelDescriptor {
            id: "alias_aware_dse".into(),
            bindings: BindingLayout {
                slots: vec![binding],
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
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 1],
                        result: Some(4),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 3],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7), LiteralValue::U32(9)],
            },
        };
        let conservative = dead_store(&desc);
        let conservative_store_count = conservative
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
            .count();
        assert_eq!(conservative_store_count, 2);

        let mut facts = crate::analyses::alias_facts::AliasFactSet::default();
        facts.insert_no_alias(crate::analyses::alias_facts::NoAliasFact {
            left_binding: 0,
            left_index: 0,
            right_binding: 0,
            right_index: 1,
        });
        let alias_aware = dead_store_with_alias_facts(&desc, &facts);
        let alias_aware_store_count = alias_aware
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
            .count();
        assert_eq!(alias_aware_store_count, 1);
    }

    #[test]
    fn different_binding_load_keeps_store_alive_without_external_no_alias_fact() {
        let mut left = out_binding_numbered(0);
        left.visibility = BindingVisibility::ReadWrite;
        let mut right = out_binding_numbered(1);
        right.visibility = BindingVisibility::ReadWrite;
        let desc = KernelDescriptor {
            id: "cross_binding_alias_conservative_dse".into(),
            bindings: BindingLayout {
                slots: vec![left, right],
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
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![1, 1],
                        result: Some(4),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 3],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7), LiteralValue::U32(9)],
            },
        };

        let conservative = dead_store(&desc);
        let conservative_store_count = conservative
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
            .count();
        assert_eq!(conservative_store_count, 2);

        let mut facts = crate::analyses::alias_facts::AliasFactSet::default();
        facts.insert_no_alias(crate::analyses::alias_facts::NoAliasFact {
            left_binding: 0,
            left_index: 0,
            right_binding: 1,
            right_index: 1,
        });
        let alias_aware = dead_store_with_alias_facts(&desc, &facts);
        let alias_aware_store_count = alias_aware
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
            .count();
        assert_eq!(alias_aware_store_count, 1);
    }

    #[test]
    fn intervening_load_from_provably_different_const_index_keeps_dse_candidate() {
        let mut binding = out_binding();
        binding.visibility = BindingVisibility::ReadWrite;
        let desc = KernelDescriptor {
            id: "store_load_other_const_store".into(),
            bindings: BindingLayout {
                slots: vec![binding],
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
                        kind: KernelOpKind::Literal,
                        operands: vec![3],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 1],
                        result: Some(4),
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
                    LiteralValue::U32(1),
                    LiteralValue::U32(7),
                    LiteralValue::U32(99),
                ],
            },
        };
        let out = dead_store(&desc);
        let stores: Vec<_> = out
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .collect();
        assert_eq!(stores.len(), 1);
        assert_eq!(stores[0].operands, vec![0, 0, 3]);
    }

    #[test]
    fn stores_to_different_indices_both_survive() {
        let desc = KernelDescriptor {
            id: "two_indices".into(),
            bindings: BindingLayout {
                slots: vec![out_binding()],
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
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1], // index = result-id 0
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 2], // index = result-id 1  -  DIFFERENT
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(0),
                    LiteralValue::U32(1),
                    LiteralValue::U32(99),
                ],
            },
        };
        let out = dead_store(&desc);
        let store_count = out
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .count();
        assert_eq!(store_count, 2);
    }

    #[test]
    fn three_consecutive_stores_keep_only_last() {
        let desc = KernelDescriptor {
            id: "triple".into(),
            bindings: BindingLayout {
                slots: vec![out_binding()],
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
                        kind: KernelOpKind::Literal,
                        operands: vec![3],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
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
                    LiteralValue::U32(8),
                    LiteralValue::U32(9),
                ],
            },
        };
        let out = dead_store(&desc);
        let store_count = out
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .count();
        assert_eq!(store_count, 1);
    }

    #[test]
    fn interleaved_same_address_stores_keep_only_last_per_address() {
        let desc = KernelDescriptor {
            id: "interleaved".into(),
            bindings: BindingLayout {
                slots: vec![out_binding()],
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
                        kind: KernelOpKind::Literal,
                        operands: vec![3],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 1, 2],
                        result: None,
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
                    LiteralValue::U32(1),
                    LiteralValue::U32(7),
                    LiteralValue::U32(9),
                ],
            },
        };
        let out = dead_store(&desc);
        let stores = out
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
            .collect::<Vec<_>>();
        assert_eq!(stores.len(), 2);
        assert_eq!(stores[0].operands, vec![0, 1, 2]);
        assert_eq!(stores[1].operands, vec![0, 0, 3]);
    }

    #[test]
    fn intervening_barrier_keeps_first_store_alive() {
        let desc = KernelDescriptor {
            id: "barrier_between".into(),
            bindings: BindingLayout {
                slots: vec![out_binding()],
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
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(2),
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
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
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
        let out = dead_store(&desc);
        // Barrier serializes visibility  -  first store is observable by other threads.
        let store_count = out
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .count();
        assert_eq!(store_count, 2);
    }

    #[test]
    fn dead_store_is_idempotent() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![out_binding()],
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
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
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
        let once = dead_store(&desc);
        let twice = dead_store(&once);
        assert_eq!(once.body.ops.len(), twice.body.ops.len());
    }
}
