use super::*;

#[test]

fn cuda_dispatch_matches_packed_int4_batched_scaled_matmul_oracle() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let weight_pattern = [-8, -3, -1, 0, 1, 3, 7, 6, 5, 4, 2, -2, -4, -6, -7, -5];
    let activation_pattern = [7, 5, 3, 1, -1, -3, -5, -7, 6, 4, 2, 0, -2, -4, -6, -8];

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
                weight_pattern
                    .iter()
                    .copied()
                    .cycle()
                    .skip(row * 5)
                    .take(cols as usize)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let activation_batches = (0..batch as usize)
            .map(|batch_index| {
                activation_pattern
                    .iter()
                    .copied()
                    .cycle()
                    .skip(batch_index * 7)
                    .take(cols as usize)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let row_scales = (0..rows)
            .map(|row| 0.125_f32 + row as f32 * 0.0625)
            .collect::<Vec<_>>();
        let batch_scales = (0..batch)
            .map(|batch_index| 0.25_f32 + batch_index as f32 * 0.03125)
            .collect::<Vec<_>>();
        let weights_packed = pack_i4_matrix_rows(&weights);
        let activation_batches_packed = pack_i4_matrix_rows(&activation_batches);
        let program = vyre_primitives::math::quantized::i4x8_batched_matmul_f32_scaled(
            "weights",
            "activations",
            "row_scales",
            "batch_scales",
            "out",
            batch,
            rows,
            cols,
        );
        let outputs = backend
            .dispatch(
                &program,
                &[
                    pack_u32(&weights_packed),
                    pack_u32(&activation_batches_packed),
                    pack_f32(&row_scales),
                    pack_f32(&batch_scales),
                ],
                &DispatchConfig::default(),
            )
            .expect(
                "Fix: CUDA must execute packed-activation batched INT4 matmul without CPU fallback.",
            );
        let expected = batched_packed_matmul_scaled_oracle(
            &weights_packed,
            &activation_batches_packed,
            &row_scales,
            &batch_scales,
            batch,
            rows,
            cols,
        );
        let actual = read_f32_vec(&outputs[0], (batch * rows) as usize);

        assert_eq!(
            actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            "batch={batch} rows={rows} cols={cols}"
        );
    }
}

#[test]
fn cuda_dispatch_matches_packed_int4_batched_scaled_matmul_top1_oracle() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let weight_pattern = [-8, -3, -1, 0, 1, 3, 7, 6, 5, 4, 2, -2, -4, -6, -7, -5];
    let activation_pattern = [7, 5, 3, 1, -1, -3, -5, -7, 6, 4, 2, 0, -2, -4, -6, -8];

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
                weight_pattern
                    .iter()
                    .copied()
                    .cycle()
                    .skip(row * 5)
                    .take(cols as usize)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let activation_batches = (0..batch as usize)
            .map(|batch_index| {
                activation_pattern
                    .iter()
                    .copied()
                    .cycle()
                    .skip(batch_index * 7)
                    .take(cols as usize)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let row_scales = (0..rows)
            .map(|row| 0.125_f32 + row as f32 * 0.0625)
            .collect::<Vec<_>>();
        let batch_scales = (0..batch)
            .map(|batch_index| 0.25_f32 + batch_index as f32 * 0.03125)
            .collect::<Vec<_>>();
        let weights_packed = pack_i4_matrix_rows(&weights);
        let activation_batches_packed = pack_i4_matrix_rows(&activation_batches);
        let program = vyre_primitives::math::quantized::i4x8_batched_matmul_top1_f32_scaled(
            "weights",
            "activations",
            "row_scales",
            "batch_scales",
            "out",
            batch,
            rows,
            cols,
        );
        let outputs = backend
            .dispatch(
                &program,
                &[
                    pack_u32(&weights_packed),
                    pack_u32(&activation_batches_packed),
                    pack_f32(&row_scales),
                    pack_f32(&batch_scales),
                ],
                &DispatchConfig::default(),
            )
            .expect(
                "Fix: CUDA must execute packed-activation INT4 top1 routing without CPU fallback.",
            );
        let (expected_scores, expected_indices) = batched_packed_matmul_top1_scaled_oracle(
            &weights_packed,
            &activation_batches_packed,
            &row_scales,
            &batch_scales,
            batch,
            rows,
            cols,
        );
        let actual_packed = read_f32_vec(&outputs[0], (batch * 2) as usize);
        let actual_scores = actual_packed[..batch as usize].to_vec();
        let actual_indices = actual_packed[batch as usize..]
            .iter()
            .map(|index| *index as u32)
            .collect::<Vec<_>>();

        assert_eq!(
            actual_scores
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            expected_scores
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            "batch={batch} rows={rows} cols={cols}"
        );
        assert_eq!(
            actual_indices, expected_indices,
            "batch={batch} rows={rows} cols={cols}"
        );
    }
}

