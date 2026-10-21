//! Promote eligible read-only global buffers to constant bindings.
//!
//! The analysis in `analyses::const_buffer_promote` identifies small
//! fixed-size read-only global buffers with repeated loads. This rewrite makes
//! that decision material: it changes the binding memory class to `Constant`
//! and rewrites all matching `LoadGlobal` ops to `LoadConstant` across nested
//! bodies. Stores/atomics/async stores against a candidate slot veto the
//! promotion even if a malformed descriptor claims the binding is read-only.

use crate::{KernelBody, KernelDescriptor, KernelOpKind, MemoryClass};
use rustc_hash::FxHashSet;

/// Promote constant-buffer candidates using the default 64 KiB budget.
#[must_use]
pub fn const_buffer_promote(desc: &KernelDescriptor) -> KernelDescriptor {
    const_buffer_promote_with_budget(
        desc,
        crate::analyses::const_buffer_promote::DEFAULT_CONST_BUFFER_BUDGET_BYTES,
    )
}

/// Promote constant-buffer candidates using a caller-provided byte budget.
#[must_use]
pub fn const_buffer_promote_with_budget(
    desc: &KernelDescriptor,
    budget_bytes: u32,
) -> KernelDescriptor {
    let plan = crate::analyses::const_buffer_promote::analyze_with_budget(desc, budget_bytes);
    if plan.candidates.is_empty() {
        return desc.clone();
    }

    let candidates = plan
        .candidates
        .iter()
        .map(|candidate| candidate.binding_slot)
        .filter(|slot| !slot_has_writes(&desc.body, *slot))
        .collect::<FxHashSet<_>>();
    if candidates.is_empty() {
        return desc.clone();
    }

    let mut out = desc.clone();
    let mut promoted_slots = FxHashSet::default();
    for binding in &mut out.bindings.slots {
        if candidates.contains(&binding.slot) {
            binding.memory_class = MemoryClass::Constant;
            promoted_slots.insert(binding.slot);
        }
    }
    if promoted_slots.is_empty() {
        return desc.clone();
    }
    rewrite_body_loads(&mut out.body, &promoted_slots);
    out
}

fn rewrite_body_loads(body: &mut KernelBody, slots: &FxHashSet<u32>) {
    for op in &mut body.ops {
        if matches!(op.kind, KernelOpKind::LoadGlobal)
            && op.operands.first().is_some_and(|slot| slots.contains(slot))
        {
            op.kind = KernelOpKind::LoadConstant;
        }
    }
    for child in &mut body.child_bodies {
        rewrite_body_loads(child, slots);
    }
}

fn slot_has_writes(body: &KernelBody, slot: u32) -> bool {
    body.ops.iter().any(|op| match &op.kind {
        KernelOpKind::StoreGlobal
        | KernelOpKind::StoreShared
        | KernelOpKind::Atomic { .. }
        | KernelOpKind::AsyncStore { .. } => op.operands.first().copied() == Some(slot),
        _ => false,
    }) || body
        .child_bodies
        .iter()
        .any(|child| slot_has_writes(child, slot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelOp, LiteralValue,
    };
    use vyre_foundation::ir::DataType;

    fn op(kind: KernelOpKind, operands: Vec<u32>, result: Option<u32>) -> KernelOp {
        KernelOp {
            kind,
            operands,
            result,
        }
    }

    fn ro_global(slot: u32, count: u32) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: DataType::F32,
            element_count: Some(count),
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadOnly,
            name: format!("ro{slot}"),
        }
    }

    fn kernel(ops: Vec<KernelOp>, child_bodies: Vec<KernelBody>) -> KernelDescriptor {
        KernelDescriptor {
            id: "const".into(),
            bindings: BindingLayout {
                slots: vec![ro_global(0, 16)],
            },
            dispatch: Dispatch::new(32, 1, 1),
            body: KernelBody {
                ops,
                child_bodies,
                literals: vec![LiteralValue::U32(0), LiteralValue::F32(1.0)],
            },
        }
    }

    #[test]
    fn promotes_repeated_read_only_global_loads() {
        let input = kernel(
            vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(2)),
            ],
            vec![],
        );
        let output = const_buffer_promote(&input);

        assert_eq!(output.bindings.slots[0].memory_class, MemoryClass::Constant);
        assert!(matches!(
            output.body.ops[1].kind,
            KernelOpKind::LoadConstant
        ));
        assert!(matches!(
            output.body.ops[2].kind,
            KernelOpKind::LoadConstant
        ));
    }

    #[test]
    fn rewrites_nested_body_loads() {
        let child = KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(2)),
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0)],
        };
        let input = kernel(
            vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::StructuredBlock, vec![0], None),
            ],
            vec![child],
        );
        let output = const_buffer_promote(&input);

        assert_eq!(output.bindings.slots[0].memory_class, MemoryClass::Constant);
        assert!(matches!(
            output.body.child_bodies[0].ops[1].kind,
            KernelOpKind::LoadConstant
        ));
        assert!(matches!(
            output.body.child_bodies[0].ops[2].kind,
            KernelOpKind::LoadConstant
        ));
    }

    #[test]
    fn write_veto_keeps_descriptor_unchanged() {
        let input = kernel(
            vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::Literal, vec![1], Some(1)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(2)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(3)),
                op(KernelOpKind::StoreGlobal, vec![0, 0, 1], None),
            ],
            vec![],
        );
        let output = const_buffer_promote(&input);

        assert_eq!(output, input);
    }

    #[test]
    fn budget_veto_keeps_descriptor_unchanged() {
        let input = kernel(
            vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(2)),
            ],
            vec![],
        );
        let output = const_buffer_promote_with_budget(&input, 32);

        assert_eq!(output, input);
    }
}
