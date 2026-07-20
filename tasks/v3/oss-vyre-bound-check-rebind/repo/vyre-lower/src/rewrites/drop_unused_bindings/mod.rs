//! Drop unused binding slots from the descriptor.
//!
//! Walks the descriptor; collects every `BindingSlot.slot` value
//! referenced by `LoadGlobal`/`LoadShared`/`LoadConstant`/
//! `StoreGlobal`/`StoreShared`/`Atomic`/`AsyncLoad`/`AsyncStore`/
//! `BufferLength` ops. Filters `desc.bindings.slots` to keep only
//! those whose `.slot` value is in the referenced set. Host-visible
//! bindings (`Global`/`Constant`) are retained even when currently
//! unreferenced because the dispatch ABI is slot-addressed by the host.
//! Backend-local bindings (`Shared`/`Scratch`) may be dropped when no op
//! references them.
//!
//! ## No renumbering needed
//!
//! Emitters look up bindings by `BindingSlot.slot` (the canonical id),
//! not by `Vec` position. Op operands carry slot ids that match `.slot`
//! directly. Dropping unreferenced entries from the Vec leaves all
//! surviving operands valid.
//!
//! ## When does this fire?
//!
//! Surprisingly often. Kernel templates often declare a "scratch"
//! binding that the rewrites then optimize away (e.g., redundant
//! Loads forwarded out, Stores collapsed by dead_store). After the
//! arithmetic rewrites finish, the binding may have zero remaining
//! references  -  dropping it saves the host from binding a buffer it
//! doesn't need.

use crate::{KernelBody, KernelDescriptor, KernelOpKind};
use rustc_hash::FxHashSet;

#[must_use]
pub fn drop_unused_bindings(desc: &KernelDescriptor) -> KernelDescriptor {
    let referenced = collect_referenced_slots(desc);

    let mut out = desc.clone();
    // Retain rule: a binding is part of the host dispatch contract
    // and must NOT be dropped when:
    //   1. some op references it (the obvious case);
    //   2. it is WriteOnly  -  the host expects readback;
    //   3. it is ReadWrite  -  the host passed input bytes for it AND
    //      expects the modified contents back, so dropping it from
    //      the descriptor breaks the dispatch ABI even when no op
    //      currently reads it (multi-stage pipelines like
    //      `c11_lex_digraphs` declare `tok_starts` / `tok_lens`
    //      ReadWrite so the surrounding pipeline passes them through);
    //   4. it is a host-visible binding  -  the host passes buffers
    //      positionally,
    //      so silently dropping a ReadOnly binding shifts every later
    //      input slot's index. Real-world parsers like
    //      `c11_annotate_typedef_names` declare a `haystack` ReadOnly
    //      that aggressive constant-folding (e.g. when the upstream
    //      stage returns a bogus count of 1) can DCE every load from,
    //      but the host dispatch still expects to address it by slot.
    //      Treating ReadOnly as "scratch droppable" was a false
    //      economy: genuine droppable scratch is Shared/Scratch memory.
    out.bindings.slots.retain(|s| {
        referenced.contains(&s.slot)
            || matches!(
                s.memory_class,
                crate::MemoryClass::Global
                    | crate::MemoryClass::Constant
                    | crate::MemoryClass::Uniform
            )
    });
    out
}

fn collect_referenced_slots(desc: &KernelDescriptor) -> FxHashSet<u32> {
    let mut acc =
        FxHashSet::with_capacity_and_hasher(desc.bindings.slots.len(), Default::default());
    walk(&desc.body, &mut acc);
    acc
}

fn walk(body: &KernelBody, acc: &mut FxHashSet<u32>) {
    for op in &body.ops {
        if let Some(slot) = slot_operand(&op.kind, &op.operands) {
            acc.insert(slot);
        }
    }
    for child in &body.child_bodies {
        walk(child, acc);
    }
}

