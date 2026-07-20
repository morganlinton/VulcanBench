//! Shared-memory bank-conflict padding rewrite.
//!
//! This pass handles the common lowered pattern
//! `shared[local_id.x * STRIDE + c]`. If `STRIDE` aliases GPU shared-memory
//! banks, it pads the logical row pitch to `STRIDE + 1` when that new pitch is
//! coprime with the bank count. The rewrite updates the stride literal feeding
//! the index expression and increases the shared binding's element count to
//! preserve the same logical row capacity with one padding element per row.

use std::collections::BTreeMap;

use super::body_index::BodyIndex;
use crate::{
    BindingLayout, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};
use rustc_hash::{FxHashMap, FxHashSet};
use vyre_foundation::ir::BinOp;

const DEFAULT_BANK_COUNT: u32 = crate::analyses::bank_conflict::DEFAULT_BANK_COUNT;

/// Pad simple strided shared-memory layouts that would otherwise serialize a
/// warp on shared-memory bank conflicts.
#[must_use]
pub fn bank_conflict_pad(desc: &KernelDescriptor) -> KernelDescriptor {
    bank_conflict_pad_with_bank_count(desc, DEFAULT_BANK_COUNT)
}

/// Same as [`bank_conflict_pad`] with an explicit bank count for tests and
/// future backend capability plumbing.
#[must_use]
pub fn bank_conflict_pad_with_bank_count(
    desc: &KernelDescriptor,
    bank_count: u32,
) -> KernelDescriptor {
    if bank_count <= 1 {
        return desc.clone();
    }
    let Some(plan) = plan_body(&desc.body, &desc.bindings, bank_count) else {
        return desc.clone();
    };
    if plan.is_empty() {
        return desc.clone();
    }

    let mut out = desc.clone();
    for (&slot, &new_count) in &plan.binding_counts {
        if let Some(binding) = out
            .bindings
            .slots
            .iter_mut()
            .find(|binding| binding.slot == slot)
        {
            binding.element_count = Some(new_count);
        }
    }
    rewrite_body_literals_recursive(&mut out.body, &plan);
    out
}

#[derive(Default)]
struct PaddingPlan {
    literal_updates: FxHashMap<u32, u32>,
    binding_counts: FxHashMap<u32, u32>,
    swizzle_updates: BTreeMap<usize, SwizzleUpdate>,
    child_plans: BTreeMap<usize, PaddingPlan>,
}

impl PaddingPlan {
    fn is_empty(&self) -> bool {
        self.literal_updates.is_empty()
            && self.binding_counts.is_empty()
            && self.swizzle_updates.is_empty()
            && self.child_plans.is_empty()
    }
}

#[derive(Clone, Copy)]
struct StrideUse {
    access_op_index: usize,
    index_result_id: u32,
    literal_result_id: u32,
    local_id_result_id: u32,
    mul_result_id: u32,
    stride: u32,
}

#[derive(Clone, Copy)]
struct SwizzleUpdate {
    index_result_id: u32,
    local_id_result_id: u32,
}

fn plan_body(body: &KernelBody, bindings: &BindingLayout, bank_count: u32) -> Option<PaddingPlan> {
    let mut plan = PaddingPlan::default();
    let index = BodyIndex::new(body);
    let shared_slots = bindings
        .slots
        .iter()
        .filter(|binding| matches!(binding.memory_class, MemoryClass::Shared))
        .filter_map(|binding| binding.element_count.map(|count| (binding.slot, count)))
        .collect::<Vec<_>>();

    for (slot, element_count) in shared_slots {
        let accesses = shared_accesses_for_slot(body, &index, slot)?;
        if accesses.is_empty() {
            continue;
        }
        let stride = accesses[0].stride;
        if accesses.iter().any(|access| access.stride != stride) {
            continue;
        }
        if stride <= 1 || gcd_u32(stride, bank_count) <= 1 {
            continue;
        }
        let mut target_mul_results_by_literal: FxHashMap<u32, FxHashSet<u32>> =
            FxHashMap::default();
        for access in &accesses {
            target_mul_results_by_literal
                .entry(access.literal_result_id)
                .or_default()
                .insert(access.mul_result_id);
        }
        let padded_stride = stride.saturating_add(1);
        let mut updated_stride_for_slot = false;
        if gcd_u32(padded_stride, bank_count) == 1 {
            for (literal_result_id, target_mul_results) in target_mul_results_by_literal {
                if index.use_count_of(literal_result_id) as usize != target_mul_results.len() {
                    continue;
                }
                plan.literal_updates
                    .insert(literal_result_id, padded_stride);
                updated_stride_for_slot = true;
            }
        }
        if !updated_stride_for_slot {
            for access in accesses {
                plan.swizzle_updates.insert(
                    access.access_op_index,
                    SwizzleUpdate {
                        index_result_id: access.index_result_id,
                        local_id_result_id: access.local_id_result_id,
                    },
                );
            }
            let swizzled_count = element_count.saturating_add(bank_count);
            plan.binding_counts.insert(slot, swizzled_count);
            continue;
        }

        let rows = element_count.div_ceil(stride);
        let padded_count = rows.saturating_mul(padded_stride);
        if padded_count > element_count {
            plan.binding_counts.insert(slot, padded_count);
        }
    }

    for (child_index, child) in body.child_bodies.iter().enumerate() {
        let child_plan = plan_body(child, bindings, bank_count)?;
        if child_plan.is_empty() {
            continue;
        }
        for (&slot, &child_count) in &child_plan.binding_counts {
            plan.binding_counts
                .entry(slot)
                .and_modify(|count| *count = (*count).max(child_count))
                .or_insert(child_count);
        }
        plan.child_plans.insert(child_index, child_plan);
    }

    Some(plan)
}

