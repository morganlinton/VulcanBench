//! Backward for `layerwise_ln_scale`:
//!
//! Forward: `y[i] = x[i] * scale[i]`
//! Backward: `grad_x[i] = grad_out[i] * scale[i]`
//!           `grad_scale[i] = grad_out[i] * x[i]`

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::nn::ln_scale_backward";

/// Backward for layerwise_ln_scale (F32).
///
/// Produces `grad_x[n]` and `grad_scale[n]`.
#[must_use]
pub fn ln_scale_backward(
    input: &str,
    scale: &str,
    grad_out: &str,
    grad_x: &str,
    grad_scale: &str,
    n: u32,
) -> Program {
    let i = Expr::var("i");
    let x = Expr::load(input, i.clone());
    let s = Expr::load(scale, i.clone());
    let dy = Expr::load(grad_out, i.clone());

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![
                Node::Store {
                    buffer: grad_x.into(),
                    index: i.clone(),
                    value: Expr::mul(dy.clone(), s),
                },
                Node::Store {
                    buffer: grad_scale.into(),
                    index: i,
                    value: Expr::mul(dy, x),
                },
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(scale, 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(grad_out, 2, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(grad_x, 3, DataType::F32).with_count(n),
            BufferDecl::storage(grad_scale, 4, BufferAccess::ReadWrite, DataType::F32)
                .with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || ln_scale_backward("input", "scale", "grad_out", "grad_x", "grad_scale", 4),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[1.0, 2.0, 3.0, 4.0]),  // input
                to_f32(&[0.5, 2.0, 1.0, 0.1]),  // scale
                to_f32(&[1.0, 1.0, 1.0, 1.0]),  // grad_out
                vec![0u8; 4 * 4],                 // grad_scale
            ]]
        }),
        expected_output: Some(|| {
            // grad_x = dy * scale = [0.5, 2.0, 1.0, 0.1]
            // grad_scale = dy * input = [1.0, 2.0, 3.0, 4.0]
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[0.5, 2.0, 1.0, 0.1]),
                to_f32(&[1.0, 2.0, 3.0, 4.0]),
            ]]
        }),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::ln_scale_backward;
    use vyre_reference::value::Value;

    fn f32_bytes(values: &[f32]) -> Vec<u8> {
        vyre_primitives::wire::pack_f32_slice(values)
    }

    #[test]
    fn reference_outputs_grad_x_and_grad_scale_liveouts() {
        let program = ln_scale_backward("input", "scale", "grad_out", "grad_x", "grad_scale", 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[1.0, 2.0, 3.0, 4.0])),
                Value::from(f32_bytes(&[0.5, 2.0, 1.0, 0.1])),
                Value::from(f32_bytes(&[1.0, 1.0, 1.0, 1.0])),
                Value::from(vec![0_u8; 16]),
            ],
        )
        .expect("Fix: ln_scale_backward must satisfy the one-output plus ReadWrite live-out IR contract.");

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].to_bytes(), f32_bytes(&[0.5, 2.0, 1.0, 0.1]));
        assert_eq!(outputs[1].to_bytes(), f32_bytes(&[1.0, 2.0, 3.0, 4.0]));
    }
}
