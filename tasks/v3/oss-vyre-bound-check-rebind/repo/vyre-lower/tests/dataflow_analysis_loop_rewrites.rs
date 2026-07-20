//! Test: dataflow-analysis loop rewrites.
use vyre_foundation::ir::DataType;
use vyre_lower::{
    analyses::{
        weir_alias::{AliasFactSet, NoAliasFact},
        weir_reaching_def::ReachingDefFactSet,
    },
    rewrites::{
        licm_with_dataflow_analysis_facts, licm_with_weir_alias_facts,
        loop_fission_with_dataflow_analysis_facts, loop_fission_with_weir_alias_facts,
        loop_fusion_with_dataflow_analysis_facts, loop_fusion_with_weir_alias_facts,
    },
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

fn loop_bounds_and_two_children(left: KernelBody, right: KernelBody) -> KernelDescriptor {
    KernelDescriptor {
        id: "dataflow_analysis_loop_pair".into(),
        bindings: BindingLayout {
            slots: vec![binding()],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                literal(2, 30),
                literal(3, 31),
                literal(4, 10),
                literal(5, 11),
                literal(0, 0),
                literal(1, 1),
                KernelOp {
                    kind: KernelOpKind::StructuredForLoop {
                        loop_var: "i".into(),
                    },
                    operands: vec![0, 1, 0],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::StructuredForLoop {
                        loop_var: "i".into(),
                    },
                    operands: vec![0, 1, 1],
                    result: None,
                },
            ],
            child_bodies: vec![left, right],
            literals: literals(),
        },
    }
}

fn literal(literal_index: u32, result: u32) -> KernelOp {
    KernelOp {
        kind: KernelOpKind::Literal,
        operands: vec![literal_index],
        result: Some(result),
    }
}

fn literals() -> Vec<LiteralValue> {
    vec![
        LiteralValue::U32(0),
        LiteralValue::U32(64),
        LiteralValue::U32(7),
        LiteralValue::U32(9),
        LiteralValue::U32(13),
        LiteralValue::U32(17),
    ]
}

fn store_body(index: u32, value: u32) -> KernelBody {
    KernelBody {
        ops: vec![KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![0, index, value],
            result: None,
        }],
        child_bodies: vec![],
        literals: vec![],
    }
}

fn alias_and_reaching() -> (AliasFactSet, ReachingDefFactSet) {
    let mut aliases = AliasFactSet::default();
    aliases.insert_no_alias(NoAliasFact {
        left_binding: 0,
        left_index: 10,
        right_binding: 0,
        right_index: 11,
    });
    aliases.insert_no_alias(NoAliasFact {
        left_binding: 0,
        left_index: 11,
        right_binding: 0,
        right_index: 12,
    });
    let mut reaching = ReachingDefFactSet::default();
    reaching.set_reaching_defs(20, vec![11]);
    (aliases, reaching)
}

fn loop_count(desc: &KernelDescriptor) -> usize {
    desc.body
        .ops
        .iter()
        .filter(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. }))
        .count()
}

#[test]
fn reaching_defs_unlock_alias_proven_loop_fusion() {
    let desc = loop_bounds_and_two_children(store_body(10, 30), store_body(20, 31));
    let (aliases, reaching) = alias_and_reaching();

    assert_eq!(
        loop_count(&loop_fusion_with_weir_alias_facts(&desc, &aliases)),
        2
    );
    assert_eq!(
        loop_count(&loop_fusion_with_dataflow_analysis_facts(
            &desc, &aliases, &reaching
        )),
        1
    );
}

#[test]
fn reaching_defs_unlock_alias_proven_loop_fission() {
    let desc = KernelDescriptor {
        id: "dataflow_analysis_loop_fission".into(),
        bindings: BindingLayout {
            slots: vec![binding()],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                literal(2, 30),
                literal(3, 31),
                literal(4, 10),
                literal(5, 11),
                literal(0, 0),
                literal(1, 1),
                KernelOp {
                    kind: KernelOpKind::StructuredForLoop {
                        loop_var: "i".into(),
                    },
                    operands: vec![0, 1, 0],
                    result: None,
                },
            ],
            child_bodies: vec![KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 10, 30],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 20, 31],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: literals(),
        },
    };
    let (aliases, reaching) = alias_and_reaching();

    assert_eq!(
        loop_count(&loop_fission_with_weir_alias_facts(&desc, &aliases)),
        1
    );
    assert_eq!(
        loop_count(&loop_fission_with_dataflow_analysis_facts(
            &desc, &aliases, &reaching
        )),
        2
    );
}

#[test]
fn reaching_defs_unlock_alias_proven_licm_load_hoist() {
    let desc = KernelDescriptor {
        id: "dataflow_analysis_licm".into(),
        bindings: BindingLayout {
            slots: vec![binding()],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                literal(4, 31),
                literal(0, 0),
                literal(1, 1),
                literal(2, 11),
                KernelOp {
                    kind: KernelOpKind::Copy,
                    operands: vec![11],
                    result: Some(20),
                },
                literal(3, 12),
                KernelOp {
                    kind: KernelOpKind::Copy,
                    operands: vec![12],
                    result: Some(40),
                },
                KernelOp {
                    kind: KernelOpKind::StructuredForLoop {
                        loop_var: "i".into(),
                    },
                    operands: vec![0, 1, 0],
                    result: None,
                },
            ],
            child_bodies: vec![KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 20],
                        result: Some(30),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 40, 31],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(64),
                LiteralValue::U32(7),
                LiteralValue::U32(9),
                LiteralValue::U32(13),
            ],
        },
    };
    let (aliases, mut reaching) = alias_and_reaching();
    reaching.set_reaching_defs(40, vec![12]);

    assert_eq!(
        top_level_load_count(&licm_with_weir_alias_facts(&desc, &aliases)),
        0
    );
    assert_eq!(
        top_level_load_count(&licm_with_dataflow_analysis_facts(
            &desc, &aliases, &reaching
        )),
        1
    );
}

fn top_level_load_count(desc: &KernelDescriptor) -> usize {
    desc.body
        .ops
        .iter()
        .filter(|op| matches!(op.kind, KernelOpKind::LoadGlobal))
        .count()
}