fn shared_accesses_for_slot(
    body: &KernelBody,
    index: &BodyIndex,
    slot: u32,
) -> Option<Vec<StrideUse>> {
    let mut accesses = Vec::new();
    for (op_index, op) in body.ops.iter().enumerate() {
        match op.kind {
            KernelOpKind::LoadShared | KernelOpKind::StoreShared => {
                if op.operands.first().copied() != Some(slot) {
                    continue;
                }
                let index_id = *op.operands.get(1)?;
                match parse_stride_index(body, index, index_id) {
                    Some(stride_use) => accesses.push(StrideUse {
                        access_op_index: op_index,
                        index_result_id: index_id,
                        ..stride_use
                    }),
                    None => return None,
                }
            }
            _ => {}
        }
    }
    Some(accesses)
}

fn parse_stride_index(body: &KernelBody, index: &BodyIndex, result_id: u32) -> Option<StrideUse> {
    let op = index.producer(body, result_id)?;
    match &op.kind {
        KernelOpKind::BinOpKind(BinOp::Add | BinOp::WrappingAdd) if op.operands.len() == 2 => {
            parse_stride_index(body, index, op.operands[0])
                .or_else(|| parse_stride_index(body, index, op.operands[1]))
        }
        KernelOpKind::BinOpKind(BinOp::Mul) if op.operands.len() == 2 => {
            parse_mul_stride(body, index, result_id, op.operands[0], op.operands[1]).or_else(|| {
                parse_mul_stride(body, index, result_id, op.operands[1], op.operands[0])
            })
        }
        KernelOpKind::LocalInvocationId => None,
        KernelOpKind::Literal => None,
        _ => None,
    }
}

fn parse_mul_stride(
    body: &KernelBody,
    index: &BodyIndex,
    mul_result_id: u32,
    id_operand: u32,
    literal_operand: u32,
) -> Option<StrideUse> {
    let id_op = index.producer(body, id_operand)?;
    if !matches!(id_op.kind, KernelOpKind::LocalInvocationId) {
        return None;
    }
    let stride = index.u32_lit(body, literal_operand)?;
    Some(StrideUse {
        access_op_index: 0,
        index_result_id: 0,
        literal_result_id: literal_operand,
        local_id_result_id: id_operand,
        mul_result_id,
        stride,
    })
}

fn rewrite_body_literals_recursive(body: &mut KernelBody, plan: &PaddingPlan) {
    for op in &mut body.ops {
        let Some(result_id) = op.result else {
            continue;
        };
        let Some(&new_stride) = plan.literal_updates.get(&result_id) else {
            continue;
        };
        if !matches!(op.kind, KernelOpKind::Literal) {
            continue;
        }
        let new_pool_index = body.literals.len() as u32;
        body.literals.push(LiteralValue::U32(new_stride));
        op.operands.clear();
        op.operands.push(new_pool_index);
    }
    for (&child_index, child_plan) in &plan.child_plans {
        if let Some(child) = body.child_bodies.get_mut(child_index) {
            rewrite_body_literals_recursive(child, child_plan);
        }
    }
    rewrite_body_swizzles(body, &plan.swizzle_updates);
}

