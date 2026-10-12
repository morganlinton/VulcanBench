//! Parallel residual block: `out = x + attn_out + mlp_out`.
//!
//! Category A composition  -  residual stream addition.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::nn::parallel_residual_block";

/// Build parallel residual block (F32).
///
/// # Errors
/// Returns `Err` if n is zero.
pub fn parallel_residual_block(
    x: &str,
    attn_out: &str,
    mlp_out: &str,
    output: &str,
    n: u32,
) -> Result<Program, String> {
    if n == 0 {
        return Err("Fix: n=0".into());
    }
    let i = Expr::var("i");
    let result = Expr::add(
        Expr::add(Expr::load(x, i.clone()), Expr::load(attn_out, i.clone())),
        Expr::load(mlp_out, i.clone()),
    );

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![Node::Store {
                buffer: output.into(),
                index: i,
                value: result,
            }],
        ),
    ];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(x, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(attn_out, 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(mlp_out, 2, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(output, 3, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    ))
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || {
            parallel_residual_block("x", "attn", "mlp", "out", 4)
                .unwrap_or_else(|error| crate::invalid_program(OP_ID, format!("Fix: parallel_residual_block fixture must build: {error}")))
        },
        test_inputs: Some(|| {
            let f = vyre_primitives::wire::pack_f32_slice;
            vec![vec![
                f(&[1.0, 2.0, 3.0, 4.0]), f(&[0.1, 0.2, 0.3, 0.4]),
                f(&[0.01, 0.02, 0.03, 0.04]),
            ]]
        }),
        expected_output: Some(|| {
            let out = [1.11_f32, 2.22, 3.33, 4.44];
            let bytes = vyre_primitives::wire::pack_f32_slice(&out);
            vec![vec![bytes]]
        }),
        category: Some("nn"),
    }
}
