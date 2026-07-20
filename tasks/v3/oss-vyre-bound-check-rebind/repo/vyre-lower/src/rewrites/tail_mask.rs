//! Tail-mask store predication for power-of-two dispatch coercion.
//!
//! `vyre-driver::program_walks::coerce_to_pow2_with_tail_mask` decides
//! when a launch may be rounded up to a uniform power-of-two element
//! count. This descriptor rewrite is the lowering-side safety half: it
//! wraps global/shared stores whose index is a global lane id in
//! `if index < logical_element_count`.

// Tainted-ids tracking is u32 keys, not adversarial input. FxHash is
// noticeably faster than std SipHash on small integer keys, and this
// set is consulted per op + cloned per recursion.
use rustc_hash::FxHashSet as HashSet;

use vyre_foundation::ir::BinOp;

use super::literal::ResultAllocator;
use crate::{KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue};

/// Predicate eligible stores with `index < logical_element_count`.
///
/// This pass is intentionally opt-in: callers run it only when they
/// actually round the dispatch element count above the logical output
/// element count. If `logical_element_count == 0`, the descriptor is
/// returned unchanged because dispatching zero logical elements should
/// be handled by the launch planner.
#[must_use]
pub fn apply_tail_mask(desc: &KernelDescriptor, logical_element_count: u32) -> KernelDescriptor {
    if logical_element_count == 0 {
        return desc.clone();
    }
    let mut out = desc.clone();
    let mut allocator = ResultAllocator::for_body_tree(&out.body);
    let inherited = HashSet::default();
    apply_to_body(
        &mut out.body,
        logical_element_count,
        &mut allocator,
        &inherited,
    );
    out
}

fn apply_to_body(
    body: &mut KernelBody,
    logical_element_count: u32,
    allocator: &mut ResultAllocator,
    inherited_tainted_ids: &HashSet<u32>,
) {
    let mut tainted_ids = inherited_tainted_ids.clone();

    // Children that existed BEFORE this pass touched `body`. Wrapper
    // child bodies appended below contain only a single masked op moved
    // verbatim from the parent; recursing into them re-tail-masks the
    // same op forever (the inherited taint set still contains the
    // index id), so we recurse into pre-existing children only.
    let original_child_count = body.child_bodies.len();

    let original_ops = std::mem::take(&mut body.ops);
    let mut rewritten = Vec::with_capacity(original_ops.len());
    for op in original_ops {
        // Capture taint metadata before consuming `op`  -  the if-branch
        // moves `op` into a child body, so we cannot borrow it afterwards.
        let taint_kind = op.kind.clone();
        let taint_operands = op.operands.clone();
        let taint_result = op.result;

        if let Some(index_id) = tail_mask_index(&op, &tainted_ids) {
            let limit_id = allocator.push_literal(
                &mut rewritten,
                &mut body.literals,
                LiteralValue::U32(logical_element_count),
            );
            let cond_id = allocator.push_result(
                &mut rewritten,
                KernelOpKind::BinOpKind(BinOp::Lt),
                vec![index_id, limit_id],
            );

            let child_idx = body.child_bodies.len() as u32;
            body.child_bodies.push(KernelBody {
                ops: vec![op],
                child_bodies: Vec::new(),
                literals: Vec::new(),
            });
            rewritten.push(KernelOp {
                kind: KernelOpKind::StructuredIfThen,
                operands: vec![cond_id, child_idx],
                result: None,
            });
        } else {
            rewritten.push(op);
        }

        mark_tainted_from_parts(&taint_kind, &taint_operands, taint_result, &mut tainted_ids);
    }
    body.ops = rewritten;

    for child in body.child_bodies[..original_child_count].iter_mut() {
        apply_to_body(child, logical_element_count, allocator, &tainted_ids);
    }
}

fn tail_mask_index(op: &KernelOp, tainted_ids: &HashSet<u32>) -> Option<u32> {
    let index_operand = masked_index_operand(op)?;
    let index_id = op.operands.get(index_operand).copied()?;
    tainted_ids.contains(&index_id).then_some(index_id)
}

