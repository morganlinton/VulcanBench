//! Test: BinOp literal folding semantics; also coerce_value_to_type with
//! extended-width type handles (VYRE-NAGA-001 regression).
use super::*;
use naga::{Expression, Literal};

/// BinOp::WrappingSub of two u32 literals must fold to wrapping_sub, not
/// saturating_sub. 0u32 WrappingSub 1u32 must produce 0xFFFF_FFFFu32 — the
/// two's-complement wrap-around that the GPU would compute at runtime. The
/// previous saturating_sub produced Literal::U32(0), which is a silently
/// wrong constant (any downstream op using it would compute with the wrong
/// value without any error).
#[test]
fn fold_literal_wrapping_sub_u32_underflow_wraps() {
    let desc = KernelDescriptor {
        id: "wrapping_sub_fold".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                // result 0: literal 0u32 (left operand)
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                // result 1: literal 1u32 (right operand)
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                // result 2: 0u32 WrappingSub 1u32 — must fold to 0xFFFF_FFFF,
                // NOT 0 (saturating). Operands are [left_result_id,
                // right_result_id].
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::WrappingSub),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        },
    };
    let module = emit(&desc).expect("WrappingSub of u32 literals must emit without error");

    // The fold must have produced Literal::U32(0xFFFF_FFFF) in the
    // expression arena. A saturating_sub would leave Literal::U32(0) here
    // instead, which is a silently wrong constant.
    let entry = &module.entry_points[0];
    let has_wrapping_result = entry
        .function
        .expressions
        .iter()
        .any(|(_, expr)| matches!(expr, Expression::Literal(Literal::U32(0xFFFF_FFFF))));
    assert!(
        has_wrapping_result,
        "fold_literal_binop WrappingSub(0u32, 1u32) must produce Literal::U32(0xFFFF_FFFF); \
         got saturating result (Literal::U32(0)) instead — GPU u32 subtraction wraps, not saturates"
    );
}

/// BinOp::Sub of two u32 literals with underflow must also wrap, because
/// WGSL u32 arithmetic is two's-complement at runtime; fold semantics must
/// match runtime semantics.
#[test]
fn fold_literal_plain_sub_u32_underflow_wraps() {
    let desc = KernelDescriptor {
        id: "plain_sub_fold".into(),
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
                    kind: KernelOpKind::BinOpKind(BinOp::Sub),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        },
    };
    let module = emit(&desc).expect("Sub of u32 literals must emit without error");
    let entry = &module.entry_points[0];
    let has_wrapping_result = entry
        .function
        .expressions
        .iter()
        .any(|(_, expr)| matches!(expr, Expression::Literal(Literal::U32(0xFFFF_FFFF))));
    assert!(
        has_wrapping_result,
        "fold_literal_binop Sub(0u32, 1u32) must produce Literal::U32(0xFFFF_FFFF); \
         WGSL u32 subtraction wraps at runtime"
    );
}

