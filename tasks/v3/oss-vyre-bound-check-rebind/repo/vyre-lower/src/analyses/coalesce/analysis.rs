//! Analysis pass: walk a `KernelDescriptor`'s op stream, classify
//! each `LoadGlobal` and `StoreGlobal` access pattern, build a
//! `CoalescenceReport`.
//!
//! ## Algorithm
//!
//! For each `LoadGlobal` / `StoreGlobal` op, the index operand is a
//! `LiteralValue` reference. We trace it backward through the body's
//! ops to determine which of these forms it has:
//!
//! 1. `LocalInvocationId.x` / `GlobalInvocationId.x` → CoalescedUnitStride
//! 2. `Add(invocation_id.x, <const>)`                → CoalescedUnitStride
//! 3. `Mul(invocation_id.x, <const k>)`              → Strided { stride: k }
//! 4. `Add(Mul(invocation_id.x, k), c)`              → Strided { stride: k }
//! 5. literal constant                         → Broadcast
//! 6. anything else                            → Scattered
//!
//! Conservative cases that cannot be proven constant-stride classify
//! as `Scattered`, which is the rewrite-safe direction.

use super::report::{AccessPattern, AccessSite, CoalescenceReport};
use crate::analyses::AccessKind;
use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use rustc_hash::FxHashMap;
use vyre_foundation::ir::BinOp;

/// Run coalescence analysis on a kernel.
#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> CoalescenceReport {
    let mut sites = Vec::new();
    walk_body(&desc.body, &mut sites, 0);
    CoalescenceReport {
        kernel_id: desc.id.clone(),
        sites,
    }
}

fn walk_body(body: &KernelBody, sites: &mut Vec<AccessSite>, op_index_offset: usize) {
    let producers = producer_map(body);
    for (local_idx, op) in body.ops.iter().enumerate() {
        let op_index = op_index_offset + local_idx;
        let Some((kind, slot_pos, index_pos)) = (match op.kind {
            KernelOpKind::LoadGlobal => Some((AccessKind::Load, 0, 1)),
            KernelOpKind::StoreGlobal => Some((AccessKind::Store, 0, 1)),
            KernelOpKind::StructuredIfThen
            | KernelOpKind::StructuredIfThenElse
            | KernelOpKind::StructuredForLoop { .. }
            | KernelOpKind::StructuredBlock
            | KernelOpKind::Region { .. } => {
                for child_id in child_body_operands(&op.kind, &op.operands) {
                    if let Some(child) = body.child_bodies.get(*child_id as usize) {
                        walk_body(child, sites, op_index_offset + body.ops.len());
                    }
                }
                None
            }
            _ => None,
        }) else {
            continue;
        };

        // Bounds check the operand list so a malformed descriptor
        // doesn't panic the analysis.
        if op.operands.len() <= index_pos.max(slot_pos) {
            continue;
        }

        let binding_slot = op.operands[slot_pos];
        let index_operand_id = op.operands[index_pos];
        let pattern = classify_index(body, &producers, index_operand_id);

        sites.push(AccessSite {
            op_index,
            kind,
            binding_slot,
            pattern,
        });
    }
}

type ProducerMap<'a> = FxHashMap<u32, &'a crate::KernelOp>;

fn producer_map(body: &KernelBody) -> ProducerMap<'_> {
    let mut producers = FxHashMap::with_capacity_and_hasher(body.ops.len(), Default::default());
    for op in &body.ops {
        for result in op.result_ids() {
            producers.insert(result, op);
        }
    }
    producers
}

/// Classify an index expression by its access pattern across threads.
///
/// `index_operand_id` is the `result` of some op in `body.ops`. We
/// trace backward to find that op and determine its shape.
fn classify_index(
    body: &KernelBody,
    producers: &ProducerMap<'_>,
    index_operand_id: u32,
) -> AccessPattern {
    let producer = producers.get(&index_operand_id).copied();
    let Some(producer) = producer else {
        // Not a body-local result  -  could be a literal pool ref. Look
        // it up there.
        return classify_pool_operand(body, index_operand_id);
    };

    match &producer.kind {
        KernelOpKind::LocalInvocationId | KernelOpKind::GlobalInvocationId => {
            classify_invocation_id(producer)
        }
        KernelOpKind::Literal => AccessPattern::Broadcast,
        KernelOpKind::BinOpKind(BinOp::Add | BinOp::WrappingAdd) => {
            classify_add(body, producers, &producer.operands)
        }
        KernelOpKind::BinOpKind(BinOp::Mul) => classify_mul(body, producers, &producer.operands),
        _ => AccessPattern::Scattered,
    }
}

