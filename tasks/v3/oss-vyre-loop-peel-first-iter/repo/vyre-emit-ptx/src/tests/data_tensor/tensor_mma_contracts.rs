use super::*;

#[test]
fn matrix_mma_emits_real_mma_sync_and_binds_all_four_results() {
    let mut ops = Vec::new();
    let mut literals = Vec::new();
    for id in 0..6 {
        literals.push(LiteralValue::U32(id));
        ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![id],
            result: Some(id),
        });
    }
    for id in 6..10 {
        literals.push(LiteralValue::F32(0.0));
        ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![id],
            result: Some(id),
        });
    }
    ops.push(KernelOp {
        kind: KernelOpKind::MatrixMma {
            shape: MatrixMmaShape::M16N8K16,
            a_layout: MatrixMmaLayout::RowMajor,
            b_layout: MatrixMmaLayout::ColMajor,
            a_type: MatrixMmaElement::F16,
            b_type: MatrixMmaElement::F16,
            accum_type: MatrixMmaElement::F32,
        },
        operands: (0..10).collect(),
        result: Some(10),
    });
    ops.push(KernelOp {
        kind: KernelOpKind::Literal,
        operands: vec![10],
        result: Some(14),
    });
    literals.push(LiteralValue::U32(0));
    ops.push(KernelOp {
        kind: KernelOpKind::StoreGlobal,
        operands: vec![0, 14, 13],
        result: None,
    });

    let kernel = KernelDescriptor {
        id: "mma".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::F32,
                element_count: Some(1),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::WriteOnly,
                name: "out".into(),
            }],
        },
        dispatch: Dispatch::new(32, 1, 1),
        body: KernelBody {
            ops,
            child_bodies: vec![],
            literals,
        },
    };

    vyre_lower::verify::verify(&kernel)
        .expect("MatrixMma must publish result ids base..base+4 to verifier");
    let s = emit_with_target(&kernel, ComputeCapability::SM_70).unwrap();
    assert!(s.contains("mma.sync.aligned.m16n8k16.row.col.f32.f16.f16.f32"));
    assert!(
        s.contains("st.global.f32"),
        "fourth MatrixMma result id must be usable by later ops"
    );
}
