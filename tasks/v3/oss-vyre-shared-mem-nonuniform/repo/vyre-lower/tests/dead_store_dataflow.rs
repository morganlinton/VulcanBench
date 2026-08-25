//! Test: dead store dataflow.
use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::{
    analyses::{alias_facts::AliasFactSet, reaching_def_facts::ReachingDefFactSet},
    rewrites::{dead_store, dead_store_with_dataflow_facts, run_all, run_all_with_dataflow_facts},
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

fn binding() -> BindingSlot {
    BindingSlot {
        slot: 0,
        element_type: DataType::U32,
        element_count: None,
        memory_class: MemoryClass::Global,
        visibility: BindingVisibility::ReadWrite,
        name: "buf".into(),
    }
}

#[test]
fn dataflow_pipeline_applies_reaching_def_memory_dse() {
    let desc = KernelDescriptor {
        id: "dataflow_pipeline_dead_store".into(),
        bindings: BindingLayout {
            slots: vec![binding()],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(10),
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
                    kind: KernelOpKind::Copy,
                    operands: vec![10],
                    result: Some(11),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 10, 1],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 11, 2],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(3),
                LiteralValue::U32(9),
            ],
        },
    };

    assert_eq!(store_count(&run_all(&desc)), 2);

    let alias_facts = AliasFactSet::default();
    let mut reaching_defs = ReachingDefFactSet::default();
    reaching_defs.set_reaching_defs(11, vec![10]);

    let optimized = run_all_with_dataflow_facts(&desc, &alias_facts, &reaching_defs);
    assert_eq!(store_count(&optimized), 1);
}

fn store_count(desc: &KernelDescriptor) -> usize {
    desc.body
        .ops
        .iter()
        .filter(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
        .count()
}

#[test]
fn reaching_def_factss_canonicalize_equivalent_store_indices_for_dse() {
    let desc = KernelDescriptor {
        id: "dataflow_dead_store".into(),
        bindings: BindingLayout {
            slots: vec![binding()],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(10),
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
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![10, 1],
                    result: Some(11),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 10, 1],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 11, 2],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(0),
                LiteralValue::U32(7),
            ],
        },
    };

    assert_eq!(store_count(&dead_store(&desc)), 2);

    let alias_facts = AliasFactSet::default();
    let mut reaching_defs = ReachingDefFactSet::default();
    reaching_defs.set_reaching_defs(11, vec![10]);

    let optimized = dead_store_with_dataflow_facts(&desc, &alias_facts, &reaching_defs);
    assert_eq!(store_count(&optimized), 1);
    assert_eq!(
        optimized
            .body
            .ops
            .iter()
            .find(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
            .expect("Fix: optimized fixture must retain the final observable store")
            .operands,
        vec![0, 11, 2]
    );
}
