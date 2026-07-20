use super::*;

#[test]
fn i4x8_batched_matmul_top1_f32_scaled_via_dispatches_boundary_batches() {
    let batch = 3_u32;
    let rows = 4_u32;
    let cols = 8_u32;
    let weights = pack_i4_rows(&[
        &[-8, -7, -1, 0, 1, 2, 6, 7],
        &[7, 6, 2, 1, 0, -1, -7, -8],
        &[-4, 5, -6, 4, -2, 3, -5, 2],
        &[3, -3, 4, -4, 5, -5, 6, -6],
    ]);
    let activations = pack_i4_rows(&[
        &[7, 5, 3, 1, -1, -3, -5, -7],
        &[-8, -6, -4, -2, 0, 2, 4, 6],
        &[1, -1, 2, -2, 3, -3, 4, -4],
    ]);
    let row_scales = [0.125, 0.25, 0.5, 0.75];
    let batch_scales = [0.25, 0.375, 0.625];

    let (scores, indices) = i4x8_batched_matmul_top1_f32_scaled_via(
        &QuantizedBatchedMatmulTop1Dispatcher,
        &weights,
        &activations,
        &row_scales,
        &batch_scales,
        batch,
        rows,
        cols,
    )
    .expect("Fix: CUDA parity tests require backend dispatch; skip test if GPU unavailable, do not panic - fake dispatcher computes top-1 packed INT4 routing");
    let (expected_scores, expected_indices) = i4x8_batched_matmul_top1_f32_scaled_cpu(
        &weights,
        &activations,
        &row_scales,
        &batch_scales,
        batch,
        rows,
        cols,
    );

    assert_eq!(
        scores
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected_scores
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
    assert_eq!(indices, expected_indices);
}

#[test]
fn i4x8_batched_matmul_top1_f32_scaled_via_reuses_cached_program_for_same_shape() {
    let batch = 2_u32;
    let rows = 3_u32;
    let cols = 8_u32;
    let weights = pack_i4_rows(&[
        &[-8, -1, 0, 7, 3, -3, 6, -6],
        &[7, 1, -1, -8, 2, -2, 5, -5],
        &[3, -3, 4, -4, 5, -5, 6, -6],
    ]);
    let activations = pack_i4_rows(&[&[7, 5, 3, 1, -1, -3, -5, -7], &[-8, -6, -4, -2, 0, 2, 4, 6]]);
    let changed_activations = pack_i4_rows(&[
        &[7, 5, 3, 1, -1, -3, -5, -7],
        &[-8, -6, -4, -2, 0, 2, 4, 6],
        &[1, -1, 2, -2, 3, -3, 4, -4],
    ]);
    let row_scales = [0.25, 0.5, 0.75];
    let batch_scales = [0.125, 0.375, 0.625];
    let mut scratch = QuantizedBatchedMatmulTop1GpuScratch::default();
    let mut scores = Vec::new();
    let mut indices = Vec::new();

    i4x8_batched_matmul_top1_f32_scaled_via_with_scratch_into(
        &QuantizedBatchedMatmulTop1Dispatcher,
        &weights,
        &activations,
        &row_scales,
        &batch_scales[..2],
        batch,
        rows,
        cols,
        &mut scratch,
        &mut scores,
        &mut indices,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - first top-1 shape succeeds");
    i4x8_batched_matmul_top1_f32_scaled_via_with_scratch_into(
        &QuantizedBatchedMatmulTop1Dispatcher,
        &weights,
        &activations,
        &row_scales,
        &batch_scales[..2],
        batch,
        rows,
        cols,
        &mut scratch,
        &mut scores,
        &mut indices,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - same top-1 shape succeeds");
    assert_eq!(
        scratch.program_cache.builds(),
        1,
        "Fix: repeated same-shape INT4 top-1 dispatch must reuse the primitive Program."
    );

    i4x8_batched_matmul_top1_f32_scaled_via_with_scratch_into(
        &QuantizedBatchedMatmulTop1Dispatcher,
        &weights,
        &changed_activations,
        &row_scales,
        &batch_scales,
        3,
        rows,
        cols,
        &mut scratch,
        &mut scores,
        &mut indices,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - changed top-1 shape succeeds");
    assert_eq!(
            scratch.program_cache.builds(),
            2,
            "Fix: INT4 top-1 dispatch should rebuild the primitive Program only when batch/rows/cols changes."
        );
}

#[test]
fn i4x8_batched_matmul_top1_f32_scaled_via_rejects_shape_errors_before_dispatch() {
    let weights = pack_i4_rows(&[&[-1, 2, 3, -4, 5, -6, 7, -8]]);
    let activations = pack_i4_rows(&[&[7, 5, 3, 1, -1, -3, -5, -7], &[-8, -6, -4, -2, 0, 2, 4, 6]]);
    let row_scales = [0.5];
    let batch_scales = [0.25, 0.375];

    let err = i4x8_batched_matmul_top1_f32_scaled_via(
        &QuantizedBatchedMatmulTop1Dispatcher,
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

    let err = i4x8_batched_matmul_top1_f32_scaled_via(
        &QuantizedBatchedMatmulTop1Dispatcher,
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

    let err = i4x8_batched_matmul_top1_f32_scaled_via(
        &QuantizedBatchedMatmulTop1Dispatcher,
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

    let err = i4x8_batched_matmul_top1_f32_scaled_via(
        &QuantizedBatchedMatmulTop1Dispatcher,
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

    let err = i4x8_batched_matmul_top1_f32_scaled_via(
        &QuantizedBatchedMatmulTop1Dispatcher,
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
fn i4x8_batched_matmul_top1_f32_scaled_via_rejects_malformed_backend_outputs() {
    let weights = pack_i4_rows(&[&[-1, 2, 3, -4, 5, -6, 7, -8]]);
    let activations = pack_i4_rows(&[&[7, 5, 3, 1, -1, -3, -5, -7], &[-8, -6, -4, -2, 0, 2, 4, 6]]);
    let row_scales = [0.5];
    let batch_scales = [0.25, 0.375];
    let no_outputs = MalformedDotDispatcher { outputs: vec![] };
    let err = i4x8_batched_matmul_top1_f32_scaled_via(
        &no_outputs,
        &weights,
        &activations,
        &row_scales,
        &batch_scales,
        2,
        1,
        8,
    )
    .expect_err("missing outputs must fail");
    assert!(err.to_string().contains("exactly two output buffers"));

    let one_output = MalformedDotDispatcher {
        outputs: vec![vec![0; 8]],
    };
    let err = i4x8_batched_matmul_top1_f32_scaled_via(
        &one_output,
        &weights,
        &activations,
        &row_scales,
        &batch_scales,
        2,
        1,
        8,
    )
    .expect_err("one output must fail");
    assert!(err.to_string().contains("exactly two output buffers"));

    let trailing_index_output = MalformedDotDispatcher {
        outputs: vec![vec![0; 8], vec![0; 12]],
    };
    let err = i4x8_batched_matmul_top1_f32_scaled_via(
        &trailing_index_output,
        &weights,
        &activations,
        &row_scales,
        &batch_scales,
        2,
        1,
        8,
    )
    .expect_err("trailing index output bytes must fail");
    assert!(err.to_string().contains("expected 8 output bytes"));
}
