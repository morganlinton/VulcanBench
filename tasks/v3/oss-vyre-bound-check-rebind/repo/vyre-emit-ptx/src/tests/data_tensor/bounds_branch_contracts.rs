use super::*;

#[test]
fn runtime_index_load_clamps_against_buffer_length() {
    // PTX has no built-in bounds check. Speculative loads in `Expr::select`
    // arms can read past buffer end -> CUDA_ERROR_ILLEGAL_ADDRESS. The
    // backend must clamp every runtime index against the per-slot length
    // stored at `[%rd0 + 4 + slot*4]`.
    let kernel = KernelDescriptor {
        id: "idx_load".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: None,
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadOnly,
                name: "input".into(),
            }],
        },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                // Use GlobalInvocationId as a non-literal index so the
                // immediate fast-path is bypassed and the clamp path runs.
                KernelOp {
                    kind: KernelOpKind::GlobalInvocationId,
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
            literals: vec![],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("ld.global.u32") && s.contains("[%rd0 + 4]"),
        "must load slot-0 length from params metadata at +4:\n{s}"
    );
    assert!(
        s.contains("setp.lt.u32") && s.contains("selp.u32"),
        "must clamp index via setp.lt + selp.u32:\n{s}"
    );
}

#[test]
fn buffer_length_registers_are_preloaded_before_branch_stores() {
    let kernel = KernelDescriptor {
        id: "preload_lengths_before_branch_store".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(16),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::WriteOnly,
                name: "out".into(),
            }],
        },
        dispatch: Dispatch::new(16, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::GlobalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StructuredIfThenElse,
                    operands: vec![1, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![
                KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    }],
                    child_bodies: vec![],
                    literals: vec![],
                },
                KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 3],
                        result: None,
                    }],
                    child_bodies: vec![],
                    literals: vec![],
                },
            ],
            literals: vec![
                LiteralValue::Bool(true),
                LiteralValue::U32(7),
                LiteralValue::U32(9),
            ],
        },
    };
    let s = emit(&kernel).unwrap();
    let length_load = s
        .find("[%rd0 + 4]")
        .expect("must load output slot length from params metadata");
    let first_store = s
        .find("st.global.u32")
        .expect("must emit predicated branch stores");
    assert!(
        length_load < first_store,
        "slot length load must dominate all branch stores:\n{s}"
    );
    assert_eq!(
        s.matches("[%rd0 + 4]").count(),
        1,
        "slot length must be preloaded once, not lazily reloaded per branch:\n{s}"
    );
}
