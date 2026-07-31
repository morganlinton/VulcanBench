//! Test: preamble.
use super::*;

#[test]
fn emit_produces_preamble_with_target() {
    let s = emit(&one_store_kernel()).unwrap();
    assert!(s.contains(".version 8.5"));
    assert!(s.contains(".target sm_70"));
    assert!(s.contains(".address_size 64"));
}

#[test]
fn emit_has_visible_entry_main() {
    let s = emit(&one_store_kernel()).unwrap();
    assert!(s.contains(".visible .entry main("));
}

#[test]
fn emit_with_target_uses_requested_capability() {
    let s = emit_with_target(&one_store_kernel(), ComputeCapability::SM_90).unwrap();
    assert!(s.contains(".target sm_90"));
}

#[test]
fn emit_writes_param_for_each_binding() {
    let s = emit(&one_store_kernel()).unwrap();
    // Param naming contract: `_arg_<sanitized_binding_name>` (binding "out").
    assert!(s.contains(".param .u64 _arg_out"));
    assert!(s.contains(".param .u64 params_buf"));
}

#[test]
fn literal_index_store_uses_immediate_byte_offset() {
    let kernel = KernelDescriptor {
        id: "literal_store_offset".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(8),
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
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 1],
                    result: None,
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(3), LiteralValue::U32(7)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("+12]"),
        "u32 index 3 should fold to a 12-byte immediate address offset:\n{s}"
    );
    assert!(
        !s.contains("mul.wide.u32"),
        "literal-index global store should not emit address multiply:\n{s}"
    );
}

#[test]
fn predicate_and_uses_and_pred_not_and_b32() {
    // Adversarial pin (DFA regression): logical `And` on two predicate
    // operands must emit `and.pred`, not `and.b32`  -  the latter trips
    // `cuModuleLoadData → CUDA_ERROR_INVALID_PTX` at runtime because PTX
    // requires the type suffix to match the operand class. This test
    // builds a kernel that boolean-ANDs two comparisons and checks the
    // emitted text.
    let kernel = KernelDescriptor {
        id: "pred_and".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: Some(1),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
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
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(2),
                },
                // p1 = (tid < 3)
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Lt),
                    operands: vec![2, 0],
                    result: Some(3),
                },
                // p2 = (tid < 5)
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Lt),
                    operands: vec![2, 1],
                    result: Some(4),
                },
                // p3 = p1 && p2  ← must be `and.pred`
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::And),
                    operands: vec![3, 4],
                    result: Some(5),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(3), LiteralValue::U32(5)],
        },
    };
    let s = emit(&kernel).unwrap();
    assert!(
        s.contains("and.pred"),
        "predicate AND must emit `and.pred`; got:\n{s}"
    );
    assert!(
        !s.contains("and.b32    %p"),
        "predicate AND must not emit `and.b32 %p…`; got:\n{s}"
    );
}

#[test]
fn emit_loads_param_and_converts_to_global() {
    let s = emit(&one_store_kernel()).unwrap();
    assert!(s.contains("ld.param.u64"));
    assert!(s.contains("cvta.to.global.u64"));
}

#[test]
fn emit_emits_literal_mov_then_store() {
    let s = emit(&one_store_kernel()).unwrap();
    assert!(s.contains("mov.u32"));
    assert!(s.contains("st.global.u32"));
}

/// A binding slot large enough that `4 + slot * 4` overflows u32 must produce
/// `EmitError::InvalidBinding`, not silently wrap to a plausible address and
/// emit wrong PTX that reads the length register from a garbage offset.
///
/// Before this fix, `4u32 + binding.slot * 4` in `preload_bindings` (and the
/// equivalent expression in `BufferLength`) used bare wrapping arithmetic, so
/// slot >= 1_073_741_823 wrapped to a small-but-wrong offset and the GPU's
/// bounds-check predicate was silently wrong.
#[test]
fn slot_offset_overflow_returns_invalid_binding_error() {
    let overflow_slot: u32 = 1_073_741_824; // slot * 4 = 0x1_0000_0000, overflows u32
    let desc = KernelDescriptor {
        id: "overflow_slot".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: overflow_slot,
                element_type: DataType::U32,
                element_count: None,
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadOnly,
                name: "huge".into(),
            }],
        },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![],
        },
    };
    let result = emit(&desc);
    match result {
        Err(crate::EmitError::InvalidBinding { slot, reason }) => {
            assert_eq!(
                slot, overflow_slot,
                "Fix: error must name the overflowing slot exactly"
            );
            assert!(
                reason.contains("overflow"),
                "Fix: error reason must mention 'overflow'; got: {reason}"
            );
        }
        Ok(ptx) => panic!(
            "Fix: emit must return Err(InvalidBinding) for overflowing slot, \
             not Ok with wrapped address in PTX:\n{ptx}"
        ),
        Err(other) => panic!(
            "Fix: expected EmitError::InvalidBinding for overflowing slot, got: {other:?}"
        ),
    }
}

#[test]
fn scalar_kernel_keeps_entry_element_count_guard() {
    let s = emit(&one_store_kernel()).unwrap();
    assert!(
        s.contains("ld.global.ca.u32   %r26, [%rd0];")
            && s.contains("setp.ge.u32     %p0, %r3, %r26;")
            && s.contains("@%p0 bra $L_exit;"),
        "scalar kernels without barriers/shared memory should keep the entry element-count guard:\n{s}"
    );
}