fn rewrite_body_swizzles(body: &mut KernelBody, swizzles: &BTreeMap<usize, SwizzleUpdate>) {
    if swizzles.is_empty() {
        return;
    }
    let mut next_result = body
        .ops
        .iter()
        .flat_map(KernelOp::result_ids)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let old_ops = std::mem::take(&mut body.ops);
    let mut new_ops = Vec::with_capacity(old_ops.len() + swizzles.len());
    for (op_index, mut op) in old_ops.into_iter().enumerate() {
        if let Some(update) = swizzles.get(&op_index) {
            let swizzled_result = next_result;
            next_result = next_result.saturating_add(1);
            new_ops.push(KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::BitXor),
                operands: vec![update.index_result_id, update.local_id_result_id],
                result: Some(swizzled_result),
            });
            if let Some(index_operand) = op.operands.get_mut(1) {
                *index_operand = swizzled_result;
            }
        }
        new_ops.push(op);
    }
    body.ops = new_ops;
}

fn gcd_u32(a: u32, b: u32) -> u32 {
    let (mut a, mut b) = (a, b);
    while b != 0 {
        let next = a % b;
        a = b;
        b = next;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingSlot, BindingVisibility, Dispatch, KernelOp, MemoryClass};
    use vyre_foundation::ir::DataType;

    fn op(kind: KernelOpKind, operands: Vec<u32>, result: Option<u32>) -> KernelOp {
        KernelOp {
            kind,
            operands,
            result,
        }
    }

    fn shared_slot(count: u32) -> BindingSlot {
        BindingSlot {
            slot: 0,
            element_type: DataType::F32,
            element_count: Some(count),
            memory_class: MemoryClass::Shared,
            visibility: BindingVisibility::ReadWrite,
            name: "tile".into(),
        }
    }

    fn kernel(stride: u32, count: u32) -> KernelDescriptor {
        KernelDescriptor {
            id: "pad".into(),
            bindings: BindingLayout {
                slots: vec![shared_slot(count)],
            },
            dispatch: Dispatch::new(32, 1, 1),
            body: KernelBody {
                ops: vec![
                    op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                    op(KernelOpKind::Literal, vec![0], Some(1)),
                    op(KernelOpKind::BinOpKind(BinOp::Mul), vec![0, 1], Some(2)),
                    op(KernelOpKind::LoadShared, vec![0, 2], Some(3)),
                    op(KernelOpKind::StoreShared, vec![0, 2, 3], None),
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(stride)],
            },
        }
    }

    #[test]
    fn pads_conflicting_stride_to_coprime_pitch() {
        let padded = bank_conflict_pad(&kernel(32, 1024));
        assert_eq!(padded.bindings.slots[0].element_count, Some(1056));
        assert!(matches!(
            padded.body.literals.last(),
            Some(LiteralValue::U32(33))
        ));
        let literal_op = &padded.body.ops[1];
        assert_eq!(literal_op.operands, vec![1]);
    }

    #[test]
    fn leaves_coprime_stride_unchanged() {
        let input = kernel(3, 96);
        let output = bank_conflict_pad(&input);
        assert_eq!(output, input);
    }

    #[test]
    fn swizzles_when_stride_literal_has_other_uses() {
        let mut input = kernel(32, 1024);
        input
            .body
            .ops
            .push(op(KernelOpKind::BinOpKind(BinOp::Add), vec![1, 1], Some(4)));
        let output = bank_conflict_pad(&input);

        assert_eq!(output.body.literals, input.body.literals);
        assert_eq!(output.bindings.slots[0].element_count, Some(1056));
        let swizzles = output
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::BinOpKind(BinOp::BitXor)))
            .count();
        assert_eq!(swizzles, 2);
        assert!(matches!(
            output.body.ops[3].kind,
            KernelOpKind::BinOpKind(BinOp::BitXor)
        ));
        assert!(matches!(output.body.ops[4].kind, KernelOpKind::LoadShared));
        assert_eq!(
            output.body.ops[4].operands[1],
            output.body.ops[3].result.unwrap()
        );
    }

    #[test]
    fn scopes_literal_rewrites_to_child_body() {
        let child = kernel(32, 1024).body;
        let mut input = kernel(3, 1024);
        input.body.ops = vec![
            op(KernelOpKind::Literal, vec![0], Some(1)),
            op(KernelOpKind::StructuredBlock, vec![0], None),
        ];
        input.body.child_bodies = vec![child];
        input.body.literals = vec![LiteralValue::U32(7)];

        let output = bank_conflict_pad(&input);

        assert_eq!(output.body.ops[0].operands, vec![0]);
        assert_eq!(output.body.literals, vec![LiteralValue::U32(7)]);
        assert_eq!(output.body.child_bodies[0].ops[1].operands, vec![1]);
        assert!(matches!(
            output.body.child_bodies[0].literals.last(),
            Some(LiteralValue::U32(33))
        ));
        assert_eq!(output.bindings.slots[0].element_count, Some(1056));
    }
}
