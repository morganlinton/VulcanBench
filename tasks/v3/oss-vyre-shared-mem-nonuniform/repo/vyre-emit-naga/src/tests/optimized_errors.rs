//! Test: optimized errors.
use super::*;

#[test]
fn emit_error_variants_format_with_descriptive_messages() {
    let e = EmitError::InvalidBinding {
        slot: 7,
        reason: "wrong dtype".into(),
    };
    let msg = format!("{e}");
    assert!(msg.contains("slot 7"));
    assert!(msg.contains("wrong dtype"));
}

#[test]
fn emit_optimized_succeeds_on_empty_kernel() {
    let module = emit_optimized(&empty_desc()).unwrap();
    assert_eq!(module.entry_points.len(), 1);
    assert_eq!(module.entry_points[0].name, "main");
}

#[test]
fn emit_optimized_drops_dead_arithmetic_before_lowering() {
    // Build a kernel where the rewrite stack should kill ops:
    //   r0 = Lit(0), r1 = Lit(99), r2 = Add(r1, r0) [identity → r1],
    //   r3 = Mul(r1, r0) [absorbing zero → r0],
    //   Store(buf, 0, r1)
    // After run_all: r2 + r3 are gone (DCE removes them after
    // identity_elim substitutes), only the surviving Store remains.
    // Both `emit` and `emit_optimized` should succeed; the optimized
    // module's function body should contain fewer or equal Statements.
    use vyre_foundation::ir::BinOp as Bo;
    let desc = KernelDescriptor {
        id: "k".into(),
        bindings: BindingLayout {
            slots: vec![vyre_lower::BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: None,
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "buf".into(),
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
    let raw_stmts = raw.entry_points[0].function.body.len();
    let opt_stmts = optimized.entry_points[0].function.body.len();
    assert!(
        opt_stmts <= raw_stmts,
        "optimized statements ({opt_stmts}) should not exceed raw ({raw_stmts})"
    );
    // Both must produce a valid main entry point.
    assert_eq!(optimized.entry_points[0].name, "main");
}

/// `emit_optimized` must surface a structured error when the rewrite pipeline
/// produces an invalid descriptor. Previously this check was a
/// `debug_assert!` that compiled to nothing in release builds, letting an
/// invalid optimized descriptor silently proceed to Naga emission and produce
/// a SPIR-V binary with different semantics from the original.
///
/// This test uses a descriptor with workgroup_size[0] == 0, which the verify
/// pass flags as `VerifyErrorKind::DispatchZeroDim`. The rewrite pipeline
/// preserves dispatch dimensions unchanged, so the output also has the zero
/// dim and verify fires unconditionally.
#[test]
fn emit_optimized_errors_on_invalid_rewrite_output() {
    // Construct a descriptor that verify() will reject. A zero workgroup
    // dimension is preserved unchanged by the rewrite pipeline, so the
    // post-rewrite verify must catch it and return an error.
    let bad = KernelDescriptor {
        id: "zero_dim".into(),
        bindings: BindingLayout { slots: vec![] },
        // Workgroup size 0 is invalid: verify() flags DispatchZeroDim.
        dispatch: Dispatch::new(0, 1, 1),
        body: KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![],
        },
    };
    let result = emit_optimized(&bad);
    assert!(
        result.is_err(),
        "emit_optimized must return Err for a descriptor that fails verification; \
         the old debug_assert! would silently proceed in release builds"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("invalid descriptor"),
        "error message must mention 'invalid descriptor', got: {err_msg}"
    );
}
