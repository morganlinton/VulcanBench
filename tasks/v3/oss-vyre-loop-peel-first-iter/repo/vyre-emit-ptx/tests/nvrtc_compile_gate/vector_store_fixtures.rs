use super::*;

pub(crate) fn ptx_for_vector_store_fusion() -> String {
    let desc = KernelDescriptor {
        id: "vector_store_fusion".into(),
        bindings: BindingLayout {
            slots: vec![slot_typed(
                0,
                "output",
                DataType::U32,
                BindingVisibility::WriteOnly,
            )],
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
                    kind: KernelOpKind::Literal,
                    operands: vec![4],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 1],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![5],
                    result: Some(5),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 5, 2],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![6],
                    result: Some(6),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 6, 3],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![7],
                    result: Some(7),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 7, 4],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(10),
                LiteralValue::U32(11),
                LiteralValue::U32(12),
                LiteralValue::U32(13),
                LiteralValue::U32(1),
                LiteralValue::U32(2),
                LiteralValue::U32(3),
            ],
        },
    };
    vyre_emit_ptx::emit_optimized(&desc).unwrap()
}

pub(crate) fn ptx_for_dynamic_vector_store_fusion() -> String {
    let desc = KernelDescriptor {
        id: "dynamic_vector_store_fusion".into(),
        bindings: BindingLayout {
            slots: vec![slot_typed(
                0,
                "output",
                DataType::U32,
                BindingVisibility::WriteOnly,
            )],
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
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![3],
                    result: Some(5),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![4],
                    result: Some(6),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 6],
                    result: Some(7),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![5],
                    result: Some(8),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 8],
                    result: Some(9),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![6],
                    result: Some(10),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 10],
                    result: Some(11),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![7],
                    result: Some(12),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 12],
                    result: Some(13),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 2, 7],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![2, 3],
                    result: Some(14),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 14, 9],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![2, 4],
                    result: Some(15),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 15, 11],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![2, 5],
                    result: Some(16),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 16, 13],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(4),
                LiteralValue::U32(1),
                LiteralValue::U32(2),
                LiteralValue::U32(3),
                LiteralValue::U32(1000),
                LiteralValue::U32(1001),
                LiteralValue::U32(1002),
                LiteralValue::U32(1003),
            ],
        },
    };
    vyre_emit_ptx::emit_optimized(&desc).unwrap()
}
