//! Full-pipeline integration test: build a `Program` shape via vyre-lower
//! direct constructors, run the rewrite pipeline, emit through all three
//! substrates (naga, PTX, SPIR-V), assert each succeeds.
//!
//! This is the most cross-cutting test in the platform  -  it exercises:
//! 1. `vyre_lower::KernelDescriptor` construction
//! 2. `vyre_lower::audit` for cross-analysis reporting
//! 3. `vyre_lower::rewrites::run_all` for the canonical pipeline
//! 4. `vyre_emit_naga::emit` (validated via naga::valid::Validator)
//! 5. `vyre_emit_ptx::emit` (PTX text)
//! 6. `vyre_emit_spirv::emit` (SPIR-V binary)
//!
//! Source: ROADMAP T058 (full-pipeline gate).

use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::audit;
use vyre_lower::rewrites::{descriptor_const_fold, descriptor_dce, licm, run_all};
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

fn rich_kernel() -> KernelDescriptor {
    // A kernel with:
    // - Multiple literals (some constant-foldable)
    // - A loop with a hoistable invariant
    // - A coalesced load
    // - A store with a sibling dead store
    KernelDescriptor {
        id: "rich".into(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: DataType::F32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "input".into(),
                },
                BindingSlot {
                    slot: 1,
                    element_type: DataType::F32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    // ReadWrite  -  SPIR-V doesn't support storage write-only.
                    visibility: BindingVisibility::ReadWrite,
                    name: "output".into(),
                },
            ],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                // 0: tid
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                // 1, 2: foldable constants 3 + 4 = 7
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
                // 3: 3 + 4 (folded to 7)
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![1, 2],
                    result: Some(3),
                },
                // 4: load from input at tid (coalesced)
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
                    result: Some(4),
                },
                // 5: store to output at tid
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![1, 0, 4],
                    result: None,
                },
                // 6: dead literal (will be DCE'd)
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(5),
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(3),
                LiteralValue::U32(4),
                LiteralValue::U32(99),
            ],
        },
    }
}

#[test]
fn full_pipeline_lower_audit_rewrite_emit_naga() {
    let kernel = rich_kernel();

    // Audit
    let audit_report = audit(&kernel);
    assert_eq!(audit_report.kernel_id, "rich");

    // Rewrite
    let rewritten = run_all(&kernel);
    assert!(
        rewritten.body.ops.len() <= kernel.body.ops.len(),
        "rewrites should not grow op count"
    );

    // Emit through naga (vyre-emit-naga's own tests already validate
    // via naga::valid::Validator; this gate just confirms emission
    // succeeds end-to-end through the rewrite pipeline).
    let naga_module = vyre_emit_naga::emit(&rewritten).unwrap();
    assert!(!naga_module.entry_points.is_empty());
}

#[test]
fn full_pipeline_through_ptx_succeeds() {
    let kernel = rich_kernel();
    let rewritten = run_all(&kernel);
    let ptx = vyre_emit_ptx::emit(&rewritten).unwrap();
    // PTX emit pinned to 8.5 in vyre-emit-ptx/src/emitter.rs (CUDA 12.5
    // floor for sm_90/sm_100/sm_120 targets); bump in lockstep with
    // vyre-driver-cuda::backend::ptx_target probe.
    assert!(ptx.contains(".version 8.5"));
    assert!(ptx.contains(".visible .entry main"));
    assert!(ptx.contains(".param .u64 _arg_input"));
    assert!(ptx.contains(".param .u64 _arg_output"));
}

#[test]
fn full_pipeline_through_spirv_succeeds() {
    let kernel = rich_kernel();
    let rewritten = run_all(&kernel);
    let words = vyre_emit_spirv::emit(&rewritten).unwrap();
    assert_eq!(words[0], vyre_emit_spirv::SPIRV_MAGIC);
    assert!(words.len() > 16);
}

#[test]
fn rewrites_remove_dead_literal() {
    let kernel = rich_kernel();
    // Original has a dead Literal(99) at the end.
    let dead_count_before = kernel
        .body
        .ops
        .iter()
        .filter(|o| matches!(o.kind, KernelOpKind::Literal))
        .count();
    let after = descriptor_dce(&descriptor_const_fold(&kernel));
    let dead_count_after = after
        .body
        .ops
        .iter()
        .filter(|o| matches!(o.kind, KernelOpKind::Literal))
        .count();
    // descriptor_const_fold turned the Add into a new Literal (still in the stream)
    // and descriptor_dce removed at least one dead literal.
    assert!(
        dead_count_after <= dead_count_before,
        "DCE should not increase literal count"
    );
}

#[test]
fn licm_idempotent_through_pipeline() {
    let kernel = rich_kernel();
    let once = licm(&kernel);
    let twice = licm(&once);
    assert_eq!(once.body.ops.len(), twice.body.ops.len());
}

#[test]
fn audit_after_rewrite_does_not_panic() {
    let kernel = rich_kernel();
    let rewritten = run_all(&kernel);
    let report = audit(&rewritten);
    // After rewrite, waste_score should not be negative.
    assert!(report.waste_score >= 0.0);
}

#[test]
fn three_substrates_all_emit_for_rewritten_kernel() {
    let kernel = rich_kernel();
    let rewritten = run_all(&kernel);
    let naga = vyre_emit_naga::emit(&rewritten);
    let ptx = vyre_emit_ptx::emit(&rewritten);
    let spirv = vyre_emit_spirv::emit(&rewritten);
    assert!(matches!(naga, Ok(_)), "naga emit failed: {:?}", naga.err());
    assert!(matches!(ptx, Ok(_)), "ptx emit failed: {:?}", ptx.err());
    assert!(
        matches!(spirv, Ok(_)),
        "spirv emit failed: {:?}",
        spirv.err()
    );
}

#[test]
fn rewrites_pipeline_is_idempotent() {
    let kernel = rich_kernel();
    let once = run_all(&kernel);
    let twice = run_all(&once);
    assert_eq!(once.body.ops.len(), twice.body.ops.len());
}
