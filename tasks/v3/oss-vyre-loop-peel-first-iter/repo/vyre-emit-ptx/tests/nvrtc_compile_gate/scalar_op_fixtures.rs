use super::*;

pub(crate) fn ptx_for_op(op_kind: KernelOpKind) -> String {
    let result_id = 3u32;
    let idx_id = 2u32;

    let (mut ops, literals, binding) = match op_kind {
        KernelOpKind::Fma => (
            vec![
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Cast {
                        target: DataType::F32,
                    },
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(idx_id),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(5),
                },
                KernelOp {
                    kind: KernelOpKind::Fma,
                    operands: vec![1, 4, 5],
                    result: Some(result_id),
                },
            ],
            vec![
                LiteralValue::F32(2.0),
                LiteralValue::U32(0),
                LiteralValue::F32(3.0),
            ],
            rw_slot_typed(0, "out", DataType::F32),
        ),
        KernelOpKind::BinOpKind(BinOp::Mul) => (
            vec![
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(idx_id),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![0, 1],
                    result: Some(result_id),
                },
            ],
            vec![LiteralValue::U32(0)],
            rw_slot(0, "out"),
        ),
        other => (
            vec![
                // Use LocalInvocationId so the op survives constant folding.
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
                    result: Some(idx_id),
                },
                KernelOp {
                    kind: other,
                    operands: vec![0, 1],
                    result: Some(result_id),
                },
            ],
            vec![LiteralValue::U32(7), LiteralValue::U32(0)],
            rw_slot(0, "out"),
        ),
    };
    ops.push(KernelOp {
        kind: KernelOpKind::StoreGlobal,
        operands: vec![0, idx_id, result_id],
        result: None,
    });

    let desc = KernelDescriptor {
        id: "test".into(),
        bindings: BindingLayout {
            slots: vec![binding],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops,
            child_bodies: vec![],
            literals,
        },
    };
    vyre_emit_ptx::emit_optimized(&desc).unwrap()
}
