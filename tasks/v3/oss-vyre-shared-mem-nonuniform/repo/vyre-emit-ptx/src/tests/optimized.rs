//! Test: optimized.
use super::*;

#[test]
fn emit_optimized_succeeds_on_one_store_kernel() {
    let s = emit_optimized(&one_store_kernel()).unwrap();
    assert!(s.contains(".visible .entry"));
    assert!(s.contains(".version"));
}

#[test]
fn emit_optimized_with_target_threads_capability_through() {
    let s = emit_optimized_with_target(&one_store_kernel(), ComputeCapability::SM_90).unwrap();
    assert!(s.contains(".target sm_90"));
}

#[test]
fn emit_optimized_with_target_with_stats_returns_both() {
    let (ptx, stats) =
        emit_optimized_with_target_with_stats(&one_store_kernel(), ComputeCapability::SM_80)
            .unwrap();
    assert!(ptx.contains(".target sm_80"));
    // The stats came from the run_all pass, so they should be populated.
    assert!(stats.iterations >= 1);
    assert!(stats.converged);
}

#[test]
fn emit_optimized_drops_dead_arithmetic() {
    // Same shape as the naga test: identity ops + absorbing zero
    // → dead after run_all. Emitted PTX should be no longer than
    // the un-optimized form.
    use vyre_foundation::ir::BinOp as Bo;
    use vyre_lower::{KernelOpKind, LiteralValue};
    let desc = KernelDescriptor {
        id: "k".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
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
                    kind: KernelOpKind::BinOpKind(Bo::Add),
                    operands: vec![1, 0],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(Bo::Mul),
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
    let raw = emit(&desc).unwrap();
    let optimized = emit_optimized(&desc).unwrap();
    // Optimized PTX has at most as many lines as raw (no extra ops
    // added; only droppable ones removed).
    assert!(
        optimized.lines().count() <= raw.lines().count(),
        "optimized PTX ({} lines) should not exceed raw ({} lines)",
        optimized.lines().count(),
        raw.lines().count()
    );
}
