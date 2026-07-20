use super::*;

pub(super) fn generated_i4_values(len: usize, seed: u32) -> Vec<i32> {
    (0..len)
        .map(|idx| {
            let mixed = (idx as u32)
                .wrapping_mul(17)
                .wrapping_add(seed.wrapping_mul(31))
                .wrapping_add((idx as u32 ^ seed).rotate_left((idx % 5) as u32));
            (mixed % 16) as i32 - 8
        })
        .collect()
}

pub(super) fn generated_f32_values(len: usize, seed: u32) -> Vec<f32> {
    (0..len)
        .map(|idx| {
            let signed = ((idx as i32 * 13 + seed as i32 * 7) % 17) - 8;
            signed as f32 * 0.125
        })
        .collect()
}

pub(super) fn generated_weight_rows(rows: u32, cols: u32, seed: u32) -> Vec<Vec<i32>> {
    (0..rows)
        .map(|row| generated_i4_values(cols as usize, seed.wrapping_add(row * 19)))
        .collect()
}

pub(super) fn generated_activation_rows(batch: u32, cols: u32, seed: u32) -> Vec<Vec<i32>> {
    (0..batch)
        .map(|batch_idx| generated_i4_values(cols as usize, seed.wrapping_add(batch_idx * 23)))
        .collect()
}

pub(super) fn pack_owned_i4_rows(rows: &[Vec<i32>]) -> Vec<u32> {
    let refs = rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    pack_i4_rows(&refs)
}

#[test]
pub(super) fn generated_quantized_wrappers_match_oracles_across_boundary_shapes() {
    for (case_idx, lane_count) in [1_u32, 7, 8, 9, 15, 16, 31, 32, 33, 65]
        .iter()
        .copied()
        .enumerate()
    {
        let lhs_values = generated_i4_values(lane_count as usize, case_idx as u32 + 1);
        let rhs_values = generated_i4_values(lane_count as usize, case_idx as u32 + 101);
        let lhs = pack_i4x8_cpu(&lhs_values);
        let rhs = pack_i4x8_cpu(&rhs_values);
        let lhs_scale = 0.125 + case_idx as f32 * 0.03125;
        let rhs_scale = 0.25 + case_idx as f32 * 0.015625;
        let actual = i4x8_dot_f32_scaled_via(
            &QuantizedDotDispatcher,
            &lhs,
            &rhs,
            lhs_scale,
            rhs_scale,
            lane_count,
        )
        .expect("Fix: CUDA parity tests require backend dispatch; skip test if GPU unavailable, do not panic - generated INT4 dot dispatch should match oracle");
        let expected = i4x8_dot_f32_scaled_cpu(&lhs, &rhs, lhs_scale, rhs_scale, lane_count);
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "dot lane_count={lane_count}"
        );
    }

    for (case_idx, (rows, cols)) in [(1_u32, 1_u32), (2, 7), (3, 8), (4, 9), (5, 17), (3, 33)]
        .iter()
        .copied()
        .enumerate()
    {
        let row_values = generated_weight_rows(rows, cols, 200 + case_idx as u32);
        let weights = pack_owned_i4_rows(&row_values);
        let x = generated_f32_values(cols as usize, 300 + case_idx as u32);
        let row_scales = generated_f32_values(rows as usize, 400 + case_idx as u32)
            .into_iter()
            .map(|value| value.abs() + 0.125)
            .collect::<Vec<_>>();
        let actual = i4x8_matvec_f32_scaled_via(
            &QuantizedMatvecDispatcher,
            &weights,
            &x,
            &row_scales,
            rows,
            cols,
        )
        .expect("Fix: CUDA parity tests require backend dispatch; skip test if GPU unavailable, do not panic - generated INT4 matvec dispatch should match oracle");
        let expected = i4x8_matvec_f32_scaled_cpu(&weights, &x, &row_scales, rows, cols);
        assert_eq!(
            actual
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            "matvec rows={rows} cols={cols}"
        );
    }

    for (case_idx, (batch, rows, cols)) in [
        (1_u32, 1_u32, 1_u32),
        (2, 2, 7),
        (3, 3, 8),
        (4, 4, 9),
        (5, 3, 17),
        (3, 5, 33),
    ]
    .iter()
    .copied()
    .enumerate()
    {
        let row_values = generated_weight_rows(rows, cols, 500 + case_idx as u32);
        let weights = pack_owned_i4_rows(&row_values);
        let x_batches = generated_f32_values((batch * cols) as usize, 600 + case_idx as u32);
        let row_scales = generated_f32_values(rows as usize, 700 + case_idx as u32)
            .into_iter()
            .map(|value| value.abs() + 0.125)
            .collect::<Vec<_>>();
        let actual = i4x8_batched_matvec_f32_scaled_via(
            &QuantizedBatchedMatvecDispatcher,
            &weights,
            &x_batches,
            &row_scales,
            batch,
            rows,
            cols,
        )
        .expect("Fix: CUDA parity tests require backend dispatch; skip test if GPU unavailable, do not panic - generated INT4 batched matvec dispatch should match oracle");
        let expected = i4x8_batched_matvec_f32_scaled_cpu(
            &weights,
            &x_batches,
            &row_scales,
            batch,
            rows,
            cols,
        );
        assert_eq!(
            actual
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            "batched matvec batch={batch} rows={rows} cols={cols}"
        );
    }

    for (case_idx, (batch, rows, cols)) in [
        (1_u32, 1_u32, 1_u32),
        (2, 2, 7),
        (3, 3, 8),
        (4, 4, 9),
        (5, 5, 17),
        (3, 7, 33),
    ]
    .iter()
    .copied()
    .enumerate()
    {
        let weight_rows = generated_weight_rows(rows, cols, 800 + case_idx as u32);
        let activation_rows = generated_activation_rows(batch, cols, 900 + case_idx as u32);
        let weights = pack_owned_i4_rows(&weight_rows);
        let activations = pack_owned_i4_rows(&activation_rows);
        let row_scales = generated_f32_values(rows as usize, 1000 + case_idx as u32)
            .into_iter()
            .map(|value| value.abs() + 0.125)
            .collect::<Vec<_>>();
        let batch_scales = generated_f32_values(batch as usize, 1100 + case_idx as u32)
            .into_iter()
            .map(|value| value.abs() + 0.25)
            .collect::<Vec<_>>();

        let actual = i4x8_batched_matmul_f32_scaled_via(
            &QuantizedBatchedMatmulDispatcher,
            &weights,
            &activations,
            &row_scales,
            &batch_scales,
            batch,
            rows,
            cols,
        )
        .expect("Fix: CUDA parity tests require backend dispatch; skip test if GPU unavailable, do not panic - generated INT4 batched matmul dispatch should match oracle");
        let expected = i4x8_batched_matmul_f32_scaled_cpu(
            &weights,
            &activations,
            &row_scales,
            &batch_scales,
            batch,
            rows,
            cols,
        );
        assert_eq!(
            actual
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            "batched matmul batch={batch} rows={rows} cols={cols}"
        );

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
        .expect("Fix: CUDA parity tests require backend dispatch; skip test if GPU unavailable, do not panic - generated INT4 top-1 dispatch should match oracle");
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
                .collect::<Vec<_>>(),
            "top1 scores batch={batch} rows={rows} cols={cols}"
        );
        assert_eq!(
            indices, expected_indices,
            "top1 indices batch={batch} rows={rows} cols={cols}"
        );
    }
}
