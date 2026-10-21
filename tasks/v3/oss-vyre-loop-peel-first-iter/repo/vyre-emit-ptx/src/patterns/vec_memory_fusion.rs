//! Shared PTX vector memory fusion chain detector.

use crate::index_facts::IndexFacts;
use rustc_hash::FxHashMap;
use vyre_foundation::ir::DataType;
use vyre_lower::{BindingSlot, KernelBody, KernelDescriptor, KernelOp, KernelOpKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MemoryFusionKind {
    Load,
    Store,
}

impl MemoryFusionKind {
    fn matches(self, kind: &KernelOpKind) -> bool {
        match self {
            Self::Load => matches!(kind, KernelOpKind::LoadGlobal | KernelOpKind::LoadConstant),
            Self::Store => matches!(kind, KernelOpKind::StoreGlobal),
        }
    }

    fn slot_and_index(self, op: &KernelOp) -> Option<(u32, u32)> {
        let min_operands = match self {
            Self::Load => 2,
            Self::Store => 3,
        };
        if op.operands.len() < min_operands {
            return None;
        }
        Some((op.operands[0], op.operands[1]))
    }

    fn value_operand(self, op: &KernelOp) -> Option<u32> {
        match self {
            Self::Load => None,
            Self::Store => op.operands.get(2).copied(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MemoryFusionCandidate {
    pub(super) first_op_idx: usize,
    pub(super) group_size: u8,
    pub(super) binding_slot: u32,
    pub(super) element_type: DataType,
    pub(super) alignment_bytes: u32,
}

#[must_use]
pub(super) fn analyze_memory_fusion(
    desc: &KernelDescriptor,
    kind: MemoryFusionKind,
) -> Vec<MemoryFusionCandidate> {
    let binding_by_slot: FxHashMap<u32, &BindingSlot> = desc
        .bindings
        .slots
        .iter()
        .map(|binding| (binding.slot, binding))
        .collect();
    let mut candidates = Vec::new();
    walk(&desc.body, &binding_by_slot, kind, &mut candidates);
    candidates
}

fn walk(
    body: &KernelBody,
    binding_by_slot: &FxHashMap<u32, &BindingSlot>,
    kind: MemoryFusionKind,
    candidates: &mut Vec<MemoryFusionCandidate>,
) {
    let facts = IndexFacts::new(body);
    let mut i = 0;
    while i < body.ops.len() {
        let op = &body.ops[i];
        if !kind.matches(&op.kind) {
            i += 1;
            continue;
        }
        let Some((slot, base_idx_id)) = kind.slot_and_index(op) else {
            i += 1;
            continue;
        };
        let Some(binding) = binding_by_slot.get(&slot).copied() else {
            i += 1;
            continue;
        };

        let mut chain_len: u8 = 1;
        let mut prev_idx_id = base_idx_id;
        let mut j = i + 1;
        while j < body.ops.len() && chain_len < 4 {
            let gap_start = j;
            while j < body.ops.len() {
                let next = &body.ops[j];
                if kind.matches(&next.kind) {
                    break;
                }
                if is_scheduling_fence(next) || !is_schedulable_pure_op(next) {
                    break;
                }
                j += 1;
            }
            if j >= body.ops.len() {
                break;
            }
            let next = &body.ops[j];
            if !kind.matches(&next.kind) {
                break;
            }
            let Some((next_slot, next_idx_id)) = kind.slot_and_index(next) else {
                break;
            };
            if let Some(next_value_id) = kind.value_operand(next) {
                if body.ops[gap_start..j]
                    .iter()
                    .any(|gap_op| gap_op.result == Some(next_value_id))
                {
                    break;
                }
            }
            if next_slot != slot || !facts.is_index_plus_one(body, next_idx_id, prev_idx_id) {
                break;
            }
            chain_len += 1;
            prev_idx_id = next_idx_id;
            j += 1;
        }

        if chain_len >= 2 {
            let group_size = if chain_len >= 4 { 4 } else { 2 };
            let elem_size = binding.element_type.size_bytes().unwrap_or(0) as u32;
            candidates.push(MemoryFusionCandidate {
                first_op_idx: i,
                group_size,
                binding_slot: slot,
                element_type: binding.element_type.clone(),
                alignment_bytes: group_size as u32 * elem_size,
            });
            i += (group_size as usize) * 2 - 1;
        } else {
            i += 1;
        }
    }

    for child in &body.child_bodies {
        walk(child, binding_by_slot, kind, candidates);
    }
}

fn is_scheduling_fence(op: &KernelOp) -> bool {
    matches!(
        op.kind,
        KernelOpKind::StoreGlobal
            | KernelOpKind::StoreShared
            | KernelOpKind::Atomic { .. }
            | KernelOpKind::Barrier { .. }
            | KernelOpKind::Return
            | KernelOpKind::Region { .. }
            | KernelOpKind::StructuredBlock
            | KernelOpKind::StructuredIfThen
            | KernelOpKind::StructuredIfThenElse
            | KernelOpKind::StructuredForLoop { .. }
            | KernelOpKind::AsyncLoad { .. }
            | KernelOpKind::AsyncStore { .. }
            | KernelOpKind::AsyncWait { .. }
            | KernelOpKind::Trap { .. }
    )
}

fn is_schedulable_pure_op(op: &KernelOp) -> bool {
    matches!(
        op.kind,
        KernelOpKind::Literal
            | KernelOpKind::LocalInvocationId
            | KernelOpKind::GlobalInvocationId
            | KernelOpKind::WorkgroupId
            | KernelOpKind::BinOpKind(_)
            | KernelOpKind::UnOpKind(_)
            | KernelOpKind::Fma
            | KernelOpKind::MatrixMma { .. }
            | KernelOpKind::Cast { .. }
            | KernelOpKind::Select
            | KernelOpKind::BufferLength
            | KernelOpKind::SubgroupLocalId
            | KernelOpKind::SubgroupSize
            | KernelOpKind::SubgroupBallot
            | KernelOpKind::SubgroupShuffle
            | KernelOpKind::SubgroupReduce { .. }
    ) && op.result.is_some()
}
