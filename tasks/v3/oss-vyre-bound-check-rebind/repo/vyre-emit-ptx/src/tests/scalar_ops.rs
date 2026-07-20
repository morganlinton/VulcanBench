//! Test: scalar ops.
use super::*;

#[test]
fn emit_ends_with_return() {
    let s = emit(&one_store_kernel()).unwrap();
    // Last meaningful line is `ret;` followed by closing brace.
    assert!(s.contains("    ret;\n}"));
}

#[test]
fn empty_kernel_emits_just_preamble_and_ret() {
    let desc = KernelDescriptor {
        id: "empty".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![],
        },
    };
    let s = emit(&desc).unwrap();
    assert!(s.contains(".visible .entry main(\n    .param .u64 params_buf\n)"));
    assert!(s.contains("ret;"));
}

#[test]
fn binop_add_emits_add_u32() {
    let kernel = KernelDescriptor {
        id: "add".into(),
        bindings: BindingLayout { slots: vec![] },
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
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(3), LiteralValue::U32(4)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("add.u32"));
}

#[test]
fn integer_single_use_mul_add_emits_mad_without_dead_mul() {
    let kernel = KernelDescriptor {
        id: "int_mad".into(),
        bindings: BindingLayout { slots: vec![] },
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
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![3, 2],
                    result: Some(4),
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::I32(-3),
                LiteralValue::I32(7),
                LiteralValue::I32(5),
            ],
        },
    };

    let s = emit(&kernel).unwrap();

    assert!(s.contains("mad.lo.s32"), "{s}");
    assert!(!s.contains("mul.lo.s32"), "{s}");
    assert!(!s.contains("add.s32"), "{s}");
}

#[test]
fn integer_multi_use_mul_add_keeps_separate_mul() {
    let kernel = KernelDescriptor {
        id: "int_mad_multi_use".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::I32,
                element_count: Some(1),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::WriteOnly,
                name: "out".into(),
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
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![3, 2],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 3],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::I32(-3),
                LiteralValue::I32(7),
                LiteralValue::I32(5),
            ],
        },
    };

    let s = emit(&kernel).unwrap();

    assert!(s.contains("mul.lo.s32"), "{s}");
    assert!(!s.contains("mad.lo.s32"), "{s}");
}

#[test]
fn binop_lt_emits_setp_lt_to_pred_register() {
    let kernel = KernelDescriptor {
        id: "lt".into(),
        bindings: BindingLayout { slots: vec![] },
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
                    kind: KernelOpKind::BinOpKind(BinOp::Lt),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(3), LiteralValue::U32(4)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("setp.lt.u32"));
    assert!(s.contains(".reg .pred"));
}

#[test]
fn integer_shift_masks_rhs_to_reference_width() {
    let kernel = KernelDescriptor {
        id: "masked_shift".into(),
        bindings: BindingLayout { slots: vec![] },
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
                    kind: KernelOpKind::BinOpKind(BinOp::Shl),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Shr),
                    operands: vec![0, 1],
                    result: Some(3),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(1), LiteralValue::U32(33)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("and.b32"),
        "Fix: PTX shift lowering must mask the RHS to five bits before shl/shr."
    );
    assert!(
        s.contains(", 31;"),
        "Fix: PTX shift lowering must match the reference `rhs & 31` contract."
    );
    assert!(s.contains("shl.b32"));
    assert!(s.contains("shr.u32"));
}

#[test]
fn u32_power_of_two_const_mod_emits_mask_without_rem() {
    let kernel = KernelDescriptor {
        id: "mod_pow2".into(),
        bindings: BindingLayout { slots: vec![] },
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
                    kind: KernelOpKind::BinOpKind(BinOp::Mod),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(37), LiteralValue::U32(8)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("and.b32"),
        "Fix: u32 `% power_of_two` must lower to an integer mask.\n{s}"
    );
    assert!(
        !s.contains("rem.u32"),
        "Fix: u32 `% power_of_two` must not emit slow total modulo control flow.\n{s}"
    );
}

#[test]
fn unop_negate_emits_neg() {
    let kernel = KernelDescriptor {
        id: "neg".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::UnOpKind(UnOp::Negate),
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::I32(-5)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("neg.s32"));
}

#[test]
fn unop_reciprocal_emits_strict_or_approx_rcp() {
    let kernel = KernelDescriptor {
        id: "reciprocal".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::UnOpKind(UnOp::Reciprocal),
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::F32(4.0)],
        },
    };
    let strict = emit(&kernel).unwrap();
    assert!(strict.contains("rcp.rn.f32"));
    let approx = emit_with_options(
        &kernel,
        PtxEmitOptions {
            target: ComputeCapability::SM_70,
            subgroup_size: 32,
            ulp_budget: Some(4),
            cooperative_grid_sync: false,
        },
    )
    .unwrap();
    assert!(approx.contains("rcp.approx.f32"));
}

#[test]
fn local_invocation_id_emits_tid_x() {
    let kernel = KernelDescriptor {
        id: "tid".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::LocalInvocationId,
                operands: vec![0],
                result: Some(0),
            }],
            child_bodies: vec![],
            literals: vec![],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("%tid.x"));
}

#[test]
fn workgroup_id_emits_ctaid() {
    let kernel = KernelDescriptor {
        id: "wid".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::WorkgroupId,
                operands: vec![1], // y axis
                result: Some(0),
            }],
            child_bodies: vec![],
            literals: vec![],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains("%ctaid.y"));
}

#[test]
fn trap_emits_lane_exit() {
    // Trap is genuinely unsupported in PTX phase 1.
    let kernel = KernelDescriptor {
        id: "k".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Trap { tag: "t".into() },
                    operands: vec![0],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0)],
        },
    };
    let r = emit(&kernel);
    let s = r.unwrap();
    assert!(s.contains("// trap tag: t"));
    assert!(s.contains("bra $L_exit;"));
}

#[test]
fn add_of_two_single_use_muls_keeps_one_mul_available_for_mad() {
    let kernel = two_slot_u32_kernel(
        "mul_mul_add",
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
                kind: KernelOpKind::Literal,
                operands: vec![4],
                result: Some(4),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Mul),
                operands: vec![1, 2],
                result: Some(5),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Mul),
                operands: vec![3, 4],
                result: Some(6),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![5, 6],
                result: Some(7),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 0, 7],
                result: None,
            },
        ],
        vec![
            LiteralValue::U32(0),
            LiteralValue::U32(2),
            LiteralValue::U32(3),
            LiteralValue::U32(5),
            LiteralValue::U32(7),
        ],
    );

    let s =
        emit(&kernel).expect("Fix: MAD fusion must not defer both Mul operands feeding one Add.");

    assert!(
        s.contains("mad.lo.u32") && s.contains("st.global.u32"),
        "one product must remain materialized as the MAD addend while the other Mul fuses:\n{s}"
    );
}
