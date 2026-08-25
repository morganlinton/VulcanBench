//! RMS normalization: `y_i = x_i / sqrt(mean(x^2) + eps)`.
//!
//! Category-A composition with a workgroup-tiled reduction. The scalar
//! [`rms_norm_reference`] entry remains available as the correctness oracle.

use crate::{
    builder::{strided_accumulate_child, strided_writeback_child},
    nn::rms::{inverse_rms_expr, square_expr, EMPTY_RMS_FIX},
    region::wrap_anonymous,
};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_primitives::reduce::workgroup_tree::{self, WorkgroupReductionScope};

const OP_ID: &str = "vyre-libs::nn::rms_norm";
const REFERENCE_OP_ID: &str = "vyre-libs::nn::rms_norm_reference";
const RMS_TILE: u32 = 256;

/// Build a Program that applies RMSNorm element-wise.
#[must_use]
pub fn rms_norm(input: &str, output: &str, n: u32, eps: f32) -> Program {
    if n == 0 {
        return invalid_rms_program(OP_ID, output);
    }
    rms_norm_tiled_program(input, output, n, eps)
}

/// Build the scalar RMSNorm correctness reference.
#[must_use]
pub fn rms_norm_reference(input: &str, output: &str, n: u32, eps: f32) -> Program {
    if n == 0 {
        return invalid_rms_program(REFERENCE_OP_ID, output);
    }
    rms_norm_reference_program(input, output, n, eps)
}

fn invalid_rms_program(op_id: &'static str, output: &str) -> Program {
    crate::builder::invalid_output_program(op_id, output, DataType::F32, EMPTY_RMS_FIX.to_string())
}

