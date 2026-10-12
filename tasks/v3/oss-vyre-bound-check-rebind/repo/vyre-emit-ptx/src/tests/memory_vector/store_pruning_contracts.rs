use super::*;

#[test]
fn emit_does_not_fuse_vector_store_across_value_producer_gap() {
    let s = emit(&two_slot_u32_kernel(
        "value_gap_vec_store",
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
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 0, 1],
                result: None,
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
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 2, 3],
                result: None,
            },
        ],
        vec![
            LiteralValue::U32(0),
            LiteralValue::U32(10),
            LiteralValue::U32(1),
            LiteralValue::U32(11),
        ],
    ))
    .unwrap();

    assert!(
        !s.contains("st.global.v2.u32") && !s.contains("st.global.v4.u32"),
        "Fix: vector store fusion must not cross a value producer that has not emitted yet.\n{s}"
    );
    assert!(
        s.matches("st.global.u32").count() >= 2,
        "expected scalar stores when the value producer is in the fusion gap\n{s}"
    );
}

#[test]
fn vector_store_pruning_keeps_parent_index_used_by_child_body() {
    let kernel = two_slot_u32_kernel(
        "vec_store_parent_index_child_use",
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
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 0, 1],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 2, 1],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 3, 1],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 4, 1],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::StructuredBlock,
                operands: vec![0],
                result: None,
            },
        ],
        vec![
            LiteralValue::U32(0),
            LiteralValue::U32(99),
            LiteralValue::U32(1),
            LiteralValue::U32(2),
            LiteralValue::U32(3),
        ],
    );
    let mut kernel = kernel;
    kernel.body.child_bodies = vec![KernelBody {
        ops: vec![KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![1, 2, 1],
            result: None,
        }],
        child_bodies: vec![],
        literals: vec![],
    }];

    let s = emit(&kernel).expect(
        "Fix: vector-store index producer pruning must keep parent results read by child bodies.",
    );

    assert!(
        s.contains("st.global.v4.u32"),
        "setup must exercise vector-store fusion before the child body:\n{s}"
    );
    assert!(
        s.matches("mov.u32").count() >= 3,
        "child-read parent literals must remain emitted even when vector-store fusion consumes adjacent stores:\n{s}"
    );
}

