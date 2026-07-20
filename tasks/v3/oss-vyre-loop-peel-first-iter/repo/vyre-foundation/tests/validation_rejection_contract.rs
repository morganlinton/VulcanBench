//! Validation-layer rejection contract tests.
//!
//! Each test constructs a malformed program and asserts that the validator
//! emits the expected diagnostic. Negative assertions (allowed operations)
//! are included where the contract explicitly permits them.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::validate::validate;

fn output_program(nodes: Vec<Node>) -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        nodes,
    )
}

// ============================================================================
// 1. V044  -  Static zero divisor
// ============================================================================

#[test]
fn div_by_lit_u32_zero_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::div(Expr::u32(1), Expr::u32(0)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("V044") && e.message().contains("Div")),
        "Div by LitU32(0) must be rejected with V044, got {:?}",
        errors
    );
}

#[test]
fn mod_by_lit_i32_zero_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::rem(Expr::u32(1), Expr::i32(0)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("V044") && e.message().contains("Mod")),
        "Mod by LitI32(0) must be rejected with V044, got {:?}",
        errors
    );
}

// ============================================================================
// 2. U64/I64 arithmetic rejection
// ============================================================================

#[test]
fn add_u64_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::add(Expr::u64(1), Expr::u64(2)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("64-bit integer arithmetic")),
        "Add with U64 operands must be rejected, got {:?}",
        errors
    );
}

#[test]
fn mul_i64_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::mul(Expr::i64(1), Expr::i64(2)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("64-bit integer arithmetic")),
        "Mul with I64 operands must be rejected, got {:?}",
        errors
    );
}

// NOTE: BitAnd, BitOr, and BitXor on U64/I64 are currently REJECTED by the
// validator (legal set is U32/I32 only). Eq and Ne are ALLOWED. The tests
// below reflect actual validator behavior.

#[test]
fn bitand_u64_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::bitand(Expr::u64(1), Expr::u64(2)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("legal integer set is `u32` or `i32`")),
        "BitAnd with U64 operands must be rejected, got {:?}",
        errors
    );
}

#[test]
fn bitor_i64_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::bitor(Expr::i64(1), Expr::i64(2)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("legal integer set is `u32` or `i32`")),
        "BitOr with I64 operands must be rejected, got {:?}",
        errors
    );
}

#[test]
fn bitxor_u64_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::bitxor(Expr::u64(1), Expr::u64(2)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("legal integer set is `u32` or `i32`")),
        "BitXor with U64 operands must be rejected, got {:?}",
        errors
    );
}

#[test]
fn eq_u64_is_allowed() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::eq(Expr::u64(1), Expr::u64(2)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .all(|e| !e.message().contains("64-bit integer arithmetic")),
        "Eq with U64 operands must be allowed, got {:?}",
        errors
    );
}

#[test]
fn ne_i64_is_allowed() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::ne(Expr::i64(1), Expr::i64(2)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .all(|e| !e.message().contains("64-bit integer arithmetic")),
        "Ne with I64 operands must be allowed, got {:?}",
        errors
    );
}

// ============================================================================
// 3. Invalid cast rejection
// ============================================================================

#[test]
fn cast_vec2u32_to_bool_is_allowed() {
    // Vec2U32 -> Bool is explicitly supported in the cast matrix.
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::cast(DataType::Bool, Expr::cast(DataType::Vec2U32, Expr::u32(1))),
    )]);
    let errors = validate(&program);
    assert!(
        errors.is_empty(),
        "Cast from Vec2U32 to Bool must be allowed, got {:?}",
        errors
    );
}

#[test]
fn cast_u64_to_f32_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::cast(DataType::F32, Expr::u64(1)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("unsupported cast from `u64` to `f32`")),
        "Cast from U64 to F32 must be rejected, got {:?}",
        errors
    );
}

#[test]
fn cast_f32_to_bool_is_allowed() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::cast(DataType::Bool, Expr::f32(1.0)),
    )]);
    let errors = validate(&program);
    assert!(
        errors.is_empty(),
        "Cast from F32 to Bool must be allowed, got {:?}",
        errors
    );
}

// ============================================================================
// 4. Missing buffer rejection
// ============================================================================

#[test]
fn load_from_undeclared_buffer_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::load("missing", Expr::u32(0)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("load from unknown buffer `missing`")),
        "Load from undeclared buffer must be rejected, got {:?}",
        errors
    );
}

#[test]
fn store_to_undeclared_buffer_is_rejected() {
    let program = output_program(vec![Node::store("missing", Expr::u32(0), Expr::u32(1))]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("store to unknown buffer `missing`")),
        "Store to undeclared buffer must be rejected, got {:?}",
        errors
    );
}

#[test]
fn buflen_of_undeclared_buffer_is_rejected() {
    let program = output_program(vec![Node::let_bind("x", Expr::buf_len("missing"))]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("buflen of unknown buffer `missing`")),
        "BufLen of undeclared buffer must be rejected, got {:?}",
        errors
    );
}

// ============================================================================
// 5. Unknown op call rejection
// ============================================================================

// NOTE: `DialectLookup` is a sealed trait (`private::Sealed`), so integration
// tests cannot construct a custom lookup. Without an active lookup the
// validator silently accepts unknown `Expr::Call` nodes. The rejection
// contract for unknown op calls is enforced in the in-crate unit tests at
// `src/validate/expr_rules.rs` (`call_resolution_uses_supplied_lookup`).

// ============================================================================
// 6. Type mismatch in Select
// ============================================================================

#[test]
fn select_with_mismatched_branch_types_is_rejected() {
    let program = output_program(vec![Node::let_bind(
        "x",
        Expr::select(Expr::bool(true), Expr::u32(1), Expr::f32(2.0)),
    )]);
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("V029") && e.message().contains("mismatched types")),
        "Select with mismatched branch types must be rejected with V029, got {:?}",
        errors
    );
}

#[test]
fn assignment_type_mismatch_is_rejected() {
    let program = output_program(vec![
        Node::let_bind("x", Expr::u32(1)),
        Node::assign("x", Expr::f32(1.0)),
    ]);
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e.message().contains("V045")),
        "Assigning F32 into a U32 binding must be rejected with V045, got {:?}",
        errors
    );
}
