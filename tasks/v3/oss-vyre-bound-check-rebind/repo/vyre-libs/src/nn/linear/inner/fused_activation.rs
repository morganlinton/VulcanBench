//! Shared fused linear + activation builder.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

pub(super) fn linear_fused_activation<F>(
    op_name: &'static str,
    op_id: &'static str,
    x: &str,
    w: &str,
    b: &str,
    out: &str,
    in_dim: u32,
    out_dim: u32,
    activation: F,
) -> Result<Program, String>
where
    F: FnOnce(Expr) -> Expr,
{
    if in_dim == 0 {
        return Err(format!(
            "Fix: {op_name} in_dim=0 is invalid: empty reduction"
        ));
    }
    if out_dim == 0 {
        return Err(format!("Fix: {op_name} out_dim=0 is invalid: empty output"));
    }
    let weight_count = in_dim.checked_mul(out_dim).ok_or_else(|| {
        format!("Fix: {op_name} in_dim*out_dim overflows u32; reduce dimensions.")
    })?;
    let i = Expr::var("i");
    let activated_acc = activation(Expr::var("acc"));
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(out_dim)),
            vec![
                Node::let_bind("acc", Expr::load(b, i.clone())),
                Node::loop_for(
                    "k",
                    Expr::u32(0),
                    Expr::u32(in_dim),
                    vec![Node::assign(
                        "acc",
                        Expr::add(
                            Expr::var("acc"),
                            Expr::mul(
                                Expr::load(x, Expr::var("k")),
                                Expr::load(
                                    w,
                                    Expr::add(
                                        Expr::mul(Expr::var("k"), Expr::u32(out_dim)),
                                        i.clone(),
                                    ),
                                ),
                            ),
                        ),
                    )],
                ),
                Node::Store {
                    buffer: out.into(),
                    index: i,
                    value: activated_acc,
                },
            ],
        ),
    ];
    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(x, 0, BufferAccess::ReadOnly, DataType::F32).with_count(in_dim),
            BufferDecl::storage(w, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(weight_count),
            BufferDecl::storage(b, 2, BufferAccess::ReadOnly, DataType::F32).with_count(out_dim),
            BufferDecl::output(out, 3, DataType::F32).with_count(out_dim),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(op_id, body)],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_fused_linear_activation_shape_matrix_builds() {
        let mut cases = 0usize;
        for in_dim in 1..=32 {
            for out_dim in 1..=32 {
                let program = linear_fused_activation(
                    "linear_identity",
                    "vyre-libs::nn::linear_identity",
                    "x",
                    "w",
                    "b",
                    "out",
                    in_dim,
                    out_dim,
                    |acc| acc,
                )
                .expect("Fix: generated fused linear activation dimensions must build.");
                let output = program
                    .buffers()
                    .iter()
                    .find(|buffer| buffer.is_output())
                    .expect("Fix: generated fused linear activation must declare output.");
                assert_eq!(output.count(), out_dim);
                cases += 1;
            }
        }
        assert_eq!(cases, 1_024);
    }

    #[test]
    fn fused_linear_activation_rejects_invalid_dimensions_and_overflow() {
        let empty_reduction = linear_fused_activation(
            "linear_identity",
            "vyre-libs::nn::linear_identity",
            "x",
            "w",
            "b",
            "out",
            0,
            1,
            |acc| acc,
        )
        .expect_err("empty reduction must be rejected");
        assert!(empty_reduction.contains("in_dim=0"));

        let empty_output = linear_fused_activation(
            "linear_identity",
            "vyre-libs::nn::linear_identity",
            "x",
            "w",
            "b",
            "out",
            1,
            0,
            |acc| acc,
        )
        .expect_err("empty output must be rejected");
        assert!(empty_output.contains("out_dim=0"));

        let overflow = linear_fused_activation(
            "linear_identity",
            "vyre-libs::nn::linear_identity",
            "x",
            "w",
            "b",
            "out",
            u32::MAX,
            2,
            |acc| acc,
        )
        .expect_err("weight element count overflow must be rejected");
        assert!(overflow.contains("overflows u32"));
    }
}
