//! Adversarial type-boundary validation tests.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::validate::validate;

#[test]
fn store_rejects_value_type_that_does_not_match_buffer_element() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::bool(true))],
    );

    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("type") || error.message().contains("U32")),
        "storing bool into u32 buffer must be rejected, got {errors:?}"
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
