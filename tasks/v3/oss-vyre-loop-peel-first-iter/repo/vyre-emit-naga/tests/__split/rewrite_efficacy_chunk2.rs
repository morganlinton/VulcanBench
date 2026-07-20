#[test]
fn run_all_is_semantics_preserving_for_emitter() {
    // For every corpus shape, both raw and optimized must successfully
    // emit a Naga module with the same `main` entry point. (We don't
    // compare module byte-for-byte because optimization changes the
    // generated code; we DO assert both emit cleanly.)
    let cases: Vec<KernelDescriptor> = vec![KernelDescriptor {
        id: "trivial".into(),
        bindings: BindingLayout {
            slots: vec![buf_slot()],
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
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
        },
    }];

    for case in &cases {
        let raw = vyre_emit_naga::emit(case).expect("raw emit");
        let opt = vyre_emit_naga::emit_optimized(case).expect("optimized emit");
        assert_eq!(raw.entry_points[0].name, opt.entry_points[0].name);
        assert_eq!(
            raw.entry_points[0].workgroup_size,
            opt.entry_points[0].workgroup_size
        );
    }
}

#[test]
fn unop_chain_collapses() {
    // r0 = Lit(7), r1 = BitNot(r0), r2 = BitNot(r1), Store(_, _, r2)
    use vyre_foundation::ir::UnOp;
    let desc = KernelDescriptor {
        id: "unop_chain".into(),
        bindings: BindingLayout {
            slots: vec![buf_slot()],
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
                    kind: KernelOpKind::UnOpKind(UnOp::BitNot),
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::UnOpKind(UnOp::BitNot),
                    operands: vec![1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7)],
        },
    };
    let before = count_ops(&desc);
    let after = count_ops(&run_all(&desc));
    assert!(
        after < before,
        "BitNot chain should shrink ({before} → {after})"
    );
}

#[test]
fn comparison_into_select_collapses_chain() {
    // r0 = Lit(3), r1 = Lit(5), r2 = Lit(7), r3 = Lit(99)
    // r4 = Lt(r0, r1)            → const_fold to Lit(true)
    // r5 = Select(r4, r2, r3)    → identity_elim picks r2
    let desc = KernelDescriptor {
        id: "cmp_select".into(),
        bindings: BindingLayout {
            slots: vec![buf_slot()],
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
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![3],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Lt),
                    operands: vec![0, 1],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::Select,
                    operands: vec![4, 2, 3],
                    result: Some(5),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 5],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(3),
                LiteralValue::U32(5),
                LiteralValue::U32(7),
                LiteralValue::U32(99),
            ],
        },
    };
    let before = count_ops(&desc);
    let after_desc = run_all(&desc);
    let after = count_ops(&after_desc);
    assert!(
        after < before,
        "comparison + Select chain should shrink ({before} → {after})"
    );
}

#[test]
fn cast_chain_folds() {
    // r0 = Lit(7), r1 = Cast(r0, I32), r2 = Cast(r1, U32)
    use vyre_foundation::ir::DataType as Dt;
    let desc = KernelDescriptor {
        id: "cast".into(),
        bindings: BindingLayout {
            slots: vec![buf_slot()],
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
                    kind: KernelOpKind::Cast { target: Dt::I32 },
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Cast { target: Dt::U32 },
                    operands: vec![1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7)],
        },
    };
    let before = count_ops(&desc);
    let after = count_ops(&run_all(&desc));
    assert!(
        after < before,
        "Cast chain should fold ({before} → {after})"
    );
}
