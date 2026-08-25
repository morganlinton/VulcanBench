//! Backward for `logit_softcap`: `d/dx [tanh(x/cap) * cap] = 1 - tanh²(x/cap)`.

use vyre::ir::{Expr, Program, UnOp};

use super::unary_f32::unary_f32_backward_program;

const OP_ID: &str = "vyre-libs::nn::logit_softcap_backward";

/// Backward for logit_softcap (F32).
#[must_use]
pub fn logit_softcap_backward(
    input: &str,
    grad_out: &str,
    grad_in: &str,
    n: u32,
    cap: f32,
) -> Program {
    unary_f32_backward_program(OP_ID, input, grad_out, grad_in, n, |x| {
        let z = Expr::div(x, Expr::f32(cap));
        let abs_z = Expr::UnOp {
            op: UnOp::Abs,
            operand: Box::new(z),
        };
        let exp_neg_2_abs_z = Expr::UnOp {
            op: UnOp::Exp,
            operand: Box::new(Expr::mul(Expr::f32(-2.0), abs_z)),
        };
        let denom = Expr::add(Expr::f32(1.0), exp_neg_2_abs_z.clone());
        Expr::div(
            Expr::mul(Expr::f32(4.0), exp_neg_2_abs_z),
            Expr::mul(denom.clone(), denom),
        )
    })
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || logit_softcap_backward("input", "grad_out", "grad_in", 4, 30.0),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[0.0, 15.0, -60.0, 100.0]),
                to_f32(&[1.0, 1.0, 1.0, 1.0]),
                vec![0u8; 4 * 4],
            ]]
        }),
        expected_output: Some(|| {
            let out = [
                f32::from_bits(0x3f80_0000),
                f32::from_bits(0x3f49_54a5),
                f32::from_bits(0x3d90_b161),
                f32::from_bits(0x3ba6_6206),
            ];
            let bytes = vyre_primitives::wire::pack_f32_slice(&out);
            vec![vec![bytes]]
        }),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_reference::value::Value;

    #[test]
    fn generated_logit_softcap_backward_matches_scalar_reference() {
        let n = 512usize;
        let cap = 30.0f32;
        let input = (0..n)
            .map(|i| ((i as i32 % 181) - 90) as f32 / 3.0)
            .collect::<Vec<_>>();
        let grad_out = (0..n)
            .map(|i| ((i as i32 % 43) - 21) as f32 / 9.0)
            .collect::<Vec<_>>();
        let program = logit_softcap_backward("input", "grad_out", "grad_in", n as u32, cap);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(vyre_primitives::wire::pack_f32_slice(&input)),
                Value::from(vyre_primitives::wire::pack_f32_slice(&grad_out)),
                Value::from(vec![0u8; n * core::mem::size_of::<f32>()]),
            ],
        )
        .expect("Fix: logit_softcap_backward must execute in the reference interpreter.");
        let actual = vyre_primitives::wire::decode_f32_le_bytes_all(&outputs[0].to_bytes());
        for (index, ((actual, x), dy)) in actual
            .iter()
            .copied()
            .zip(input.iter().copied())
            .zip(grad_out.iter().copied())
            .enumerate()
        {
            let t = (x / cap).tanh();
            let expected = dy * (1.0 - t * t);
            assert!(
                (actual - expected).abs() <= 1.0e-5,
                "generated logit_softcap_backward mismatch at {index}: {actual} != {expected}"
            );
        }
    }
}
