use super::*;

#[test]
fn generated_cuda_int4_release_parity_sweeps_boundary_shapes() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");

    for seed in 0_u32..8 {
        for lane_count in [1_u32, 2, 7, 8, 9, 15, 16, 31, 32, 33, 65, 96] {
            let lhs = generated_i4_values(lane_count as usize, seed.wrapping_mul(17) + 1);
            let rhs = generated_i4_values(lane_count as usize, seed.wrapping_mul(31) + 7);
            let lhs_packed = pack_i4x8(&lhs);
            let rhs_packed = pack_i4x8(&rhs);

            let dot_program =
                vyre_primitives::math::quantized::i4x8_dot_i32("lhs", "rhs", "out", lane_count);
            let dot_outputs = backend
                .dispatch(
                    &dot_program,
                    &[pack_u32(&lhs_packed), pack_u32(&rhs_packed)],
                    &DispatchConfig::default(),
                )
                .expect("Fix: generated CUDA INT4 i32 dot parity must dispatch on live GPU.");
            let dot_actual = read_i32(&dot_outputs[0]);
            let dot_expected = dot_i32_oracle(&lhs_packed, &rhs_packed, lane_count);
            assert_eq!(
                dot_actual, dot_expected,
                "generated i32 dot seed={seed} lane_count={lane_count}"
            );

            let lhs_scale = 0.0625_f32 * (1 + (seed % 7)) as f32;
            let rhs_scale = 0.03125_f32 * (1 + (lane_count % 9)) as f32;
            let program = vyre_primitives::math::quantized::i4x8_dot_f32_scaled(
                "lhs",
                "rhs",
                "lhs_scale",
                "rhs_scale",
                "out",
                lane_count,
            );
            let outputs = backend
                .dispatch(
                    &program,
                    &[
                        pack_u32(&lhs_packed),
                        pack_u32(&rhs_packed),
                        pack_f32(&[lhs_scale]),
                        pack_f32(&[rhs_scale]),
                    ],
                    &DispatchConfig::default(),
                )
                .expect("Fix: generated CUDA INT4 dot parity must dispatch on live GPU.");
            let actual = read_f32(&outputs[0]);
            let expected =
                dot_scaled_oracle(&lhs_packed, &rhs_packed, lhs_scale, rhs_scale, lane_count);
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "generated dot seed={seed} lane_count={lane_count}"
            );
        }
    }

    for seed in 0_u32..6 {
        for (rows, cols) in [
            (1_u32, 1_u32),
            (2, 7),
            (3, 8),
            (4, 9),
            (5, 17),
            (6, 33),
            (3, 64),
            (7, 65),
        ] {
            let weights = generated_i4_rows(rows, cols, seed.wrapping_mul(101) + 11);
            let x = generated_f32_values(cols as usize, seed.wrapping_mul(109) + rows + cols);
            let scales = generated_positive_scales(rows as usize, seed + rows * 13 + cols);
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
                .expect("Fix: generated CUDA INT4 matvec parity must dispatch on live GPU.");
            let actual = read_f32_vec(&outputs[0], rows as usize);
            let expected = matvec_scaled_oracle(&weights_packed, &x, &scales, rows, cols);
            assert_eq!(
                f32_bits(&actual),
                f32_bits(&expected),
                "generated matvec seed={seed} rows={rows} cols={cols}"
            );
        }
    }

    for seed in 0_u32..5 {
        for (batch, rows, cols) in [
            (1_u32, 1_u32, 1_u32),
            (2, 2, 7),
            (3, 3, 8),
            (4, 4, 9),
            (5, 5, 17),
            (3, 6, 33),
            (2, 7, 65),
        ] {
            let weights = generated_i4_rows(rows, cols, seed.wrapping_mul(127) + 19);
            let x_batches =
                generated_f32_values((batch * cols) as usize, seed.wrapping_mul(131) + 23);
            let scales = generated_positive_scales(rows as usize, seed + 29);
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
                .expect("Fix: generated CUDA INT4 batched matvec parity must dispatch.");
            let actual = read_f32_vec(&outputs[0], (batch * rows) as usize);
            let expected = batched_matvec_scaled_oracle(
                &weights_packed,
                &x_batches,
                &scales,
                batch,
                rows,
                cols,
            );
            assert_eq!(
                f32_bits(&actual),
                f32_bits(&expected),
                "generated batched matvec seed={seed} batch={batch} rows={rows} cols={cols}"
            );
        }
    }

    for seed in 0_u32..5 {
        for (batch, rows, cols) in [
            (1_u32, 1_u32, 1_u32),
            (2, 2, 7),
            (3, 3, 8),
            (4, 4, 9),
            (5, 5, 17),
            (3, 6, 33),
            (2, 7, 65),
        ] {
            let weights = generated_i4_rows(rows, cols, seed.wrapping_mul(149) + 31);
            let activations = generated_i4_rows(batch, cols, seed.wrapping_mul(151) + 37);
            let row_scales = generated_positive_scales(rows as usize, seed + 41);
            let batch_scales = generated_positive_scales(batch as usize, seed + 43);
            let weights_packed = pack_i4_matrix_rows(&weights);
            let activations_packed = pack_i4_matrix_rows(&activations);
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
                        pack_u32(&activations_packed),
                        pack_f32(&row_scales),
                        pack_f32(&batch_scales),
                    ],
                    &DispatchConfig::default(),
                )
                .expect("Fix: generated CUDA INT4 batched matmul parity must dispatch.");
            let actual = read_f32_vec(&outputs[0], (batch * rows) as usize);
            let expected = batched_packed_matmul_scaled_oracle(
                &weights_packed,
                &activations_packed,
                &row_scales,
                &batch_scales,
                batch,
                rows,
                cols,
            );
            assert_eq!(
                f32_bits(&actual),
                f32_bits(&expected),
                "generated batched matmul seed={seed} batch={batch} rows={rows} cols={cols}"
            );

            let top1_program =
                vyre_primitives::math::quantized::i4x8_batched_matmul_top1_f32_scaled(
                    "weights",
                    "activations",
                    "row_scales",
                    "batch_scales",
                    "out",
                    batch,
                    rows,
                    cols,
                );
            let top1_outputs = backend
                .dispatch(
                    &top1_program,
                    &[
                        pack_u32(&weights_packed),
                        pack_u32(&activations_packed),
                        pack_f32(&row_scales),
                        pack_f32(&batch_scales),
                    ],
                    &DispatchConfig::default(),
                )
                .expect("Fix: generated CUDA INT4 top1 parity must dispatch.");
            let (expected_scores, expected_indices) = batched_packed_matmul_top1_scaled_oracle(
                &weights_packed,
                &activations_packed,
                &row_scales,
                &batch_scales,
                batch,
                rows,
                cols,
            );
            let actual_packed = read_f32_vec(&top1_outputs[0], (batch * 2) as usize);
            let actual_scores = actual_packed[..batch as usize].to_vec();
            let actual_indices = actual_packed[batch as usize..]
                .iter()
                .map(|index| *index as u32)
                .collect::<Vec<_>>();
            assert_eq!(
                f32_bits(&actual_scores),
                f32_bits(&expected_scores),
                "generated top1 score seed={seed} batch={batch} rows={rows} cols={cols}"
            );
            assert_eq!(
                actual_indices, expected_indices,
                "generated top1 index seed={seed} batch={batch} rows={rows} cols={cols}"
            );
        }
    }
}

