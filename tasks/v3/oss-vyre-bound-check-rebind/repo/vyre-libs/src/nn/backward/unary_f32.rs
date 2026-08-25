//! Shared F32 unary backward kernel builder.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

pub(super) fn unary_f32_backward_program<F>(
    op_id: &'static str,
    input: &str,
    grad_out: &str,
    grad_in: &str,
    n: u32,
    local_grad: F,
) -> Program
where
    F: FnOnce(Expr) -> Expr,
{
    let i = Expr::var("i");
    let x = Expr::load(input, i.clone());
    let dy = Expr::load(grad_out, i.clone());
    let grad = Expr::mul(dy, local_grad(x));
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![Node::Store {
                buffer: grad_in.into(),
                index: i,
                value: grad,
            }],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(grad_out, 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(grad_in, 2, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(op_id, body)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_unary_backward_program_lengths_are_declared_exactly() {
        let mut cases = 0usize;
        for n in 0..=2048 {
            let program = unary_f32_backward_program(
                "vyre-libs::nn::test_unary_backward",
                "input",
                "grad_out",
                "grad_in",
                n,
                |x| x,
            );
            assert_eq!(program.buffers().len(), 3);
            let output = program
                .buffers()
                .iter()
                .find(|buffer| buffer.is_output())
                .expect("Fix: unary backward program must declare grad output.");
            assert_eq!(output.count(), n);
            cases += 1;
        }
        assert_eq!(cases, 2_049);
    }
}
