//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn parity_rms_norm_linear_matches_reference_three_sizes() {
    for (n, in_dim, out_dim) in [(4_u32, 4_u32, 4_u32), (16, 64, 16), (64, 128, 64)] {
        parity_case(n, in_dim, out_dim);
    }
}

#[test]
fn try_rms_norm_linear_rejects_bad_dimensions_without_panic() {
    assert!(matches!(
        try_rms_norm_linear("input", "w", "b", "out", 0, 4, 4, 1e-5),
        Err(crate::tensor_ref::TensorRefError::ShapeMismatch { .. })
    ));
    assert!(matches!(
        try_rms_norm_linear("input", "w", "b", "out", 8, 4, 4, 1e-5),
        Err(crate::tensor_ref::TensorRefError::ShapeMismatch { .. })
    ));
    assert!(matches!(
        try_rms_norm_linear("input", "w", "b", "out", 1, u32::MAX, 2, 1e-5),
        Err(crate::tensor_ref::TensorRefError::ElementCountOverflow { .. })
    ));
}

#[test]
fn rms_norm_linear_very_small_variance_eps_dominates() {
    let n = 4u32;
    let in_dim = 4u32;
    let out_dim = 2u32;
    let eps = 1e-5_f32;
    let input = [3.0f32; 4];
    let weights: Vec<f32> = (0..(in_dim * out_dim)).map(|i| i as f32 * 0.1).collect();
    let bias = [0.0f32, 0.0];
    let fused = rms_norm_linear("input", "w", "b", "out", n, in_dim, out_dim, eps);
    let fused_inputs = vec![
        Value::from(to_f32_bytes(&input)),
        Value::from(to_f32_bytes(&weights)),
        Value::from(to_f32_bytes(&bias)),
        Value::from(vec![0u8; out_dim as usize * 4]),
    ];
    let fused_outputs = vyre_reference::reference_eval(&fused, &fused_inputs)
        .expect("Fix: rms_norm_linear must execute on zero-variance input");
    let fused_out = bytes_to_f32(&fused_outputs[0].to_bytes());
    for (i, &v) in fused_out.iter().enumerate() {
        assert!(
            v.is_finite(),
            "rms_norm_linear zero-variance output at {i} must be finite, got {v}"
        );
    }
}

#[test]
fn rms_norm_linear_very_large_variance() {
    let n = 4u32;
    let in_dim = 4u32;
    let out_dim = 2u32;
    let eps = 1e-5_f32;
    let input = [1e20f32, -1e20, 1e20, -1e20];
    let weights: Vec<f32> = (0..(in_dim * out_dim)).map(|i| i as f32 * 0.1).collect();
    let bias = [0.0f32, 0.0];
    let fused = rms_norm_linear("input", "w", "b", "out", n, in_dim, out_dim, eps);
    let fused_inputs = vec![
        Value::from(to_f32_bytes(&input)),
        Value::from(to_f32_bytes(&weights)),
        Value::from(to_f32_bytes(&bias)),
        Value::from(vec![0u8; out_dim as usize * 4]),
    ];
    let fused_outputs = vyre_reference::reference_eval(&fused, &fused_inputs)
        .expect("Fix: rms_norm_linear must execute on large-variance input");
    let fused_out = bytes_to_f32(&fused_outputs[0].to_bytes());
    for (i, &v) in fused_out.iter().enumerate() {
        assert!(
            v.is_finite(),
            "rms_norm_linear large-variance output at {i} must be finite, got {v}"
        );
    }
}

#[test]
fn rms_norm_linear_single_element() {
    let n = 1u32;
    let in_dim = 4u32;
    let out_dim = 2u32;
    let eps = 1e-5_f32;
    let input = [5.0f32, 0.0, 0.0, 0.0];
    let weights: Vec<f32> = (0..(in_dim * out_dim)).map(|i| i as f32 * 0.1).collect();
    let bias = [0.0f32, 0.0];
    let fused = rms_norm_linear("input", "w", "b", "out", n, in_dim, out_dim, eps);
    let fused_inputs = vec![
        Value::from(to_f32_bytes(&input)),
        Value::from(to_f32_bytes(&weights)),
        Value::from(to_f32_bytes(&bias)),
        Value::from(vec![0u8; out_dim as usize * 4]),
    ];
    let fused_outputs = vyre_reference::reference_eval(&fused, &fused_inputs)
        .expect("Fix: rms_norm_linear single element must execute");
    let fused_out = bytes_to_f32(&fused_outputs[0].to_bytes());
    let expected = linear_reference(
        &input,
        &input[0..1],
        &weights,
        &bias,
        out_dim,
        in_dim,
        n,
        eps,
    );
    compare_ulp(&fused_out, &expected, n, in_dim, out_dim);
}

#[test]
fn rms_norm_linear_empty_tensor_traps() {
    let result = try_rms_norm_linear("input", "w", "b", "out", 0, 4, 4, 1e-5);
    assert!(
        matches!(
            result,
            Err(crate::tensor_ref::TensorRefError::ShapeMismatch { .. })
        ),
        "rms_norm_linear n=0 must be rejected by the builder"
    );
}