fn masked_index_operand(op: &KernelOp) -> Option<usize> {
    match op.kind {
        KernelOpKind::LoadGlobal
        | KernelOpKind::LoadShared
        | KernelOpKind::LoadConstant
        | KernelOpKind::StoreGlobal
        | KernelOpKind::StoreShared
        | KernelOpKind::Atomic { .. } => Some(1),
        KernelOpKind::AsyncLoad { .. } | KernelOpKind::AsyncStore { .. } => Some(2),
        _ => None,
    }
}

fn mark_tainted_from_parts(
    kind: &KernelOpKind,
    operands: &[u32],
    result: Option<u32>,
    tainted_ids: &mut HashSet<u32>,
) {
    let is_lane_x = matches!(kind, KernelOpKind::GlobalInvocationId)
        && operands.first().copied().unwrap_or(0) == 0;
    let flows_from_tainted = operands.iter().any(|operand| tainted_ids.contains(operand));

    if is_lane_x || flows_from_tainted {
        if let Some(result) = result {
            tainted_ids.insert(result);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelDescriptor, MemoryClass,
    };
    use vyre_foundation::ir::DataType;

    fn desc_with_store_index(index_kind: KernelOpKind) -> KernelDescriptor {
        KernelDescriptor {
            id: "tail-mask-test".to_string(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: Some(100),
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::WriteOnly,
                    name: "out".to_string(),
                }],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: index_kind,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: Vec::new(),
                literals: vec![LiteralValue::U32(7)],
            },
        }
    }

    #[test]
    fn masks_global_lane_store() {
        let desc = desc_with_store_index(KernelOpKind::GlobalInvocationId);
        let out = apply_tail_mask(&desc, 100);
        crate::verify::verify(&out)
            .expect("Fix: tail-mask rewrite must preserve descriptor invariants");
        assert_eq!(out.body.child_bodies.len(), 1);
        assert!(matches!(
            out.body.ops.last().map(|op| &op.kind),
            Some(KernelOpKind::StructuredIfThen)
        ));
        assert!(matches!(
            out.body.ops[out.body.ops.len() - 2].kind,
            KernelOpKind::BinOpKind(BinOp::Lt)
        ));
        assert_eq!(out.body.literals.last(), Some(&LiteralValue::U32(100)));
        assert!(matches!(
            out.body.child_bodies[0].ops[0].kind,
            KernelOpKind::StoreGlobal
        ));
    }

    #[test]
    fn leaves_non_global_lane_store_unmasked() {
        let desc = desc_with_store_index(KernelOpKind::LocalInvocationId);
        let out = apply_tail_mask(&desc, 100);
        assert_eq!(out, desc);
    }

    #[test]
    fn zero_logical_count_is_noop() {
        let desc = desc_with_store_index(KernelOpKind::GlobalInvocationId);
        let out = apply_tail_mask(&desc, 0);
        assert_eq!(out, desc);
    }

    #[test]
    fn generated_result_ids_follow_existing_ids() {
        let desc = desc_with_store_index(KernelOpKind::GlobalInvocationId);
        let out = apply_tail_mask(&desc, 100);
        let lit = &out.body.ops[out.body.ops.len() - 3];
        let cond = &out.body.ops[out.body.ops.len() - 2];
        assert_eq!(lit.result, Some(2));
        assert_eq!(cond.result, Some(3));
    }

    #[test]
    fn wrapper_child_is_not_re_tail_masked() {
        // Adversarial: catches the prior infinite-recursion in
        // apply_to_body where the freshly-pushed StructuredIfThen wrapper
        // body inherited the tainted index id from the parent and re-wrapped
        // its sole StoreGlobal forever. Single mask, single nested level  -
        // not nested wrappers all the way down.
        let desc = desc_with_store_index(KernelOpKind::GlobalInvocationId);
        let out = apply_tail_mask(&desc, 100);
        assert_eq!(
            out.body.child_bodies.len(),
            1,
            "exactly one wrapper child must be appended; recursion must not produce nested wrappers"
        );
        let wrapper = &out.body.child_bodies[0];
        assert!(
            wrapper.child_bodies.is_empty(),
            "wrapper body must contain no nested wrapper child  -  re-masking would have appended one"
        );
        assert_eq!(
            wrapper.ops.len(),
            1,
            "wrapper body must contain exactly the one moved StoreGlobal op"
        );
        assert!(matches!(wrapper.ops[0].kind, KernelOpKind::StoreGlobal));
    }
}
