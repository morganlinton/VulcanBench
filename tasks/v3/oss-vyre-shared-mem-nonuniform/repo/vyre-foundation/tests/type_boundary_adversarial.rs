//! Adversarial type-boundary validation tests.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::validate::validate;

#[test]
fn store_rejects_value_type_that_does_not_match_buffer_element() {
    // `store_value_compatible` permits exact, U32<->Bytes, U32<->Bool, and
    // same-width INTEGER reinterprets -- but never F32<->U32 (a float is not a
    // same-width int and has no defined coercion into a u32 element), so an F32
    // value stored into a U32 buffer must be rejected.
    //
    // NB: Bool->U32 is INTENTIONALLY allowed since commit 45bb64b208 (a
    // comparison flag `a < b` stored straight into a u32 buffer), so this
    // adversarial case uses F32->U32, which remains a genuine type mismatch.
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::f32(1.0))],
    );

    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("type") || error.message().contains("U32")),
        "storing f32 into u32 buffer must be rejected, got {errors:?}"
    );
}

#[test]
fn store_rejects_non_integer_index_type() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::bool(false), Expr::u32(1))],
    );

    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("index") || error.message().contains("U32")),
        "bool buffer index must be rejected, got {errors:?}"
    );
}

#[test]
fn valid_u32_store_remains_accepted() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );

    let errors = validate(&program);
    assert!(
        errors.is_empty(),
        "valid u32 store must remain accepted, got {errors:?}"
    );
}
