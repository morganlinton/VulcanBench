use super::*;

#[test]
fn cuda_dispatch_matches_packed_int4_scaled_matvec_oracle() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let pattern = [-8, -3, -1, 0, 1, 3, 7, 6, 5, 4, 2, -2, -4, -6, -7, -5];

    for (rows, cols) in [
        (1_u32, 1_u32),
        (2, 7),
        (3, 8),
        (4, 9),
        (5, 17),
        (6, 33),
        (7, 65),
    ] {
        let weights = (0..rows as usize)
            .map(|row| {
                pattern
                    .iter()
                    .copied()
                    .cycle()
                    .skip(row * 3)
                    .take(cols as usize)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let x = (0..cols)
            .map(|col| (col % 13) as f32 * 0.125 - 0.75)
            .collect::<Vec<_>>();
        let scales = (0..rows)
            .map(|row| 0.125_f32 + row as f32 * 0.0625)
            .collect::<Vec<_>>();
        let weights_packed = pack_i4_matrix_rows(&weights);
        let program = vyre_primitives::math::quantized::i4x8_matvec_f32_scaled(
            "weights", "x", "scales", "out", rows, cols,
        );
        let outputs = backend
            .dispatch(
                &program,
                &[pack_u32(&weights_packed), pack_f32(&x), pack_f32(&scales)],
                &DispatchConfig::default(),
            )
            .expect("Fix: CUDA must execute fused packed INT4 scaled matvec without CPU fallback.");
        let expected = matvec_scaled_oracle(&weights_packed, &x, &scales, rows, cols);
        let actual = read_f32_vec(&outputs[0], rows as usize);

        assert_eq!(
            actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            "rows={rows} cols={cols}"
        );
    }
}

#[test]
fn cuda_dispatch_matches_packed_int4_batched_scaled_matvec_oracle() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let pattern = [-8, -3, -1, 0, 1, 3, 7, 6, 5, 4, 2, -2, -4, -6, -7, -5];

    for (batch, rows, cols) in [
        (1_u32, 1_u32, 1_u32),
        (2, 2, 7),
        (3, 3, 8),
        (4, 4, 9),
        (5, 5, 17),
        (6, 6, 33),
        (3, 7, 65),
    ] {
        let weights = (0..rows as usize)
            .map(|row| {
                pattern
                    .iter()
                    .copied()
                    .cycle()
                    .skip(row * 5)
                    .take(cols as usize)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let x_batches = (0..batch * cols)
            .map(|index| (index % 17) as f32 * 0.0625 - 0.5)
            .collect::<Vec<_>>();
        let scales = (0..rows)
            .map(|row| 0.125_f32 + row as f32 * 0.0625)
            .collect::<Vec<_>>();
        let weights_packed = pack_i4_matrix_rows(&weights);
        let program = vyre_primitives::math::quantized::i4x8_batched_matvec_f32_scaled(
            "weights", "x", "scales", "out", batch, rows, cols,
        );
        let outputs = backend
            .dispatch(
                &program,
                &[
                    pack_u32(&weights_packed),
                    pack_f32(&x_batches),
                    pack_f32(&scales),
                ],
                &DispatchConfig::default(),
            )
            .expect(
                "Fix: CUDA must execute batched fused packed INT4 scaled matvec without CPU fallback.",
            );
        let expected =
            batched_matvec_scaled_oracle(&weights_packed, &x_batches, &scales, batch, rows, cols);
        let actual = read_f32_vec(&outputs[0], (batch * rows) as usize);

        assert_eq!(
            actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            "batch={batch} rows={rows} cols={cols}"
        );
    }
}

