use super::*;

#[test]
fn emit_fuses_four_adjacent_u32_stores_to_ptx_vector_store() {
    let s = emit(&two_slot_u32_kernel(
        "vec_store",
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
                kind: KernelOpKind::Literal,
                operands: vec![5],
                result: Some(5),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 0, 2],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![0, 1],
                result: Some(6),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 6, 3],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![6, 1],
                result: Some(7),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 7, 4],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![7, 1],
                result: Some(8),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 8, 5],
                result: None,
            },
        ],
        vec![
            LiteralValue::U32(0),
            LiteralValue::U32(1),
            LiteralValue::U32(10),
            LiteralValue::U32(11),
            LiteralValue::U32(12),
            LiteralValue::U32(13),
        ],
    ))
    .unwrap();
    assert!(s.contains("st.global.v4.u32"));
    assert!(!s.contains("st.global.u32"));
    assert!(
        !s.contains("add.u32"),
        "fused vector store must not leave dead scalar index-increment adds:\n{s}"
    );
}

#[test]
fn generated_dynamic_reassociated_store_indices_fuse_to_v4() {
    for seed in 0..1024 {
        let s = emit(&dynamic_reassociated_vector_store_kernel(seed))
            .unwrap_or_else(|error| panic!("seed {seed} failed to emit: {error}"));
        assert!(
            s.contains("st.global.v4.u32"),
            "seed {seed} must recover v4 store fusion after affine reassociation:\n{s}"
        );
        assert_eq!(
            s.matches("st.global.u32").count(),
            0,
            "seed {seed} must not leave scalar stores after v4 store fusion:\n{s}"
        );
    }
}

#[test]
fn emit_fuses_vector_store_across_folded_literal_index_gaps() {
    let s = emit(&two_slot_u32_kernel(
        "folded_literal_vec_store",
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
                kind: KernelOpKind::Literal,
                operands: vec![5],
                result: Some(5),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 5, 2],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![6],
                result: Some(6),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 6, 3],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![7],
                result: Some(7),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 7, 4],
                result: None,
            },
        ],
        vec![
            LiteralValue::U32(0),
            LiteralValue::U32(10),
            LiteralValue::U32(11),
            LiteralValue::U32(12),
            LiteralValue::U32(13),
            LiteralValue::U32(1),
            LiteralValue::U32(2),
            LiteralValue::U32(3),
        ],
    ))
    .unwrap();

    assert!(
        s.contains("st.global.v4.u32"),
        "Fix: folded adjacent store indices must still fuse into a vector store.\n{s}"
    );
    assert!(
        !s.contains("st.global.u32"),
        "Fix: folded-index vector store fusion must not leave scalar stores behind.\n{s}"
    );
}


