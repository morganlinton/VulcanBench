//! Test: egraph pipeline integration.
use vyre_foundation::ir::BinOp;
use vyre_foundation::ir::DataType;
use vyre_lower::{
    rewrites, BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody,
    KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

#[test]
fn canonical_run_all_applies_egraph_constant_chain_reassociation() {
    let desc = KernelDescriptor {
        id: "canonical_egraph_chain".to_string(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(64),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::WriteOnly,
                name: "out".to_string(),
            }],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
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
                    kind: KernelOpKind::BinOpKind(BinOp::BitOr),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::BitOr),
                    operands: vec![3, 2],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 4],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0b0010), LiteralValue::U32(0b0100)],
        },
    };

    let one_pass = rewrites::run_all_once(&desc);
    assert!(
        one_pass
            .body
            .ops
            .iter()
            .all(|op| op.result != Some(3) && !op.operands.contains(&3)),
        "Fix: canonical run_all_once must immediately clean up the intermediate bitwise-chain result after e-graph saturation: {one_pass:?}"
    );
    assert!(
        one_pass
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::BinOpKind(BinOp::BitOr)))
            .count()
            <= 1,
        "Fix: canonical run_all_once must collapse bitwise constant chains without waiting for a later fixed-point iteration: {one_pass:?}"
    );

    let optimized = rewrites::run_all(&desc);
    assert!(
        optimized
            .body
            .ops
            .iter()
            .all(|op| op.result != Some(3) && !op.operands.contains(&3)),
        "Fix: canonical run_all must eliminate the intermediate bitwise-chain result after e-graph saturation and cleanup: {optimized:?}"
    );
    assert!(
        optimized
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::BinOpKind(BinOp::BitOr)))
            .count()
            <= 1,
        "Fix: canonical run_all must use e-graph reassociation plus cleanup to collapse bitwise constant chains: {optimized:?}"
    );
}