fn rms_norm_tiled_program(input: &str, output: &str, n: u32, eps: f32) -> Program {
    let tile = RMS_TILE.min(n).max(1);
    let chunks = n.div_ceil(tile);
    let local = Expr::var("local");
    let mut body = vec![
        Node::let_bind("local", Expr::LocalId { axis: 0 }),
        strided_accumulate_child(
            OP_ID,
            tile,
            chunks,
            n,
            "local_sum",
            Expr::f32(0.0),
            "rms_scratch",
            |idx, acc| {
                let value = Expr::load(input, idx);
                Expr::add(acc, square_expr(value))
            },
        ),
        Node::barrier(),
    ];
    body.push(workgroup_tree::sum_f32_child(
        OP_ID,
        tile,
        "rms_scratch",
        WorkgroupReductionScope::FirstWorkgroup,
    ));
    body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::WorkgroupId { axis: 0 }, Expr::u32(0)),
            Expr::eq(local.clone(), Expr::u32(0)),
        ),
        vec![Node::Store {
            buffer: "rms_scale".into(),
            index: Expr::u32(0),
            value: inverse_rms_expr(Expr::load("rms_scratch", Expr::u32(0)), n, eps),
        }],
    ));
    body.push(Node::barrier());
    body.push(strided_writeback_child(
        OP_ID,
        tile,
        chunks,
        n,
        output,
        vec![Node::let_bind(
            "scale",
            Expr::load("rms_scale", Expr::u32(0)),
        )],
        |idx| Expr::mul(Expr::load(input, idx), Expr::var("scale")),
    ));

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::workgroup("rms_scratch", tile, DataType::F32),
            BufferDecl::workgroup("rms_scale", 1, DataType::F32),
            BufferDecl::output(output, 1, DataType::F32).with_count(n),
        ],
        [tile, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

fn rms_norm_reference_program(input: &str, output: &str, n: u32, eps: f32) -> Program {
    let body = vec![
        Node::let_bind("sum_sq", Expr::f32(0.0)),
        Node::loop_for(
            "k",
            Expr::u32(0),
            Expr::u32(n),
            vec![
                Node::let_bind("val", Expr::load(input, Expr::var("k"))),
                Node::assign(
                    "sum_sq",
                    Expr::add(Expr::var("sum_sq"), square_expr(Expr::var("val"))),
                ),
            ],
        ),
        Node::let_bind("rms", inverse_rms_expr(Expr::var("sum_sq"), n, eps)),
        Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(Expr::var("idx"), Expr::u32(n)),
            vec![Node::Store {
                buffer: output.into(),
                index: Expr::var("idx"),
                value: Expr::mul(Expr::load(input, Expr::var("idx")), Expr::var("rms")),
            }],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(output, 1, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(REFERENCE_OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::nn::rms_norm",
        build: || rms_norm("input", "output", 4, 1e-5),
        test_inputs: Some(|| {
            let to_bytes =
                |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            // Input = [1.0, 2.0, 3.0, 4.0].
            vec![vec![to_bytes(&[1.0, 2.0, 3.0, 4.0])]]
        }),
        expected_output: Some(|| {
            let to_bytes =
                |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            // mean(x^2) = (1+4+9+16)/4 = 7.5.
            // rms = inverseSqrt(7.5 + 1e-5).
            // y_i = x_i * rms.
            let mean_sq = (1.0_f32 + 4.0 + 9.0 + 16.0) / 4.0;
            let rms = (mean_sq + 1e-5_f32).sqrt().recip();
            let y: [f32; 4] = [1.0 * rms, 2.0 * rms, 3.0 * rms, 4.0 * rms];
            vec![vec![to_bytes(&y)]]
        }),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_f32;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    #[test]
    fn tiled_rms_norm_matches_scalar_reference_across_multiple_tiles() {
        let n = 777_u32;
        let eps = 1.0e-5_f32;
        let input = (0..n)
            .map(|i| ((i as f32) * 0.017).cos() * 3.0 + (i % 11) as f32 * 0.125)
            .collect::<Vec<_>>();
        let run = |program: Program| {
            let outputs = vyre_reference::reference_eval(
                &program,
                &[
                    Value::from(f32_bytes(&input)),
                    Value::from(vec![0u8; n as usize * 4]),
                ],
            )
            .expect("Fix: rms_norm program must execute in the reference interpreter.");
            decode_f32(&outputs[0].to_bytes())
        };
        let actual = run(rms_norm("input", "output", n, eps));
        let expected = run(rms_norm_reference("input", "output", n, eps));
        for (idx, (lhs, rhs)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                (lhs - rhs).abs() <= 1.0e-5,
                "rms_norm mismatch at lane {idx}: tiled={lhs:?} reference={rhs:?}"
            );
        }
    }

    #[test]
    fn generated_rms_norm_matches_reference_for_2048_lanes() {
        let n = 2048_u32;
        let eps = 1.0e-5_f32;
        let input = (0..n)
            .map(|i| {
                let wave = ((i as f32) * 0.011_718_75).sin() * 17.0;
                let saw = ((i % 37) as f32 - 18.0) * 0.03125;
                wave + saw
            })
            .collect::<Vec<_>>();
        let run = |program: Program| {
            let outputs = vyre_reference::reference_eval(
                &program,
                &[
                    Value::from(f32_bytes(&input)),
                    Value::from(vec![0u8; n as usize * 4]),
                ],
            )
            .expect("Fix: generated rms_norm program must execute in the reference interpreter.");
            decode_f32(&outputs[0].to_bytes())
        };
        let actual = run(rms_norm("input", "output", n, eps));
        let expected = run(rms_norm_reference("input", "output", n, eps));
        for (idx, (lhs, rhs)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                (lhs - rhs).abs() <= 1.0e-5,
                "generated rms_norm mismatch at lane {idx}: tiled={lhs:?} reference={rhs:?}"
            );
        }
    }

    #[test]
    fn zero_length_rms_norm_traps_without_panicking() {
        let program = rms_norm("input", "output", 0, 1.0e-5);
        let err = vyre_reference::reference_eval(
            &program,
            &[Value::from(vec![0u8; core::mem::size_of::<f32>()])],
        )
        .expect_err("zero-length rms_norm must trap instead of constructing a fake output");
        assert!(
            err.to_string().contains(EMPTY_RMS_FIX),
            "wrong error: {err}"
        );
    }

    // Adversarial float tests: expose tolerance misconfiguration gaps.

    #[test]
    fn rms_norm_very_small_variance_eps_dominates() {
        // All elements equal to tiny value → mean_sq = x^2, eps dominates.
        // output = x / sqrt(x^2 + eps) ≈ x / sqrt(eps).
        let n = 4u32;
        let eps = 1e-5_f32;
        let x = 1e-20f32;
        let input = [x; 4];
        let program = rms_norm("input", "output", n, eps);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 16])],
        )
        .expect("Fix: rms_norm must not panic on tiny input");
        let out = decode_f32(&outputs[0].to_bytes());
        let scale = 1.0 / (x * x + eps).sqrt();
        let expected = x * scale;
        for (i, &v) in out.iter().enumerate() {
            assert!(
                (v - expected).abs() <= 1.0e-6,
                "rms_norm tiny-input mismatch at {i}: {v} != {expected}"
            );
        }
    }

    #[test]
    fn rms_norm_very_large_variance() {
        // Large magnitude elements: mean_sq ≈ 1e20, sqrt(mean_sq) ≈ 1e10.
        // output = x / sqrt(mean_sq + eps) ≈ ±1.
        // We use 1e10 instead of 1e20 to avoid x^2 overflowing f32.
        let n = 4u32;
        let eps = 1e-5_f32;
        let input = [1e10f32, -1e10, 1e10, -1e10];
        let program = rms_norm("input", "output", n, eps);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 16])],
        )
        .expect("Fix: rms_norm must not panic on large-variance input");
        let out = decode_f32(&outputs[0].to_bytes());
        for (i, &v) in out.iter().enumerate() {
            assert!(
                v.is_finite(),
                "rms_norm large-variance output at {i} must be finite, got {v}"
            );
            assert!(
                (v.abs() - 1.0).abs() <= 1.0e-4,
                "rms_norm large-variance output at {i} should be ~±1, got {v}"
            );
        }
    }

    #[test]
    fn rms_norm_single_element() {
        // Single element: output = x / sqrt(x^2 + eps).
        let x = 5.0f32;
        let eps = 1e-5_f32;
        let input = [x];
        let program = rms_norm("input", "output", 1, eps);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 4])],
        )
        .expect("Fix: rms_norm single element must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        let expected = x / (x * x + eps).sqrt();
        assert!(
            (out[0] - expected).abs() <= 1.0e-6,
            "rms_norm single element mismatch: {} != {}",
            out[0],
            expected
        );
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn rms_norm_output_rms_is_one(input in prop::collection::vec(-1e10f32..1e10f32, 1..32)) {
            let n = input.len() as u32;
            let program = rms_norm("input", "output", n, 1e-5);
            let outputs = vyre_reference::reference_eval(
                &program,
                &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; input.len() * 4])],
            )
            .expect("Fix: rms_norm must execute");
            let out = decode_f32(&outputs[0].to_bytes());
            let mean_sq = out.iter().map(|v| v * v).sum::<f32>() / out.len() as f32;
            prop_assert!(
                (mean_sq - 1.0).abs() <= 1.0e-3,
                "rms_norm output RMS must be ~1, got {mean_sq}"
            );
        }
    }
}