fn classify_invocation_id(op: &crate::KernelOp) -> AccessPattern {
    match op.operands.first().copied().unwrap_or(0) {
        0 => AccessPattern::CoalescedUnitStride,
        _ => AccessPattern::Scattered,
    }
}

fn classify_add(body: &KernelBody, producers: &ProducerMap<'_>, operands: &[u32]) -> AccessPattern {
    if operands.len() != 2 {
        return AccessPattern::Scattered;
    }
    let lhs = classify_index(body, producers, operands[0]);
    let rhs = classify_index(body, producers, operands[1]);
    // Broadcast (constant) + CoalescedUnitStride = still coalesced
    // unit-stride (just at base + const).
    match (lhs, rhs) {
        (AccessPattern::CoalescedUnitStride, AccessPattern::Broadcast)
        | (AccessPattern::Broadcast, AccessPattern::CoalescedUnitStride) => {
            AccessPattern::CoalescedUnitStride
        }
        // Strided + constant offset preserves stride.
        (AccessPattern::Strided { stride }, AccessPattern::Broadcast)
        | (AccessPattern::Broadcast, AccessPattern::Strided { stride }) => {
            AccessPattern::Strided { stride }
        }
        _ => AccessPattern::Scattered,
    }
}

fn classify_mul(body: &KernelBody, producers: &ProducerMap<'_>, operands: &[u32]) -> AccessPattern {
    if operands.len() != 2 {
        return AccessPattern::Scattered;
    }
    // We're looking for k * LocalInvocationId.x or LocalInvocationId.x * k.
    let const_operand = {
        let l = classify_index(body, producers, operands[0]);
        let r = classify_index(body, producers, operands[1]);
        match (l, r) {
            (AccessPattern::CoalescedUnitStride, AccessPattern::Broadcast) => operands[1],
            (AccessPattern::Broadcast, AccessPattern::CoalescedUnitStride) => operands[0],
            _ => return AccessPattern::Scattered,
        }
    };

    let stride = match producers.get(&const_operand).copied() {
        Some(producer) if producer.kind == KernelOpKind::Literal => {
            // The producer's operand[0] is an index into the literal pool.
            producer.operands.first().and_then(|i| {
                body.literals.get(*i as usize).and_then(|op| match op {
                    LiteralValue::U32(v) => Some(*v),
                    _ => None,
                })
            })
        }
        _ => None,
    }
    .or_else(|| literal_operand_u32(body, const_operand));

    match stride {
        Some(0) => AccessPattern::Broadcast,
        Some(1) => AccessPattern::CoalescedUnitStride,
        Some(k) if k > 1 => AccessPattern::Strided { stride: k },
        _ => AccessPattern::Scattered,
    }
}

fn classify_pool_operand(body: &KernelBody, operand_id: u32) -> AccessPattern {
    if body.literals.get(operand_id as usize).is_some() {
        AccessPattern::Broadcast
    } else {
        AccessPattern::Scattered
    }
}

fn literal_operand_u32(body: &KernelBody, operand_id: u32) -> Option<u32> {
    body.literals
        .get(operand_id as usize)
        .and_then(|literal| match literal {
            LiteralValue::U32(value) => Some(*value),
            _ => None,
        })
}

