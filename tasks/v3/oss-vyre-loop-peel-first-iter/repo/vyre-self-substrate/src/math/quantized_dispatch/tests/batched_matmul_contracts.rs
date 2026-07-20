use super::*;

#[test]
fn i4x8_batched_matmul_f32_scaled_via_dispatches_boundary_batches() {
    let batch = 2_u32;
    let rows = 3_u32;
    let cols = 9_u32;
    let weights = pack_i4_rows(&[
        &[-8, -7, -1, 0, 1, 2, 6, 7, -3],
        &[7, 6, 2, 1, 0, -1, -7, -8, 3],
        &[-4, 5, -6, 4, -2, 3, -5, 2, 1],
    ]);
    let activations = pack_i4_rows(&[
        &[7, 5, 3, 1, -1, -3, -5, -7, 6],
        &[-8, -6, -4, -2, 0, 2, 4, 6, 7],
    ]);
    let row_scales = [0.125, 0.25, 0.5];
    let batch_scales = [0.25, 0.375];

    let out = i4x8_batched_matmul_f32_scaled_via(
        &QuantizedBatchedMatmulDispatcher,
        &weights,
        &activations,
        &row_scales,
        &batch_scales,
        batch,
        rows,
        cols,
    )
    .expect("Fix: CUDA parity tests require backend dispatch; skip test if GPU unavailable, do not panic - fake dispatcher computes batched scaled INT4 matmul");
    let expected = i4x8_batched_matmul_f32_scaled_cpu(
        &weights,
        &activations,
        &row_scales,
        &batch_scales,
        batch,
        rows,
        cols,
    );

    assert_eq!(out.len(), (batch * rows) as usize);
    for (actual, expected) in out.iter().zip(expected.iter()) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
}

#[test]
fn i4x8_batched_matmul_f32_scaled_via_reuses_cached_program_for_same_shape() {
    let batch = 2_u32;
    let rows = 2_u32;
    let cols = 8_u32;
    let weights = pack_i4_rows(&[&[-8, -1, 0, 7, 3, -3, 6, -6], &[7, 1, -1, -8, 2, -2, 5, -5]]);
    let activations = pack_i4_rows(&[&[7, 5, 3, 1, -1, -3, -5, -7], &[-8, -6, -4, -2, 0, 2, 4, 6]]);
    let changed_activations = pack_i4_rows(&[
        &[7, 5, 3, 1, -1, -3, -5, -7],
        &[-8, -6, -4, -2, 0, 2, 4, 6],
        &[1, -1, 2, -2, 3, -3, 4, -4],
    ]);
    let row_scales = [0.25, 0.5];
    let batch_scales = [0.125, 0.375, 0.625];
    let mut scratch = QuantizedBatchedMatmulGpuScratch::default();
    let mut out = Vec::new();

    i4x8_batched_matmul_f32_scaled_via_with_scratch_into(
        &QuantizedBatchedMatmulDispatcher,
        &weights,
        &activations,
        &row_scales,
        &batch_scales[..2],
        batch,
        rows,
        cols,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - first batched matmul shape succeeds");
    i4x8_batched_matmul_f32_scaled_via_with_scratch_into(
        &QuantizedBatchedMatmulDispatcher,
        &weights,
        &activations,
        &row_scales,
        &batch_scales[..2],
        batch,
        rows,
        cols,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - same batched matmul shape succeeds");
    assert_eq!(
        scratch.program_cache.builds(),
        1,
        "Fix: repeated same-shape INT4 batched matmul dispatch must reuse the primitive Program."
    );

    i4x8_batched_matmul_f32_scaled_via_with_scratch_into(
        &QuantizedBatchedMatmulDispatcher,
        &weights,
        &changed_activations,
        &row_scales,
        &batch_scales,
        3,
        rows,
        cols,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - changed batched matmul shape succeeds");
    assert_eq!(
            scratch.program_cache.builds(),
            2,
            "Fix: INT4 batched matmul dispatch should rebuild the primitive Program only when batch/rows/cols changes."
        );
}

#[test]

fn i4x8_batched_matmul_f32_scaled_via_rejects_shape_errors_before_dispatch() {
    let weights = pack_i4_rows(&[&[-1, 2, 3, -4, 5, -6, 7, -8]]);
    let activations = pack_i4_rows(&[&[7, 5, 3, 1, -1, -3, -5, -7], &[-8, -6, -4, -2, 0, 2, 4, 6]]);
    let row_scales = [0.5];
    let batch_scales = [0.25, 0.375];

    let err = i4x8_batched_matmul_f32_scaled_via(
        &QuantizedBatchedMatmulDispatcher,
        &weights,
        &activations,
        &row_scales,
        &batch_scales,
        0,
        1,
        8,
    )
    .expect_err("zero batch must fail");
    assert!(err.to_string().contains("batch > 0"));

    let err = i4x8_batched_matmul_f32_scaled_via(
        &QuantizedBatchedMatmulDispatcher,
        &[],
        &activations,
        &row_scales,
        &batch_scales,
        2,
        1,
        8,
    )
    .expect_err("missing weights must fail");
    assert!(err.to_string().contains("weights_packed.len()"));

    let err = i4x8_batched_matmul_f32_scaled_via(
        &QuantizedBatchedMatmulDispatcher,
        &weights,
        &activations[..1],
        &row_scales,
        &batch_scales,
        2,
        1,
        8,
    )
    .expect_err("short activations must fail");
    assert!(err.to_string().contains("activation_batches_packed.len()"));

    let err = i4x8_batched_matmul_f32_scaled_via(
        &QuantizedBatchedMatmulDispatcher,
        &weights,
        &activations,
        &[],
        &batch_scales,
        2,
        1,
        8,
    )
    .expect_err("missing row scale must fail");
    assert!(err.to_string().contains("row_scales.len() == rows"));

    let err = i4x8_batched_matmul_f32_scaled_via(
        &QuantizedBatchedMatmulDispatcher,
        &weights,
        &activations,
        &row_scales,
        &batch_scales[..1],
        2,
        1,
        8,
    )
    .expect_err("missing batch scale must fail");
    assert!(err.to_string().contains("batch_scales.len() == batch"));
}

#[test]
fn i4x8_batched_matmul_f32_scaled_via_rejects_malformed_backend_outputs() {
    let weights = pack_i4_rows(&[&[-1, 2, 3, -4, 5, -6, 7, -8]]);
    let activations = pack_i4_rows(&[&[7, 5, 3, 1, -1, -3, -5, -7], &[-8, -6, -4, -2, 0, 2, 4, 6]]);
    let row_scales = [0.5];
    let batch_scales = [0.25, 0.375];
    let no_outputs = MalformedDotDispatcher { outputs: vec![] };
    let err = i4x8_batched_matmul_f32_scaled_via(
        &no_outputs,
        &weights,
        &activations,
        &row_scales,
        &batch_scales,
        2,
        1,
        8,
    )
    .expect_err("missing output must fail");
    assert!(err.to_string().contains("exactly one output"));

    let trailing_output = MalformedDotDispatcher {
        outputs: vec![vec![0; 12]],
    };
    let err = i4x8_batched_matmul_f32_scaled_via(
        &trailing_output,
        &weights,
        &activations,
        &row_scales,
        &batch_scales,
        2,
        1,
        8,
    )
    .expect_err("trailing output bytes must fail");
    assert!(err.to_string().contains("expected 8 output bytes"));
}

