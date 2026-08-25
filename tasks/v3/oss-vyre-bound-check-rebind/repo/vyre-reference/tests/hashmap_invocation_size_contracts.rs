//! Hashmap reference interpreter invocation-count contracts.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_reference::{reference_eval, value::Value};

#[test]
fn huge_workgroup_dimensions_return_structured_error() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [u32::MAX, 2, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );

    let error = reference_eval(&program, &[Value::from(vec![0u8; 4])])
        .expect_err("huge workgroup dimensions must not allocate or panic");
    let message = error.to_string();
    assert!(
        message.contains("workgroup") || message.contains("validation"),
        "oversized invocation diagnostic must be structured and actionable, got: {message}"
    );
}
