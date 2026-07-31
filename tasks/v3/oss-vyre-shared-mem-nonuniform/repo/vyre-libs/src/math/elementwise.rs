//! Shared elementwise Program builders.
//!
//! Category-A math and NN wrappers keep domain-specific names and op ids, but
//! the repeated per-lane load/compute/store skeleton lives here.

use crate::builder::BuildOptions;
use crate::region::wrap_anonymous;
use crate::tensor_ref::{TensorRef, TensorRefError};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Right-hand side source for an elementwise F32 multiply.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum F32MulRhs<'a> {
    /// Reuse the left input as RHS, producing `x * x`.
    SameInput,
    /// Read RHS from a second buffer.
    Buffer(&'a str),
}

/// Build `output[i] = input[i] * rhs[i]` over F32 lanes.
#[must_use]
pub(crate) fn f32_elementwise_mul(
    op_id: &'static str,
    input: &str,
    rhs: F32MulRhs<'_>,
    output: &str,
    n: u32,
) -> Program {
    let i = Expr::var("i");
    let lhs_value = Expr::load(input, i.clone());
    let rhs_value = match rhs {
        F32MulRhs::SameInput => lhs_value.clone(),
        F32MulRhs::Buffer(buffer) => Expr::load(buffer, i.clone()),
    };
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![Node::Store {
                buffer: output.into(),
                index: i,
                value: Expr::mul(lhs_value, rhs_value),
            }],
        ),
    ];
    let mut buffers =
        vec![BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n)];
    if let F32MulRhs::Buffer(buffer) = rhs {
        buffers.push(
            BufferDecl::storage(buffer, 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
        );
        buffers.push(BufferDecl::output(output, 2, DataType::F32).with_count(n));
    } else {
        buffers.push(BufferDecl::output(output, 1, DataType::F32).with_count(n));
    }
    Program::wrapped(buffers, [64, 1, 1], vec![wrap_anonymous(op_id, body)])
}

/// Build a checked elementwise unary u32 operation.
pub(crate) fn try_u32_elementwise_unary<F>(
    op_id: &'static str,
    input: &str,
    out: &str,
    size: u32,
    op: F,
) -> Result<Program, TensorRefError>
where
    F: Fn(Expr) -> Expr,
{
    crate::builder::build_elementwise_unary(
        op_id,
        TensorRef::u32_1d(input, size),
        TensorRef::u32_1d(out, size),
        BuildOptions::default(),
        op,
    )
}

/// Build an elementwise unary u32 operation with a diagnostic invalid-program fallback.
#[must_use]
pub(crate) fn u32_elementwise_unary<F>(
    op_id: &'static str,
    input: &str,
    out: &str,
    size: u32,
    op: F,
) -> Program
where
    F: Fn(Expr) -> Expr,
{
    try_u32_elementwise_unary(op_id, input, out, size, op).unwrap_or_else(|err| {
        crate::builder::invalid_output_program(op_id, out, DataType::U32, format!("Fix: {err}"))
    })
}

/// Build a checked elementwise binary u32 operation.
pub(crate) fn try_u32_elementwise_binary<F>(
    op_id: &'static str,
    a: &str,
    b: &str,
    out: &str,
    size: u32,
    op: F,
) -> Result<Program, TensorRefError>
where
    F: Fn(Expr, Expr) -> Expr,
{
    crate::builder::build_elementwise_binary(
        op_id,
        TensorRef::u32_1d(a, size),
        TensorRef::u32_1d(b, size),
        TensorRef::u32_1d(out, size),
        BuildOptions::default(),
        op,
    )
}

/// Build an elementwise binary u32 operation with a diagnostic invalid-program fallback.
#[must_use]
pub(crate) fn u32_elementwise_binary<F>(
    op_id: &'static str,
    a: &str,
    b: &str,
    out: &str,
    size: u32,
    op: F,
) -> Program
where
    F: Fn(Expr, Expr) -> Expr,
{
    try_u32_elementwise_binary(op_id, a, b, out, size, op).unwrap_or_else(|err| {
        crate::builder::invalid_output_program(op_id, out, DataType::U32, format!("Fix: {err}"))
    })
}