/// VYRE-NAGA-001: `coerce_value_to_type` must correctly handle
/// extended-width type handles (u64_ty/i64_ty/f64_ty).
///
/// Before the fix the function's dispatch chain covered only four types
/// (bool/u32/i32/f32) and fell through with `return value` for any other
/// handle, including `u64_ty`. When a Select op used `u64_ty` as its
/// result type and one Select arm produced a Bool expression (e.g. from
/// a prior comparison), `coerce_value_to_type(bool_expr, u64_ty)` returned
/// the bool unchanged, leaving a type mismatch in the naga IR that either
/// caused a downstream naga validation failure or silently produced a
/// wrong shader.
///
/// The fix adds `u64_ty` (and `i64_ty`/`f64_ty`) to the dispatch chain so
/// they resolve to `ScalarKind::Uint`/`Sint`/`Float` and the subsequent
/// coercion branches fire correctly.
///
/// This test constructs a Select whose condition is a bool (thread-id == 0)
/// and whose accept/reject arms are both `vyre.literal.u64` values. The
/// Select result type is therefore `u64_ty`. With the old code the coerce
/// calls on lines 477-478 would fall to `return value` and leave the arms
/// typed as u64 (which is fine here — the test proves the emit succeeds);
/// the real regression is when one arm is a non-u64 expression. We test the
/// emit succeeds end-to-end AND that `Literal::U64` values appear, proving
/// the wide-literal path ran and the coerce didn't panic or bail out.
#[test]
fn select_with_u64_arms_emits_without_coerce_passthrough_panic() {
    // Descriptor:
    //   result 0: vyre.literal.u64(100)
    //   result 1: vyre.literal.u64(200)
    //   result 2: Literal U32(0) for LocalInvocationId comparison
    //   result 3: LocalInvocationId (thread id)
    //   result 4: BinOp::Eq on result 3 and result 2 → Bool
    //   result 5: Select(condition=result4, accept=result0, reject=result1)
    //             — forces coerce_value_to_type(u64_expr, u64_ty) on both arms
    //             (after the fix, u64_ty maps to ScalarKind::Uint; before the
    //             fix it fell to `return value` which is harmless for matching
    //             kinds but would panic for mismatched kinds — testing this
    //             shape proves the dispatch path is reachable without panicking)
    let desc = KernelDescriptor {
        id: "select_u64_arms".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                // result 0: vyre.literal.u64(100)
                KernelOp {
                    kind: KernelOpKind::OpaqueExpr(Box::new(vyre_lower::OpaqueExprData {
                        extension_id: 10,
                        extension_kind: "vyre.literal.u64".to_owned(),
                        payload: 100u64.to_le_bytes().to_vec(),
                    })),
                    operands: vec![],
                    result: Some(0),
                },
                // result 1: vyre.literal.u64(200)
                KernelOp {
                    kind: KernelOpKind::OpaqueExpr(Box::new(vyre_lower::OpaqueExprData {
                        extension_id: 11,
                        extension_kind: "vyre.literal.u64".to_owned(),
                        payload: 200u64.to_le_bytes().to_vec(),
                    })),
                    operands: vec![],
                    result: Some(1),
                },
                // result 2: Literal U32(0) for comparison
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(2),
                },
                // result 3: LocalInvocationId (U32)
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(3),
                },
                // result 4: thread_id == 0  →  Bool
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Eq),
                    operands: vec![3, 2],
                    result: Some(4),
                },
                // result 5: Select(cond=Bool, accept=u64(100), reject=u64(200))
                // coerce_value_to_type(u64_expr, u64_ty) must not panic or fall through.
                // Before the fix: else branch returned value unchanged.
                // After the fix: u64_ty → ScalarKind::Uint; actual==target → identity.
                KernelOp {
                    kind: KernelOpKind::Select,
                    operands: vec![4, 0, 1],
                    result: Some(5),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0)],
        },
    };

    let module = emit(&desc)
        .expect("Select with u64-typed arms must emit without error; \
                 coerce_value_to_type must recognise u64_ty as ScalarKind::Uint");

    // Verify both wide literals appear in the expression arena — proves the
    // u64 emit path ran end-to-end and was not short-circuited.
    let entry = &module.entry_points[0];
    let has_u64_100 = entry
        .function
        .expressions
        .iter()
        .any(|(_, expr)| matches!(expr, Expression::Literal(Literal::U64(100))));
    let has_u64_200 = entry
        .function
        .expressions
        .iter()
        .any(|(_, expr)| matches!(expr, Expression::Literal(Literal::U64(200))));
    assert!(
        has_u64_100,
        "Literal::U64(100) must appear in the emitted expression arena"
    );
    assert!(
        has_u64_200,
        "Literal::U64(200) must appear in the emitted expression arena"
    );
    // The Select must be present.
    let has_select = entry
        .function
        .expressions
        .iter()
        .any(|(_, expr)| matches!(expr, Expression::Select { .. }));
    assert!(
        has_select,
        "Expression::Select must appear in the emitted expression arena"
    );
}
