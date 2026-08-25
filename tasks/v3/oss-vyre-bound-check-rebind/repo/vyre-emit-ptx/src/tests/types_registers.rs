//! Test: types registers.
use super::*;

#[test]
fn capability_constants_present() {
    assert_eq!(ComputeCapability::SM_70.major, 7);
    assert_eq!(ComputeCapability::SM_90.major, 9);
    assert!(ComputeCapability::SM_80.supports_async_copy());
    assert!(!ComputeCapability::SM_70.supports_async_copy());
    assert!(ComputeCapability::SM_75.supports_ldmatrix());
    assert!(!ComputeCapability::SM_70.supports_ldmatrix());
}

#[test]
fn ptx_type_from_dtype_covers_scalars() {
    assert_eq!(PtxType::from_dtype(&DataType::Bool).unwrap(), PtxType::Bool);
    assert_eq!(PtxType::from_dtype(&DataType::U8).unwrap(), PtxType::U32);
    assert_eq!(PtxType::from_dtype(&DataType::I8).unwrap(), PtxType::I32);
    assert_eq!(PtxType::from_dtype(&DataType::U16).unwrap(), PtxType::U32);
    assert_eq!(PtxType::from_dtype(&DataType::I16).unwrap(), PtxType::I32);
    assert_eq!(PtxType::from_dtype(&DataType::U32).unwrap(), PtxType::U32);
    assert_eq!(PtxType::from_dtype(&DataType::I32).unwrap(), PtxType::I32);
    assert_eq!(PtxType::from_dtype(&DataType::F32).unwrap(), PtxType::F32);
}

#[test]
fn ptx_type_from_dtype_rejects_unsupported() {
    assert!(matches!(
        PtxType::from_dtype(&DataType::Tensor),
        Err(EmitError::UnsupportedDataType(_))
    ));
}

#[test]
fn ptx_type_from_dtype_rejects_bytes_instead_of_silent_u32() {
    // `Bytes` is a packed-byte buffer element, not a scalar register type.
    // Before this guard it was folded into `.u32` (grouped with U8/U16/U32),
    // silently reinterpreting a byte stream as a word: a `Bytes` buffer load
    // would index words not bytes, and a `Cast { Bytes }` of a u32 would no-op
    // (src == dst). The emitter must fail closed and name the pack-to-u32 fix.
    let err = PtxType::from_dtype(&DataType::Bytes)
        .expect_err("from_dtype(Bytes) must fail closed, not silently map to .u32");
    let EmitError::UnsupportedDataType(msg) = &err else {
        panic!("Bytes rejection must be UnsupportedDataType; got {err:?}");
    };
    assert!(
        msg.contains("Bytes") && msg.contains("pack-to-u32 pre-pass"),
        "Bytes rejection must name the type and the fix (pack-to-u32 pre-pass); got: {msg}"
    );
}

#[test]
fn reg_display_uses_correct_prefix() {
    assert_eq!(format!("{}", Reg(PtxType::U32, 5)), "%r5");
    assert_eq!(format!("{}", Reg(PtxType::I32, 0)), "%s0");
    assert_eq!(format!("{}", Reg(PtxType::F32, 3)), "%f3");
    assert_eq!(format!("{}", Reg(PtxType::Bool, 1)), "%p1");
    assert_eq!(format!("{}", Reg(PtxType::U64, 7)), "%rd7");
}

#[test]
fn register_declaration_sized_to_used_count() {
    // A kernel with 3 u32 ops declares those registers on top of
    // the reserved launch-ABI registers.
    let kernel = KernelDescriptor {
        id: "regs".into(),
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
            literals: vec![LiteralValue::U32(1), LiteralValue::U32(2)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(s.contains(".reg .u32   %r<30>;"));
}

fn narrow_global_copy_kernel(element_type: DataType) -> KernelDescriptor {
    KernelDescriptor {
        id: "narrow_copy".into(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: element_type.clone(),
                    element_count: Some(8),
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "input".into(),
                },
                BindingSlot {
                    slot: 1,
                    element_type,
                    element_count: Some(8),
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::WriteOnly,
                    name: "output".into(),
                },
            ],
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
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![1, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0)],
        },
    }
}

#[test]
fn narrow_integer_global_memory_uses_narrow_ptx_ops() {
    for (data_type, load_op, store_op) in [
        (DataType::U8, "ld.global.u8", "st.global.u8"),
        (DataType::I8, "ld.global.s8", "st.global.u8"),
        (DataType::U16, "ld.global.u16", "st.global.u16"),
        (DataType::I16, "ld.global.s16", "st.global.u16"),
    ] {
        let ptx = emit(&narrow_global_copy_kernel(data_type.clone())).unwrap();
        assert!(
            ptx.contains(load_op),
            "Fix: {data_type:?} loads must use byte/halfword PTX instead of widening the memory transaction:\n{ptx}"
        );
        assert!(
            ptx.contains(store_op),
            "Fix: {data_type:?} stores must use byte/halfword PTX instead of widening the memory transaction:\n{ptx}"
        );
    }
}
