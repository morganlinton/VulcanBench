//! Snapshot test pinning the optimization output of the kitchen-sink
//! kernel from `examples/optimize.rs`.
//!
//! If a pass change accidentally breaks the pipeline (a pass stops
//! firing, fires differently, or composes differently with its
//! neighbors), this test fails with a precise diff. Update only when
//! the change is intentional and you've verified the new output is
//! semantically equivalent.

use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::{
    rewrites::run_all_with_stats, BindingLayout, BindingSlot, BindingVisibility, Dispatch,
    KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

fn kitchen_sink_descriptor() -> KernelDescriptor {
    let bindings = vec![
        BindingSlot {
            slot: 0,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: "output".into(),
        },
        BindingSlot {
            slot: 1,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: "scratch_a".into(),
        },
        BindingSlot {
            slot: 2,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: "scratch_b".into(),
        },
    ];

    let ops = vec![
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
            kind: KernelOpKind::BinOpKind(BinOp::Add),
            operands: vec![2, 0],
            result: Some(4),
        },
        KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Mul),
            operands: vec![2, 1],
            result: Some(5),
        },
        KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Mul),
            operands: vec![2, 0],
            result: Some(6),
        },
        KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Mul),
            operands: vec![2, 3],
            result: Some(7),
        },
        KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![0, 0, 4],
            result: None,
        },
        KernelOp {
            kind: KernelOpKind::LoadGlobal,
            operands: vec![0, 0],
            result: Some(8),
        },
        KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![0, 0, 7],
            result: None,
        },
    ];

    KernelDescriptor {
        id: "kitchen_sink".into(),
        bindings: BindingLayout { slots: bindings },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops,
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(7),
                LiteralValue::U32(8),
            ],
        },
    }
}

#[test]
fn kitchen_sink_snapshot_stats() {
    let desc = kitchen_sink_descriptor();
    let (_optimized, stats) = run_all_with_stats(&desc);

    // Pinned values  -  change ONLY when the optimization changes intentionally.
    assert_eq!(stats.ops_before, 11, "input op count drifted");
    assert_eq!(stats.ops_after, 3, "optimized op count drifted (was 3)");
    assert_eq!(stats.bindings_before, 3, "input binding count drifted");
    // drop_unused_bindings retains every Global/Constant binding
    // unconditionally (commit 9afc2f1ff9 / 435a5b96e2): the dispatch ABI is
    // slot-addressed by the host, so silently dropping unreferenced
    // host-visible bindings shifts every later input slot's index.
    // Only Shared/Scratch bindings are eligible for dropping. The
    // kitchen-sink fixture's three bindings are all Global → all retained.
    assert_eq!(
        stats.bindings_after, 3,
        "Global bindings are now retained for dispatch ABI soundness"
    );
    assert_eq!(stats.literals_before, 4, "input literal pool drifted");
    assert_eq!(
        stats.literals_after, 2,
        "optimized literal pool drifted (was 2)"
    );
    assert_eq!(stats.iterations, 2, "fixed-point iteration count drifted");
    assert!(stats.converged, "pipeline must converge");
}

#[test]
fn kitchen_sink_snapshot_final_op_shape() {
    let desc = kitchen_sink_descriptor();
    let (optimized, _stats) = run_all_with_stats(&desc);

    // Exactly: r0 = Lit(pool 0); r7 = Lit(pool 1); Store(slot 0, idx r0, val r7).
    // Rewrites preserve sparse ids because structured child bodies can
    // legally reference parent/child result ids across body boundaries.
    assert_eq!(optimized.body.ops.len(), 3);

    assert_eq!(optimized.body.ops[0].kind, KernelOpKind::Literal);
    assert_eq!(optimized.body.ops[0].result, Some(0));
    assert_eq!(optimized.body.ops[0].operands, vec![0]);

    assert_eq!(optimized.body.ops[1].kind, KernelOpKind::Literal);
    assert_eq!(optimized.body.ops[1].result, Some(7));
    assert_eq!(optimized.body.ops[1].operands, vec![1]);

    assert_eq!(optimized.body.ops[2].kind, KernelOpKind::StoreGlobal);
    assert!(optimized.body.ops[2].result.is_none());
    assert_eq!(optimized.body.ops[2].operands, vec![0, 0, 7]);
}

#[test]
fn kitchen_sink_snapshot_literal_pool() {
    let desc = kitchen_sink_descriptor();
    let (optimized, _stats) = run_all_with_stats(&desc);

    // Pool[0] = U32(0), Pool[1] = U32(56)  -  both required by surviving ops.
    // 56 = 7 << 3 (strength_reduce(Mul(7, 8)) → Shl(7, 3) → const_fold).
    assert_eq!(optimized.body.literals.len(), 2);
    assert_eq!(optimized.body.literals[0], LiteralValue::U32(0));
    assert_eq!(optimized.body.literals[1], LiteralValue::U32(56));
}

#[test]
fn kitchen_sink_snapshot_surviving_binding() {
    let desc = kitchen_sink_descriptor();
    let (optimized, _stats) = run_all_with_stats(&desc);

    // All three Global bindings survive: drop_unused_bindings retains
    // host-visible (Global/Constant) bindings unconditionally to keep the
    // dispatch ABI's slot indexing stable. Only Shared/Scratch bindings
    // are eligible for dropping. See `drop_unused_bindings` doc + commit
    // 9afc2f1ff9 / 435a5b96e2.
    assert_eq!(optimized.bindings.slots.len(), 3);
    let slot_names: Vec<&str> = optimized
        .bindings
        .slots
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(slot_names, vec!["output", "scratch_a", "scratch_b"]);
}
