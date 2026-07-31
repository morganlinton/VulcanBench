use super::*;

#[test]
fn emit_uniform_load_uses_readonly_global_addressing() {
    let mut desc = two_slot_u32_kernel(
        "uniform_load",
        vec![
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
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 0, 1],
                result: None,
            },
        ],
        vec![LiteralValue::U32(0)],
    );
    desc.bindings.slots[0].memory_class = MemoryClass::Uniform;
    let s = emit(&desc).unwrap();
    assert!(s.contains("ld.global"), "{s}");
    assert!(s.contains("st.global.u32"), "{s}");
}

#[test]
fn emit_hoists_ready_pure_op_into_vector_load_gap() {
    let s = emit(&two_slot_u32_kernel(
        "scheduled_vector_load_gap",
        vec![
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
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 0],
                result: Some(2),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![0, 1],
                result: Some(3),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 3],
                result: Some(4),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![3, 1],
                result: Some(5),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 5],
                result: Some(6),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![5, 1],
                result: Some(7),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 7],
                result: Some(8),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![2],
                result: Some(9),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![9, 1],
                result: Some(10),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 0, 8],
                result: None,
            },
        ],
        vec![
            LiteralValue::U32(0),
            LiteralValue::U32(1),
            LiteralValue::U32(11),
        ],
    ))
    .unwrap();

    let ld = s
        .find("ld.global.nc.v4.u32")
        .expect("test kernel must contain a fused vector load");
    let schedule_first = s
        .find("// schedule: hoist independent op#9 into vector-load gap after op#2")
        .expect("PTX emitter must hoist ready independent literal work after fused vector loads");
    let schedule_second = s
        .find("// schedule: hoist independent op#10 into vector-load gap after op#2")
        .expect("PTX emitter must keep filling fused vector-load gaps with newly-ready pure work");
    let store = s
        .find("st.global.u32")
        .expect("test kernel must contain the final global store");

    assert!(
        ld < schedule_first && schedule_first < schedule_second && schedule_second < store,
        "Fix: vector-load scheduling should hide packed-load latency before the visible store.\n{s}"
    );
}

#[test]
fn emit_hoists_ready_pure_op_into_load_use_gap() {
    let s = emit(&two_slot_u32_kernel(
        "scheduled_load_gap",
        vec![
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
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 0],
                result: Some(2),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![2, 1],
                result: Some(3),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![2],
                result: Some(4),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![4, 1],
                result: Some(5),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 0, 3],
                result: None,
            },
        ],
        vec![
            LiteralValue::U32(0),
            LiteralValue::U32(7),
            LiteralValue::U32(11),
        ],
    ))
    .unwrap();

    let ld = s
        .find("ld.global.u32")
        .expect("test kernel must contain a scalar global load");
    let schedule_first = s
        .find("// schedule: hoist independent op#4 into load-use gap after op#2")
        .expect("PTX emitter must hoist a ready independent op into the load-use gap");
    let schedule_second = s
        .find("// schedule: hoist independent op#5 into load-use gap after op#2")
        .expect("PTX emitter must keep filling the load-use gap with newly-ready independent work");
    let store = s
        .find("st.global.u32")
        .expect("test kernel must contain the final global store");

    assert!(
        ld < schedule_first && schedule_first < schedule_second && schedule_second < store,
        "Fix: B9 scheduling should place all ready independent pure work between a load and its visible memory effect.\n{s}"
    );
}

#[test]
fn emit_uses_read_only_cache_loads_for_texture_promoted_bindings() {
    let s = emit(&two_slot_u32_kernel(
        "readonly_cache_loads",
        vec![
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
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 0, 2],
                result: None,
            },
        ],
        vec![LiteralValue::U32(0)],
    ))
    .unwrap();

    assert!(
        s.contains("ld.global.nc.u32"),
        "Fix: repeated read-only global loads should use CUDA's read-only/non-coherent cache path.\n{s}"
    );
}

#[test]
fn emit_keeps_read_write_loads_on_coherent_global_path() {
    let desc = KernelDescriptor {
        id: "rw_global_loads".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(16),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "rw".into(),
            }],
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
    let s = emit(&desc).unwrap();

    assert!(s.contains("ld.global.u32"));
    assert!(
        !s.contains("ld.global.nc.u32"),
        "Fix: ReadWrite bindings must not use the non-coherent read-only cache path.\n{s}"
    );
}

