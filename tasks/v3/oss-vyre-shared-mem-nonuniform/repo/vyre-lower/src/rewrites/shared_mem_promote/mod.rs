//! Promote simple repeated global tile loads into workgroup-shared memory.
//!
//! This rewrite intentionally handles a narrow, fully proven pattern:
//! repeated `LoadGlobal` sites against a U32 binding indexed by
//! `global_invocation_id.x` or `local_invocation_id.x`. It inserts one
//! per-workgroup async copy into a fresh shared binding, waits for it, and
//! rewrites the repeated loads to `LoadShared(shared_slot, local_id.x)`.
//! More complex affine/tiled index shapes need range facts before they can be
//! promoted without changing semantics.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::body_index::BodyIndex;
use super::literal::ResultAllocator;
use crate::{
    BindingSlot, BindingVisibility, KernelBody, KernelDescriptor, KernelOp, KernelOpKind,
    LiteralValue, MemoryClass,
};
use rustc_hash::FxHashSet;
use vyre_foundation::ir::{BinOp, DataType, MemoryOrdering};

/// Promote simple repeated global loads into shared-memory tile reads.
#[must_use]
pub fn shared_mem_promote(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    // Newly-promoted bindings are `MemoryClass::Shared` and must live in the
    // workgroup slot range so they cannot collide with host-bound slots
    // already in `out.bindings.slots`. Seed `next_slot` to the higher of
    // (a) WORKGROUP_SLOT_BASE  -  guarantees we are above every host slot  -
    // and (b) max-existing-Shared/Scratch + 1  -  picks a fresh shared slot
    // when prior runs of this rewrite already placed bindings in the range.
    let max_shared = out
        .bindings
        .slots
        .iter()
        .filter(|binding| {
            matches!(
                binding.memory_class,
                MemoryClass::Shared | MemoryClass::Scratch,
            )
        })
        .map(|binding| binding.slot)
        .max();
    let mut next_slot = max_shared
        .map(|slot| slot.saturating_add(1))
        .unwrap_or(crate::lower::WORKGROUP_SLOT_BASE)
        .max(crate::lower::WORKGROUP_SLOT_BASE);
    let mut shared_slots = Vec::new();
    let changed = rewrite_body(
        &mut out.body,
        &out.bindings.slots,
        desc.dispatch.workgroup_size[0].max(1),
        &mut next_slot,
        &mut shared_slots,
    );
    if !changed {
        return desc.clone();
    }
    out.bindings.slots.extend(shared_slots);
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TileIndexKind {
    GlobalX,
    LocalX,
}

#[derive(Debug, Clone)]
struct Candidate {
    source_slot: u32,
    index_kind: TileIndexKind,
    op_indices: Vec<usize>,
}

fn rewrite_body(
    body: &mut KernelBody,
    bindings: &[BindingSlot],
    workgroup_x: u32,
    next_slot: &mut u32,
    shared_slots: &mut Vec<BindingSlot>,
) -> bool {
    let candidates = collect_candidates(body, bindings);
    let mut changed = false;
    let mut prefix = Vec::new();
    let mut waits = Vec::new();
    let mut replacements = BTreeMap::<usize, (u32, u32)>::new();
    let mut allocator = ResultAllocator::for_body_tree(body);
    let mut first_replaced_op = usize::MAX;

    for candidate in candidates {
        let Some(source_binding) = bindings
            .iter()
            .find(|binding| binding.slot == candidate.source_slot)
        else {
            continue;
        };
        if source_binding.element_type != DataType::U32 {
            continue;
        }
        let shared_slot = *next_slot;
        *next_slot = next_slot.saturating_add(1);
        shared_slots.push(BindingSlot {
            slot: shared_slot,
            element_type: source_binding.element_type.clone(),
            element_count: Some(workgroup_x),
            memory_class: MemoryClass::Shared,
            visibility: BindingVisibility::ReadWrite,
            name: format!("{}_shared_tile", source_binding.name),
        });

        let local_id = allocator.push_result(&mut prefix, KernelOpKind::LocalInvocationId, vec![0]);
        let offset_id = match candidate.index_kind {
            TileIndexKind::LocalX => {
                allocator.push_literal(&mut prefix, &mut body.literals, LiteralValue::U32(0))
            }
            TileIndexKind::GlobalX => {
                let workgroup_id =
                    allocator.push_result(&mut prefix, KernelOpKind::WorkgroupId, vec![0]);
                let tile_bytes = allocator.push_literal(
                    &mut prefix,
                    &mut body.literals,
                    LiteralValue::U32(workgroup_x * 4),
                );
                allocator.push_result(
                    &mut prefix,
                    KernelOpKind::BinOpKind(BinOp::Mul),
                    vec![workgroup_id, tile_bytes],
                )
            }
        };
        let size_id = allocator.push_literal(
            &mut prefix,
            &mut body.literals,
            LiteralValue::U32(workgroup_x * 4),
        );
        prefix.push(KernelOp {
            kind: KernelOpKind::AsyncLoad {
                tag: Arc::from(format!(
                    "__shared_tile_slot{}_to{}",
                    candidate.source_slot, shared_slot
                )),
            },
            operands: vec![candidate.source_slot, shared_slot, offset_id, size_id],
            result: None,
        });
        waits.push(KernelOp {
            kind: KernelOpKind::AsyncWait {
                tag: Arc::from(format!(
                    "__shared_tile_slot{}_to{}",
                    candidate.source_slot, shared_slot
                )),
            },
            operands: vec![],
            result: None,
        });
        waits.push(KernelOp {
            kind: KernelOpKind::Barrier {
                ordering: MemoryOrdering::Acquire,
            },
            operands: vec![],
            result: None,
        });
        for op_index in candidate.op_indices {
            first_replaced_op = first_replaced_op.min(op_index);
            replacements.insert(op_index, (shared_slot, local_id));
        }
        changed = true;
    }

    if changed {
        for (op_index, (shared_slot, local_id)) in replacements {
            if let Some(op) = body.ops.get_mut(op_index) {
                op.kind = KernelOpKind::LoadShared;
                op.operands = vec![shared_slot, local_id];
            }
        }
        let old_ops = std::mem::take(&mut body.ops);
        let overlap_count = old_ops
            .iter()
            .take(first_replaced_op)
            .take_while(|op| can_overlap_before_async_wait(&op.kind))
            .count();
        let mut old_ops = old_ops.into_iter();
        prefix.extend(old_ops.by_ref().take(overlap_count));
        prefix.extend(waits);
        prefix.extend(old_ops);
        body.ops = prefix;
    }

    for child in &mut body.child_bodies {
        changed |= rewrite_body(child, bindings, workgroup_x, next_slot, shared_slots);
    }

    changed
}

fn can_overlap_before_async_wait(kind: &KernelOpKind) -> bool {
    matches!(
        kind,
        KernelOpKind::Literal
            | KernelOpKind::LocalInvocationId
            | KernelOpKind::GlobalInvocationId
            | KernelOpKind::WorkgroupId
            | KernelOpKind::SubgroupLocalId
            | KernelOpKind::SubgroupSize
            | KernelOpKind::LoopIndex { .. }
            | KernelOpKind::BinOpKind(_)
            | KernelOpKind::UnOpKind(_)
            | KernelOpKind::Fma
            | KernelOpKind::Select
            | KernelOpKind::Cast { .. }
    )
}

fn collect_candidates(body: &KernelBody, bindings: &[BindingSlot]) -> Vec<Candidate> {
    let readonly_u32_globals = bindings
        .iter()
        .filter(|binding| {
            matches!(binding.memory_class, MemoryClass::Global)
                && matches!(binding.visibility, BindingVisibility::ReadOnly)
                && binding.element_type == DataType::U32
        })
        .map(|binding| binding.slot)
        .collect::<FxHashSet<_>>();
    if readonly_u32_globals.is_empty() {
        return Vec::new();
    }

    let index = BodyIndex::new(body);
    let mut groups = BTreeMap::<(u32, TileIndexKind), Vec<usize>>::new();
    for (op_index, op) in body.ops.iter().enumerate() {
        if !matches!(op.kind, KernelOpKind::LoadGlobal) {
            continue;
        }
        let (Some(&slot), Some(&index_id)) = (op.operands.first(), op.operands.get(1)) else {
            continue;
        };
        if !readonly_u32_globals.contains(&slot) {
            continue;
        }
        let Some(index_kind) = classify_tile_index(body, &index, index_id) else {
            continue;
        };
        groups.entry((slot, index_kind)).or_default().push(op_index);
    }

    groups
        .into_iter()
        .filter_map(|((source_slot, index_kind), op_indices)| {
            if op_indices.len() < 2 {
                return None;
            }
            Some(Candidate {
                source_slot,
                index_kind,
                op_indices,
            })
        })
        .collect()
}

fn classify_tile_index(
    body: &KernelBody,
    index: &BodyIndex,
    index_id: u32,
) -> Option<TileIndexKind> {
    let producer = index.producer(body, index_id)?;
    match producer.kind {
        KernelOpKind::GlobalInvocationId
            if producer.operands.first().copied().unwrap_or(0) == 0 =>
        {
            Some(TileIndexKind::GlobalX)
        }
        KernelOpKind::LocalInvocationId if producer.operands.first().copied().unwrap_or(0) == 0 => {
            Some(TileIndexKind::LocalX)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch};

    fn op(kind: KernelOpKind, operands: Vec<u32>, result: Option<u32>) -> KernelOp {
        KernelOp {
            kind,
            operands,
            result,
        }
    }

    fn binding(slot: u32, dtype: DataType, visibility: BindingVisibility) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: dtype,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility,
            name: format!("b{slot}"),
        }
    }

    fn kernel(binding: BindingSlot, index_kind: KernelOpKind) -> KernelDescriptor {
        KernelDescriptor {
            id: "shared".into(),
            bindings: BindingLayout {
                slots: vec![binding],
            },
            dispatch: Dispatch::new(32, 1, 1),
            body: KernelBody {
                ops: vec![
                    op(index_kind, vec![0], Some(0)),
                    op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
                    op(KernelOpKind::LoadGlobal, vec![0, 0], Some(2)),
                ],
                child_bodies: vec![],
                literals: vec![],
            },
        }
    }

    #[test]
    fn promotes_repeated_global_x_u32_loads() {
        let input = kernel(
            binding(0, DataType::U32, BindingVisibility::ReadOnly),
            KernelOpKind::GlobalInvocationId,
        );
        let output = shared_mem_promote(&input);

        assert_eq!(output.bindings.slots.len(), 2);
        assert_eq!(output.bindings.slots[1].memory_class, MemoryClass::Shared);
        assert_eq!(output.bindings.slots[1].element_count, Some(32));
        assert!(matches!(
            output.body.ops[0].kind,
            KernelOpKind::LocalInvocationId
        ));
        assert!(output
            .body
            .ops
            .iter()
            .any(|op| matches!(op.kind, KernelOpKind::AsyncLoad { .. })));
        assert!(output
            .body
            .ops
            .iter()
            .any(|op| matches!(op.kind, KernelOpKind::AsyncWait { .. })));
        let load_kinds = output
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::LoadShared))
            .count();
        assert_eq!(load_kinds, 2);
    }

    #[test]
    fn promotes_repeated_local_x_u32_loads() {
        let input = kernel(
            binding(0, DataType::U32, BindingVisibility::ReadOnly),
            KernelOpKind::LocalInvocationId,
        );
        let output = shared_mem_promote(&input);

        assert_eq!(output.bindings.slots.len(), 2);
        assert_eq!(
            output
                .body
                .ops
                .iter()
                .filter(|op| matches!(op.kind, KernelOpKind::LoadShared))
                .count(),
            2
        );
    }

    #[test]
    fn leaves_pure_preload_compute_between_async_issue_and_wait() {
        let input = kernel(
            binding(0, DataType::U32, BindingVisibility::ReadOnly),
            KernelOpKind::GlobalInvocationId,
        );
        let output = shared_mem_promote(&input);

        let async_pos = output
            .body
            .ops
            .iter()
            .position(|op| matches!(op.kind, KernelOpKind::AsyncLoad { .. }))
            .expect("Fix: shared-memory promotion must issue an async tile load");
        let wait_pos = output
            .body
            .ops
            .iter()
            .position(|op| matches!(op.kind, KernelOpKind::AsyncWait { .. }))
            .expect("Fix: shared-memory promotion must wait before shared loads");
        let original_index_pos = output
            .body
            .ops
            .iter()
            .position(|op| {
                matches!(op.kind, KernelOpKind::GlobalInvocationId) && op.result == Some(0)
            })
            .expect("Fix: the original pure index op must be preserved");

        assert!(
            async_pos < original_index_pos && original_index_pos < wait_pos,
            "Fix: pure work that originally preceded the promoted loads should overlap the async copy instead of being forced after AsyncWait."
        );
    }

    #[test]
    fn skips_non_u32_until_emitters_support_typed_async_copy() {
        let input = kernel(
            binding(0, DataType::F32, BindingVisibility::ReadOnly),
            KernelOpKind::GlobalInvocationId,
        );
        let output = shared_mem_promote(&input);

        assert_eq!(output, input);
    }

    #[test]
    fn skips_writable_bindings() {
        let input = kernel(
            binding(0, DataType::U32, BindingVisibility::ReadWrite),
            KernelOpKind::GlobalInvocationId,
        );
        let output = shared_mem_promote(&input);

        assert_eq!(output, input);
    }

    #[test]
    fn skips_single_load() {
        let mut input = kernel(
            binding(0, DataType::U32, BindingVisibility::ReadOnly),
            KernelOpKind::GlobalInvocationId,
        );
        input.body.ops.pop();
        let output = shared_mem_promote(&input);

        assert_eq!(output, input);
    }
}
