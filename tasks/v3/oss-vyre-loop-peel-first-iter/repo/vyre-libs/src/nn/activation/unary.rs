//! Shared F32 unary activation Program builder.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

/// Build `output[i] = op(input[i])` for an F32 activation.
#[must_use]
pub(crate) fn f32_unary_activation_program<F>(
    op_id: &'static str,
    input: &str,
    output: &str,
    n: u32,
    op: F,
) -> Program
where
    F: Fn(Expr) -> Expr,
{
    let i = Expr::var("i");
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::buf_len(input)),
            vec![Node::Store {
                buffer: output.into(),
                index: i.clone(),
                value: op(Expr::load(input, i)),
            }],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(output, 1, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(op_id, body)],
    )
}
