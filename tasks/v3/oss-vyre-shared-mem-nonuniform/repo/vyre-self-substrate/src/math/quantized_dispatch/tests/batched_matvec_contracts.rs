use super::*;

#[test]
fn i4x8_batched_matvec_f32_scaled_via_dispatches_boundary_batches() {
    let batch = 2_u32;
    let rows = 3_u32;
    let cols = 9_u32;
    let mut weights = Vec::new();
    for row in [
        &[-8, -7, -1, 0, 1, 2, 6, 7, -3][..],
        &[7, 6, 2, 1, 0, -1, -7, -8, 3][..],
        &[-4, 5, -6, 4, -2, 3, -5, 2, 1][..],
    ] {
        weights.extend(pack_i4x8_cpu(row));
    }
    let x_batches = [
        0.5, -1.0, 2.0, -0.25, 0.75, -1.5, 1.25, 0.125, -0.875, -0.25, 0.5, -0.75, 1.0, -1.25, 1.5,
        -1.75, 2.0, -2.25,
    ];
    let row_scales = [0.125, 0.25, 0.5];

    let out = i4x8_batched_matvec_f32_scaled_via(
        &QuantizedBatchedMatvecDispatcher,
        &weights,
        &x_batches,
        &row_scales,
        batch,
        rows,
        cols,
    )
    .expect("Fix: CUDA parity tests require backend dispatch; skip test if GPU unavailable, do not panic - fake dispatcher computes batched scaled INT4 matvec");
    let expected =
        i4x8_batched_matvec_f32_scaled_cpu(&weights, &x_batches, &row_scales, batch, rows, cols);

    assert_eq!(out.len(), (batch * rows) as usize);
    for (actual, expected) in out.iter().zip(expected.iter()) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
}

#[test]
fn i4x8_batched_matvec_f32_scaled_via_reuses_cached_program_for_same_shape() {
    let batch = 2_u32;
    let rows = 2_u32;
    let cols = 8_u32;
    let weights = pack_i4x8_cpu(&[-8, -1, 0, 7, 3, -3, 6, -6, 7, 1, -1, -8, 2, -2, 5, -5]);
    let x_batches = [
        1.0, -1.0, 0.5, 0.25, -0.75, 1.5, -0.5, 2.0, -0.125, 0.25, -0.5, 0.75, -1.0, 1.25, -1.5,
        1.75,
    ];
    let row_scales = [0.25, 0.5];
    let changed_x_batches = [
        1.0, -1.0, 0.5, 0.25, -0.75, 1.5, -0.5, 2.0, -0.125, 0.25, -0.5, 0.75, -1.0, 1.25, -1.5,
        1.75, 0.375, -0.625, 0.875, -1.125, 1.375, -1.625, 1.875, -2.125,
    ];
    let mut scratch = QuantizedBatchedMatvecGpuScratch::default();
    let mut out = Vec::new();

    i4x8_batched_matvec_f32_scaled_via_with_scratch_into(
        &QuantizedBatchedMatvecDispatcher,
        &weights,
        &x_batches,
        &row_scales,
        batch,
        rows,
        cols,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - first batched matvec shape succeeds");
    i4x8_batched_matvec_f32_scaled_via_with_scratch_into(
        &QuantizedBatchedMatvecDispatcher,
        &weights,
        &x_batches,
        &row_scales,
        batch,
        rows,
        cols,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - same batched matvec shape succeeds");
    assert_eq!(
        scratch.program_cache.builds(),
        1,
        "Fix: repeated same-shape INT4 batched matvec dispatch must reuse the primitive Program."
    );

    i4x8_batched_matvec_f32_scaled_via_with_scratch_into(
        &QuantizedBatchedMatvecDispatcher,
        &weights,
        &changed_x_batches,
        &row_scales,
        3,
        rows,
        cols,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - changed batched matvec shape succeeds");
    assert_eq!(
            scratch.program_cache.builds(),
            2,
            "Fix: INT4 batched matvec dispatch should rebuild the primitive Program only when batch/rows/cols changes."
        );
}

#[test]
fn i4x8_batched_matvec_f32_scaled_via_rejects_shape_errors_before_dispatch() {
    let weights = pack_i4x8_cpu(&[-1, 2, 3, -4, 5, -6, 7, -8]);
    let x_batches = [1.0; 16];
    let row_scales = [0.5];

    let err = i4x8_batched_matvec_f32_scaled_via(
        &QuantizedBatchedMatvecDispatcher,
        &weights,
        &x_batches,
        &row_scales,
        0,
        1,
        8,
    )
    .expect_err("zero batch must fail");
    assert!(err.to_string().contains("batch > 0"));

    let err = i4x8_batched_matvec_f32_scaled_via(
        &QuantizedBatchedMatvecDispatcher,
        &[],
        &x_batches,
        &row_scales,
        2,
        1,
        8,
    )
    .expect_err("missing weights must fail");
    assert!(err.to_string().contains("weights_packed.len()"));

    let err = i4x8_batched_matvec_f32_scaled_via(
        &QuantizedBatchedMatvecDispatcher,
        &weights,
        &x_batches[..15],
        &row_scales,
        2,
        1,
        8,
    )
    .expect_err("short x batch must fail");
    assert!(err.to_string().contains("x_batches.len() == batch*cols"));

    let err = i4x8_batched_matvec_f32_scaled_via(
        &QuantizedBatchedMatvecDispatcher,
        &weights,
        &x_batches,
        &[],
        2,
        1,
        8,
    )
    .expect_err("missing scale must fail");
    assert!(err.to_string().contains("row_scales.len() == rows"));
}

#[test]
fn i4x8_batched_matvec_f32_scaled_via_rejects_malformed_backend_outputs() {
    let weights = pack_i4x8_cpu(&[-1, 2, 3, -4, 5, -6, 7, -8]);
    let x_batches = [1.0; 16];
    let row_scales = [0.5];
    let no_outputs = MalformedDotDispatcher { outputs: vec![] };
    let err =
        i4x8_batched_matvec_f32_scaled_via(&no_outputs, &weights, &x_batches, &row_scales, 2, 1, 8)
            .expect_err("missing output must fail");
    assert!(err.to_string().contains("exactly one output"));

    let trailing_output = MalformedDotDispatcher {
        outputs: vec![vec![0; 12]],
    };
    let err = i4x8_batched_matvec_f32_scaled_via(
        &trailing_output,
        &weights,
        &x_batches,
        &row_scales,
        2,
        1,
        8,
    )
    .expect_err("trailing output bytes must fail");
    assert!(err.to_string().contains("expected 8 output bytes"));
}