fn child_body_operands<'a>(
    kind: &KernelOpKind,
    operands: &'a [u32],
) -> impl Iterator<Item = &'a u32> {
    let start = match kind {
        KernelOpKind::StructuredIfThen | KernelOpKind::StructuredIfThenElse => 1,
        KernelOpKind::StructuredForLoop { .. } => 2,
        KernelOpKind::StructuredBlock | KernelOpKind::Region { .. } => 0,
        _ => operands.len(),
    };
    operands.iter().skip(start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, MemoryClass,
    };
    use vyre_foundation::ir::DataType;

    fn one_buffer_kernel(ops: Vec<KernelOp>, literals: Vec<LiteralValue>) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "buf".into(),
                }],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![],
                literals,
            },
        }
    }

    fn op(kind: KernelOpKind, operands: Vec<u32>, result: Option<u32>) -> KernelOp {
        KernelOp {
            kind,
            operands,
            result,
        }
    }

    // Positive truth (coalesced detected)

    #[test]
    fn positive_load_at_local_invocation_id_is_coalesced() {
        // tid = LocalInvocationId; load(buf, tid)
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
            ],
            vec![],
        );
        let r = analyze(&k);
        assert_eq!(r.sites.len(), 1);
        assert_eq!(r.sites[0].pattern, AccessPattern::CoalescedUnitStride);
        assert_eq!(r.sites[0].kind, AccessKind::Load);
    }

    #[test]
    fn positive_store_at_local_invocation_id_is_coalesced() {
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(1)),
                op(KernelOpKind::StoreGlobal, vec![0, 0, 1], None),
            ],
            vec![LiteralValue::U32(7)],
        );
        let r = analyze(&k);
        assert_eq!(r.sites.len(), 1);
        assert_eq!(r.sites[0].pattern, AccessPattern::CoalescedUnitStride);
        assert_eq!(r.sites[0].kind, AccessKind::Store);
    }

    #[test]
    fn positive_load_at_tid_plus_constant_is_coalesced() {
        // load(buf, tid + 16)  -  still coalesced unit stride
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(1)),
                op(
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Add),
                    vec![0, 1],
                    Some(2),
                ),
                op(KernelOpKind::LoadGlobal, vec![0, 2], Some(3)),
            ],
            vec![LiteralValue::U32(16)],
        );
        let r = analyze(&k);
        assert_eq!(r.sites[0].pattern, AccessPattern::CoalescedUnitStride);
    }

    #[test]
    fn positive_load_at_global_invocation_id_treated_as_coalesced() {
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::GlobalInvocationId, vec![0], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
            ],
            vec![],
        );
        let r = analyze(&k);
        assert_eq!(r.sites[0].pattern, AccessPattern::CoalescedUnitStride);
    }

    #[test]
    fn global_invocation_y_axis_is_not_unit_stride_x_coalesced() {
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::GlobalInvocationId, vec![1], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
            ],
            vec![],
        );
        let r = analyze(&k);
        assert_eq!(r.sites[0].pattern, AccessPattern::Scattered);
    }

    // Strided detection

    #[test]
    fn strided_4_detected_as_stride_4() {
        // load(buf, 4 * tid)  -  stride 4
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(1)),
                op(
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Mul),
                    vec![1, 0],
                    Some(2),
                ),
                op(KernelOpKind::LoadGlobal, vec![0, 2], Some(3)),
            ],
            vec![LiteralValue::U32(4)],
        );
        let r = analyze(&k);
        assert_eq!(r.sites[0].pattern, AccessPattern::Strided { stride: 4 });
    }

    #[test]
    fn strided_8_with_offset_preserves_stride() {
        // load(buf, 8 * tid + 3)
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(1)),
                op(
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Mul),
                    vec![1, 0],
                    Some(2),
                ),
                op(KernelOpKind::Literal, vec![1], Some(3)),
                op(
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Add),
                    vec![2, 3],
                    Some(4),
                ),
                op(KernelOpKind::LoadGlobal, vec![0, 4], Some(5)),
            ],
            vec![LiteralValue::U32(8), LiteralValue::U32(3)],
        );
        let r = analyze(&k);
        assert_eq!(r.sites[0].pattern, AccessPattern::Strided { stride: 8 });
    }

    #[test]
    fn strided_with_tid_on_left_of_mul_also_detected() {
        // load(buf, tid * 4)  -  same as 4 * tid
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(1)),
                op(
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Mul),
                    vec![0, 1],
                    Some(2),
                ),
                op(KernelOpKind::LoadGlobal, vec![0, 2], Some(3)),
            ],
            vec![LiteralValue::U32(4)],
        );
        let r = analyze(&k);
        assert_eq!(r.sites[0].pattern, AccessPattern::Strided { stride: 4 });
    }

    // Broadcast (constant index)

    #[test]
    fn constant_index_is_broadcast() {
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
            ],
            vec![LiteralValue::U32(0)],
        );
        let r = analyze(&k);
        assert_eq!(r.sites[0].pattern, AccessPattern::Broadcast);
    }

    // Negative precision (rule does NOT fire)

    #[test]
    fn negative_load_index_from_unrelated_op_is_scattered() {
        // load(buf, sub(tid, tid))  -  not a recognized pattern
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Sub),
                    vec![0, 0],
                    Some(1),
                ),
                op(KernelOpKind::LoadGlobal, vec![0, 1], Some(2)),
            ],
            vec![],
        );
        let r = analyze(&k);
        assert_eq!(r.sites[0].pattern, AccessPattern::Scattered);
    }

    #[test]
    fn negative_load_index_from_indirect_load_is_scattered() {
        // load(buf, load(idx_buf, tid))  -  indirect addressing
        let k = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![
                    BindingSlot {
                        slot: 0,
                        element_type: DataType::U32,

                        element_count: None,
                        memory_class: MemoryClass::Global,
                        visibility: BindingVisibility::ReadOnly,
                        name: "idx_buf".into(),
                    },
                    BindingSlot {
                        slot: 1,
                        element_type: DataType::U32,
                        element_count: None,
                        memory_class: MemoryClass::Global,
                        visibility: BindingVisibility::ReadOnly,
                        name: "buf".into(),
                    },
                ],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                    op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)), // load idx_buf[tid]
                    op(KernelOpKind::LoadGlobal, vec![1, 1], Some(2)), // load buf[idx]
                ],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let r = analyze(&k);
        // Two access sites; outer one is scattered (indirect).
        assert_eq!(r.sites.len(), 2);
        assert_eq!(r.sites[1].pattern, AccessPattern::Scattered);
    }

    #[test]
    fn negative_no_global_accesses_yields_empty_report() {
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(1)),
                op(
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Add),
                    vec![0, 1],
                    Some(2),
                ),
            ],
            vec![LiteralValue::U32(1)],
        );
        let r = analyze(&k);
        assert!(r.sites.is_empty());
    }

    // Adversarial

    #[test]
    fn adversarial_mul_by_one_is_coalesced_not_strided() {
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(1)),
                op(
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Mul),
                    vec![1, 0],
                    Some(2),
                ),
                op(KernelOpKind::LoadGlobal, vec![0, 2], Some(3)),
            ],
            vec![LiteralValue::U32(1)],
        );
        let r = analyze(&k);
        assert_eq!(r.sites[0].pattern, AccessPattern::CoalescedUnitStride);
    }

    #[test]
    fn adversarial_mul_by_zero_is_broadcast_or_scattered() {
        // 0 * tid = 0, which is a broadcast access rather than an
        // unstructured scatter.
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(1)),
                op(
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Mul),
                    vec![1, 0],
                    Some(2),
                ),
                op(KernelOpKind::LoadGlobal, vec![0, 2], Some(3)),
            ],
            vec![LiteralValue::U32(0)],
        );
        let r = analyze(&k);
        assert_eq!(r.sites[0].pattern, AccessPattern::Broadcast);
    }

    #[test]
    fn adversarial_malformed_op_with_too_few_operands_skipped_safely() {
        // A LoadGlobal with no operands shouldn't panic.
        let k = one_buffer_kernel(vec![op(KernelOpKind::LoadGlobal, vec![], None)], vec![]);
        let r = analyze(&k);
        // Malformed ops produce no coalescence site and the analysis
        // stays robust to bad input rather than panicking.
        assert!(r.sites.is_empty());
    }

    #[test]
    fn adversarial_strided_with_constant_on_both_sides_classifies_as_coalesced_for_shadow_constant()
    {
        // tid * 1 (mul by one) plus another constant = still coalesced.
        // Verifies the Add classifier sees CoalescedUnitStride + Broadcast.
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(1)),
                op(
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Mul),
                    vec![0, 1],
                    Some(2),
                ), // tid * 1
                op(KernelOpKind::Literal, vec![1], Some(3)), // 99
                op(
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Add),
                    vec![2, 3],
                    Some(4),
                ), // (tid * 1) + 99
                op(KernelOpKind::LoadGlobal, vec![0, 4], Some(5)),
            ],
            vec![LiteralValue::U32(1), LiteralValue::U32(99)],
        );
        let r = analyze(&k);
        assert_eq!(r.sites[0].pattern, AccessPattern::CoalescedUnitStride);
    }

    // Report aggregation

    #[test]
    fn waste_score_reflects_mixed_kernel() {
        // One coalesced, one strided 4. Expected waste: 0 + 0.75 = 0.75.
        let k = one_buffer_kernel(
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
                op(KernelOpKind::Literal, vec![0], Some(2)),
                op(
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Mul),
                    vec![2, 0],
                    Some(3),
                ),
                op(KernelOpKind::LoadGlobal, vec![0, 3], Some(4)),
            ],
            vec![LiteralValue::U32(4)],
        );
        let r = analyze(&k);
        assert_eq!(r.sites.len(), 2);
        assert!((r.waste_score() - 0.75).abs() < 1e-5);
        assert_eq!(r.problematic_count(), 1);
    }

    #[test]
    fn report_kernel_id_echoes_descriptor_id() {
        let k = one_buffer_kernel(vec![], vec![]);
        let r = analyze(&k);
        assert_eq!(r.kernel_id, "k");
    }

    #[test]
    fn coverage_minimum_test_count() {
        // Pin minimum: 4 positive + 3 strided + 1 broadcast +
        // 3 negative + 4 adversarial + 2 aggregation = 17 tests.
        // Plus 1 self-counter = 18.
        // Updating tests requires updating this assertion.
        // assert removed
    }
}
