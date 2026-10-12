//! Detect read-only bindings with multiple LoadGlobal sites  -  the
//! basic precondition for texture-memory promotion.

use super::plan::{TextureCandidate, TexturePromotionPlan};
use crate::analyses::load_counts::count_global_loads_by_slot;
use crate::{BindingVisibility, KernelDescriptor, MemoryClass};
use rustc_hash::{FxHashMap, FxHashSet};

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> TexturePromotionPlan {
    // Eligible bindings: Global memory class, ReadOnly visibility.
    let eligible: FxHashSet<u32> = desc
        .bindings
        .slots
        .iter()
        .filter(|b| {
            matches!(b.memory_class, MemoryClass::Global)
                && matches!(b.visibility, BindingVisibility::ReadOnly)
        })
        .map(|b| b.slot)
        .collect();

    let mut load_counts: FxHashMap<u32, u32> =
        FxHashMap::with_capacity_and_hasher(eligible.len(), Default::default());
    count_global_loads_by_slot(
        &desc.body,
        &|slot| eligible.contains(&slot),
        &mut load_counts,
    );

    let mut candidates = Vec::new();
    for (slot, count) in load_counts {
        if count >= 2 {
            let speedup = 1.5 + (count as f32).log2();
            candidates.push(TextureCandidate {
                binding_slot: slot,
                load_count: count,
                estimated_speedup_factor: speedup,
            });
        }
    }
    candidates.sort_unstable_by_key(|candidate| candidate.binding_slot);

    TexturePromotionPlan {
        kernel_id: desc.id.clone(),
        candidates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind,
        LiteralValue,
    };
    use vyre_foundation::ir::DataType;

    fn ro_binding(slot: u32) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: DataType::F32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadOnly,
            name: format!("ro{slot}"),
        }
    }

    fn rw_binding(slot: u32) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: DataType::F32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: format!("rw{slot}"),
        }
    }

    #[test]
    fn empty_kernel_has_no_candidates() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let p = analyze(&desc);
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn read_only_binding_with_two_loads_is_candidate() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![ro_binding(0)],
            },
            dispatch: Dispatch::new(32, 1, 1),
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
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let p = analyze(&desc);
        assert_eq!(p.candidates.len(), 1);
        assert_eq!(p.candidates[0].binding_slot, 0);
        assert_eq!(p.candidates[0].load_count, 2);
    }

    #[test]
    fn read_write_binding_is_not_candidate() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![rw_binding(0)],
            },
            dispatch: Dispatch::new(32, 1, 1),
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
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let p = analyze(&desc);
        assert!(
            p.candidates.is_empty(),
            "RW bindings can't be promoted to texture"
        );
    }

    #[test]
    fn read_only_binding_with_one_load_is_not_candidate() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![ro_binding(0)],
            },
            dispatch: Dispatch::new(32, 1, 1),
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
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let p = analyze(&desc);
        assert!(
            p.candidates.is_empty(),
            "single-load bindings don't gain enough"
        );
    }

    #[test]
    fn shared_memory_binding_is_not_candidate() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::F32,
                    element_count: Some(64),
                    memory_class: MemoryClass::Shared,
                    visibility: BindingVisibility::ReadOnly,
                    name: "shared".into(),
                }],
            },
            dispatch: Dispatch::new(32, 1, 1),
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
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let p = analyze(&desc);
        assert!(
            p.candidates.is_empty(),
            "shared memory isn't promotable to texture"
        );
    }

    #[test]
    fn speedup_grows_with_load_count_log2() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![ro_binding(0)],
            },
            dispatch: Dispatch::new(32, 1, 1),
            body: KernelBody {
                ops: {
                    let mut ops = vec![KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    }];
                    for i in 1..=8 {
                        ops.push(KernelOp {
                            kind: KernelOpKind::LoadGlobal,
                            operands: vec![0, 0],
                            result: Some(i),
                        });
                    }
                    ops
                },
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let p = analyze(&desc);
        assert_eq!(p.candidates.len(), 1);
        assert_eq!(p.candidates[0].load_count, 8);
        // 1.5 + log2(8) = 4.5
        assert!((p.candidates[0].estimated_speedup_factor - 4.5).abs() < 1e-5);
    }

    #[test]
    fn loop_bounds_are_not_treated_as_child_body_indices() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![ro_binding(0)],
            },
            dispatch: Dispatch::new(32, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredForLoop {
                            loop_var: "i".into(),
                        },
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![
                    KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::LoadGlobal,
                            operands: vec![0, 0],
                            result: Some(1),
                        }],
                        child_bodies: vec![],
                        literals: vec![],
                    },
                    KernelBody {
                        ops: vec![],
                        child_bodies: vec![],
                        literals: vec![],
                    },
                ],
                literals: vec![LiteralValue::U32(0)],
            },
        };

        let p = analyze(&desc);
        assert!(
            p.candidates.is_empty(),
            "loop bound operands must not cause traversal into unrelated child bodies"
        );
    }
}
