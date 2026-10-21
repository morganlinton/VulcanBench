// Rewrite-efficacy harness.
//
// Builds a corpus of synthetic `KernelDescriptor`s, each with a
// known efficiency profile (e.g. dead arithmetic, redundant loads,
// identity ops, common subexpressions), runs each through
// `vyre_lower::rewrites::run_all`, and asserts the optimized form is
// strictly smaller in op count.
//
// This guards the rewrite pipeline against regressions: if a future
// pass change accidentally stops eliminating one of these patterns,
// the corresponding case here turns red.
//
// Source: ROADMAP T072 (rewrite-efficacy gate).

use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::{
    rewrites::run_all, BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody,
    KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

fn buf_slot() -> BindingSlot {
    BindingSlot {
        slot: 0,
        element_type: DataType::U32,
        element_count: None,
        memory_class: MemoryClass::Global,
        visibility: BindingVisibility::ReadWrite,
        name: "buf".into(),
    }
}

fn count_ops(desc: &KernelDescriptor) -> usize {
    desc.body.ops.len()
}

#[test]
fn dead_arithmetic_kernel_shrinks() {
    // r0=Lit(0), r1=Lit(99), r2=Add(r1, r0)→r1, r3=Mul(r1, r0)→r0,
    // Store(_, _, r1).  Three pure ops should die after run_all.
    let desc = KernelDescriptor {
        id: "dead_arith".into(),
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
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![1, 0],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![1, 0],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(99)],
        },
    };
    let before = count_ops(&desc);
    let after = count_ops(&run_all(&desc));
    assert!(
        after < before,
        "dead arithmetic kernel should shrink (was {before}, now {after})"
    );
    assert!(
        after <= before - 2,
        "expected at least 2 ops eliminated (was {before}, now {after})"
    );
}

#[test]
fn redundant_load_kernel_shrinks() {
    // Store(buf, 0, 7); r2=Load(buf, 0); Store(buf, 0, r2).
    // After run_all: load_forwarding rewrites r2→r1, dce drops the
    // Load, dead_store collapses the two Stores → 1 store + 2 lits.
    let desc = KernelDescriptor {
        id: "redundant_load".into(),
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
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 2],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
        },
    };
    let before = count_ops(&desc);
    let after = count_ops(&run_all(&desc));
    assert!(
        after <= before - 2,
        "expected ≥2 ops eliminated (was {before}, now {after})"
    );
}

#[test]
fn duplicate_literals_kernel_shrinks() {
    // r0=Lit(7), r1=Lit(7), r2=Lit(7), Store(_, _, r2).
    // CSE should merge r1 and r2 into r0 → 2 ops eliminated.
    let desc = KernelDescriptor {
        id: "dup_lits".into(),
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
                }, // idx
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // val (dup)
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(2),
                }, // val (dup)
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(3),
                }, // val (dup)
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 3],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
        },
    };
    let before = count_ops(&desc);
    let after = count_ops(&run_all(&desc));
    assert!(
        after <= before - 2,
        "CSE should collapse 3 dup literals into 1 (was {before}, now {after})"
    );
}

#[test]
fn const_foldable_arithmetic_kernel_shrinks() {
    // r0=Lit(3), r1=Lit(4), r2=Add(r0, r1)=7. const_fold replaces
    // r2 with a Lit(7); CSE may dedupe (no other Lit(7) here, so it stays).
    // Net: same op count, but r2 is now Lit not Add. Op count won't drop
    // unless something downstream removes the lits  -  they're used.
    //
    // To force a real shrink: have an unused Add too. Then the unused
    // Add becomes an unused Lit and DCE drops it.
    let desc = KernelDescriptor {
        id: "const_fold".into(),
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
                }, // 3
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // 4
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                }, // 0 (idx)
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(3),
                }, // unused sum
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![0, 1],
                    result: Some(4),
                }, // unused product
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 2, 0],
                    result: None,
                }, // store r0 (3) at idx r2 (0)
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(3),
                LiteralValue::U32(4),
                LiteralValue::U32(0),
            ],
        },
    };
    let before = count_ops(&desc);
    let after = count_ops(&run_all(&desc));
    assert!(
        after < before,
        "unused arithmetic should be eliminated (was {before}, now {after})"
    );
}

#[test]
fn empty_kernel_stays_empty() {
    let desc = KernelDescriptor {
        id: "empty".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![],
        },
    };
    let after = run_all(&desc);
    assert!(after.body.ops.is_empty());
}

#[test]
fn already_optimal_kernel_doesnt_grow() {
    // A minimal store kernel with no redundancy. run_all must not add ops.
    let desc = KernelDescriptor {
        id: "minimal".into(),
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
    };
    let before = count_ops(&desc);
    let after = count_ops(&run_all(&desc));
    assert_eq!(
        after, before,
        "minimal kernel must not grow (was {before}, now {after})"
    );
}

#[test]
fn aggregate_corpus_shrinks_significantly() {
    // Combine all the corpus shapes; assert aggregate reduction is
    // ≥20%. A meaningful headline number that catches "one pass became
    // a no-op" regressions.
    let cases: Vec<KernelDescriptor> = vec![
        // dead arithmetic
        KernelDescriptor {
            id: "c1".into(),
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
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![1, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Mul),
                        operands: vec![1, 0],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(99)],
            },
        },
        // redundant load
        KernelDescriptor {
            id: "c2".into(),
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
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        },
        // duplicates
        KernelDescriptor {
            id: "c3".into(),
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
                        operands: vec![1],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 3],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        },
    ];

    let total_before: usize = cases.iter().map(count_ops).sum();
    let total_after: usize = cases.iter().map(|c| count_ops(&run_all(c))).sum();
    let saved = total_before.saturating_sub(total_after);
    let pct = (saved as f64) / (total_before as f64) * 100.0;
    assert!(
        pct >= 20.0,
        "expected ≥20% op reduction across corpus, got {pct:.1}% ({total_before} → {total_after})"
    );
    println!(
        "rewrite_efficacy: corpus reduced from {total_before} to {total_after} ops ({pct:.1}% saved)"
    );
}