fn slot_operand(kind: &KernelOpKind, operands: &[u32]) -> Option<u32> {
    use KernelOpKind::*;
    match kind {
        LoadGlobal
        | LoadShared
        | LoadConstant
        | StoreGlobal
        | StoreShared
        | Atomic { .. }
        | AsyncLoad { .. }
        | AsyncStore { .. }
        | BufferLength => operands.first().copied(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };
    use vyre_foundation::ir::DataType;

    fn slot(id: u32, name: &str) -> BindingSlot {
        BindingSlot {
            slot: id,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: name.into(),
        }
    }

    /// Input-style host-visible slot. These are retained even when
    /// unreferenced because the host dispatch ABI is slot-addressed.
    fn input_slot(id: u32, name: &str) -> BindingSlot {
        BindingSlot {
            slot: id,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadOnly,
            name: name.into(),
        }
    }

    fn shared_slot(id: u32, name: &str) -> BindingSlot {
        BindingSlot {
            slot: id,
            element_type: DataType::U32,
            element_count: Some(16),
            memory_class: MemoryClass::Shared,
            visibility: BindingVisibility::ReadWrite,
            name: name.into(),
        }
    }

    #[test]
    fn no_bindings_no_op() {
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
        let out = drop_unused_bindings(&desc);
        assert!(out.bindings.slots.is_empty());
    }

    #[test]
    fn all_referenced_unchanged() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![slot(0, "buf0"), slot(1, "buf1")],
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
                        operands: vec![0, 0, 1], // slot 0
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![1, 0, 1], // slot 1
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let out = drop_unused_bindings(&desc);
        assert_eq!(out.bindings.slots.len(), 2);
    }

    #[test]
    fn declared_bindings_retained_even_when_unreferenced() {
        // ALL declared bindings are part of the host dispatch contract
        // and survive this rewrite, regardless of visibility. ReadOnly
        // bindings used to be droppable when no op referenced them, but
        // that broke the positional input-mapping in the dispatcher
        // when aggressive constant-folding DCE'd the loads (e.g. the
        // c11 parser pipeline's `haystack` input when an upstream stage
        // returns a 1-node count). True scratch is Local/Shared memory
        // class  -  never a host-visible binding.
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![
                    slot(0, "buf0"),
                    input_slot(1, "buf1_unused"),
                    slot(2, "buf2"),
                ],
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
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![2, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let out = drop_unused_bindings(&desc);
        assert_eq!(out.bindings.slots.len(), 3);
        assert!(out.bindings.slots.iter().any(|s| s.slot == 0));
        assert!(out.bindings.slots.iter().any(|s| s.slot == 1));
        assert!(out.bindings.slots.iter().any(|s| s.slot == 2));
    }

    #[test]
    fn child_body_references_keep_slot() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![input_slot(0, "buf0"), slot(7, "buf7_in_child")],
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
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![0, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![
                        KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![0],
                            result: Some(0),
                        },
                        KernelOp {
                            kind: KernelOpKind::StoreGlobal,
                            operands: vec![7, 0, 0], // refers to slot 7 in the child
                            result: None,
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(99)],
                }],
                literals: vec![LiteralValue::U32(1)],
            },
        };
        let out = drop_unused_bindings(&desc);
        // Both slots survive: declared bindings are part of the host
        // dispatch contract, even when only the child body touches one.
        assert!(out.bindings.slots.iter().any(|s| s.slot == 7));
        assert!(out.bindings.slots.iter().any(|s| s.slot == 0));
    }

    #[test]
    fn idempotent() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![slot(0, "a"), input_slot(1, "b_unused")],
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
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(1)],
            },
        };
        let once = drop_unused_bindings(&desc);
        let twice = drop_unused_bindings(&once);
        assert_eq!(once.bindings.slots, twice.bindings.slots);
        assert_eq!(once.bindings.slots.len(), 2);
    }

    #[test]
    fn loadglobal_keeps_its_slot() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![slot(3, "buf3"), input_slot(99, "unused")],
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
                        operands: vec![3, 0],
                        result: Some(1),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let out = drop_unused_bindings(&desc);
        assert!(out.bindings.slots.iter().any(|s| s.slot == 3));
        assert!(out.bindings.slots.iter().any(|s| s.slot == 99));
    }

    #[test]
    fn writeonly_and_readwrite_outputs_retained_when_unreferenced() {
        // Soundness: a binding declared as a host-visible output
        // (WriteOnly or ReadWrite) is part of the dispatch contract.
        // Even if every Store to it got DCE'd by an upstream pass,
        // the host's output_binding_layouts() still expects the slot
        // to exist in the lowered descriptor. This test pins the rule
        // by declaring 4 unreferenced bindings  -  2 ReadOnly inputs
        // (must be dropped) and 2 outputs (must survive).
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![
                    input_slot(0, "in_unused"),
                    BindingSlot {
                        slot: 1,
                        element_type: DataType::U32,
                        element_count: None,
                        memory_class: MemoryClass::Global,
                        visibility: BindingVisibility::WriteOnly,
                        name: "out_writeonly".into(),
                    },
                    input_slot(2, "in_unused2"),
                    BindingSlot {
                        slot: 3,
                        element_type: DataType::U32,
                        element_count: None,
                        memory_class: MemoryClass::Global,
                        visibility: BindingVisibility::ReadWrite,
                        name: "out_readwrite".into(),
                    },
                ],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let out = drop_unused_bindings(&desc);
        assert!(
            out.bindings.slots.iter().any(|s| s.slot == 1),
            "WriteOnly output must survive even when unreferenced"
        );
        assert!(
            out.bindings.slots.iter().any(|s| s.slot == 3),
            "ReadWrite output must survive even when unreferenced"
        );
        assert!(
            out.bindings.slots.iter().any(|s| s.slot == 0),
            "ReadOnly input is part of the dispatch contract; survives"
        );
        assert!(
            out.bindings.slots.iter().any(|s| s.slot == 2),
            "ReadOnly input is part of the dispatch contract; survives"
        );
    }

    #[test]
    fn unreferenced_backend_local_bindings_are_dropped() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![input_slot(0, "host_input"), shared_slot(9, "unused_shared")],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let out = drop_unused_bindings(&desc);
        assert!(out.bindings.slots.iter().any(|slot| slot.slot == 0));
        assert!(!out.bindings.slots.iter().any(|slot| slot.slot == 9));
    }
}
