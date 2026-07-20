//! Hashmap reference interpreter buffer-size contracts.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_reference::{reference_eval, value::Value};

#[test]
fn huge_declared_buffer_size_returns_structured_error() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("huge", 0, DataType::Vec4U32).with_count(u32::MAX),
            BufferDecl::output("out", 1, DataType::Vec4U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("huge", Expr::u32(0)),
        )],
    );

    let error = reference_eval(
        &program,
        &[Value::from(vec![0u8; 16]), Value::from(vec![0u8; 4])],
    )
    .expect_err("oversized declared input must not panic or allocate implicitly");
    let message = error.to_string();
    assert!(
        message.contains("huge") && message.contains("requires at least"),
        "buffer size diagnostic must name the buffer and required byte count, got: {message}"
    );
}
