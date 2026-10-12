//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn linear_relu_parity_with_sequential_linear_plus_relu() {
    let dims = [(4, 8), (16, 32), (64, 128), (128, 256), (256, 512)];
    let mut rng = 0x1234_5678_u64;
    let mut next_f32 = || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        f32::from_bits((rng >> 32) as u32)
    };

    for (in_dim, out_dim) in dims {
        let x: Vec<f32> = (0..in_dim).map(|_| next_f32()).collect();
        let w: Vec<f32> = (0..in_dim * out_dim).map(|_| next_f32()).collect();
        let b: Vec<f32> = (0..out_dim).map(|_| next_f32()).collect();

        let x_bytes = vyre_primitives::wire::pack_f32_slice(&x);
        let w_bytes = vyre_primitives::wire::pack_f32_slice(&w);
        let b_bytes = vyre_primitives::wire::pack_f32_slice(&b);

        // Run fused linear_relu
        let fused = linear_relu("x", "w", "b", "out", in_dim, out_dim).unwrap();
        let fused_out = vyre_reference::reference_eval(
            &fused,
            &[
                Value::from(x_bytes.clone()),
                Value::from(w_bytes.clone()),
                Value::from(b_bytes.clone()),
                Value::from(vec![0u8; (out_dim as usize) * 4]),
            ],
        )
        .unwrap();

        // Compute unfused reference: linear then relu
        let mut expected = vec![0.0f32; out_dim as usize];
        for i in 0..out_dim {
            let mut acc = b[i as usize];
            for k in 0..in_dim {
                acc += x[k as usize] * w[(k * out_dim + i) as usize];
            }
            expected[i as usize] = acc.max(0.0);
        }
        let expected_bytes = vyre_primitives::wire::pack_f32_slice(&expected);

        assert_eq!(
            fused_out[0].to_bytes(),
            expected_bytes,
            "linear_relu must match linear followed by relu for (in_dim={in_dim}, out_dim={out_dim})"
        );
    }
}

#[test]
fn linear_builder_rejects_mismatched_bias_dimensions() {
    use super::super::builder::Linear;
    use crate::tensor_ref::TensorRef;
    let err = Linear::new(
        TensorRef::u32_1d("x", 4),
        TensorRef::u32_2d("w", 4, 8),
        TensorRef::u32_1d("b", 4), // wrong: bias len 4 != out_dim 8
        TensorRef::u32_1d("out", 8),
    )
    .build()
    .unwrap_err();
    assert!(
        matches!(&err, crate::tensor_ref::TensorRefError::ShapeMismatch { name, .. } if name == "b"),
        "linear builder must reject bias with mismatched dimensions, got {err:?}"
    );
}

#[test]
fn linear_relu_clamps_negative_accumulators_without_clamping_positive_lanes() {
    let program = linear_relu("x", "w", "b", "out", 1, 3).unwrap();
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(vyre_primitives::wire::pack_f32_slice(&[2.0])),
            Value::from(vyre_primitives::wire::pack_f32_slice(&[-3.0, 0.0, 4.0])),
            Value::from(vyre_primitives::wire::pack_f32_slice(&[1.0, -5.0, -1.0])),
            Value::from(vec![0u8; 12]),
        ],
    )
    .expect("Fix: linear_relu must execute on mixed-sign accumulator lanes.");
    assert_eq!(
        outputs[0].to_bytes(),
        vyre_primitives::wire::pack_f32_slice(&[0.0, 0.0, 7.0]),
        "linear_relu must reuse activation ReLU semantics for fused outputs"
    );
}

// Adversarial tests for rms_norm_linear
