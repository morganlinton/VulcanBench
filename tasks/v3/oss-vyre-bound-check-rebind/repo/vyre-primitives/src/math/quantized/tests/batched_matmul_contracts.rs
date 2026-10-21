use super::*;

#[test]
fn batched_matmul_cpu_matches_dequantized_reference() {
    let weights = vec![
        vec![1, 2, 3, 4, -1, -2, -3, -4, 5],
        vec![4, 3, 2, 1, -4, -3, -2, -1, -5],
        vec![-8, -7, -6, -5, -4, -3, -2, -1, 0],
    ];
    let activations = vec![
        vec![1, -1, 2, -2, 3, -3, 4, -4, 5],
        vec![-5, 4, -4, 3, -3, 2, -2, 1, -1],
    ];
    let row_scales = [0.5_f32, 0.25, 0.125];
    let batch_scales = [0.25_f32, 0.5];
    let weights_packed = pack_i4_matrix_rows(&weights);
    let activations_packed = pack_i4_matrix_rows(&activations);
    let actual = i4x8_batched_matmul_f32_scaled_cpu(
        &weights_packed,
        &activations_packed,
        &row_scales,
        &batch_scales,
        2,
        3,
        9,
    );
    let expected = activations
        .iter()
        .zip(batch_scales)
        .flat_map(|(activation, batch_scale)| {
            weights.iter().zip(row_scales).map(move |(row, row_scale)| {
                row.iter()
                    .zip(activation)
                    .fold(0.0_f32, |acc, (&w, &x)| acc + w as f32 * x as f32)
                    * row_scale
                    * batch_scale
            })
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
}

#[test]
fn batched_matmul_top1_cpu_matches_full_matmul_argmax() {
    let weights = vec![
        vec![1, 2, 3, 4, -1, -2, -3, -4, 5],
        vec![4, 3, 2, 1, -4, -3, -2, -1, -5],
        vec![-8, -7, -6, -5, -4, -3, -2, -1, 0],
    ];
    let activations = vec![
        vec![1, -1, 2, -2, 3, -3, 4, -4, 5],
        vec![-5, 4, -4, 3, -3, 2, -2, 1, -1],
    ];
    let row_scales = [0.5_f32, 0.25, 0.125];
    let batch_scales = [0.75_f32, 0.375];
    let weights_packed = pack_i4_matrix_rows(&weights);
    let activations_packed = pack_i4_matrix_rows(&activations);
    let logits = i4x8_batched_matmul_f32_scaled_cpu(
        &weights_packed,
        &activations_packed,
        &row_scales,
        &batch_scales,
        activations.len() as u32,
        weights.len() as u32,
        weights[0].len() as u32,
    );
    let (scores, indices) = i4x8_batched_matmul_top1_f32_scaled_cpu(
        &weights_packed,
        &activations_packed,
        &row_scales,
        &batch_scales,
        activations.len() as u32,
        weights.len() as u32,
        weights[0].len() as u32,
    );

    for batch_index in 0..activations.len() {
        let row_start = batch_index * weights.len();
        let (expected_index, expected_score) = (0..weights.len())
            .map(|row| (row as u32, logits[row_start + row]))
            .max_by(|(_, lhs), (_, rhs)| lhs.total_cmp(rhs))
            .expect("Fix: top1 test requires at least one row.");
        assert_eq!(indices[batch_index], expected_index);
        assert_eq!(scores[batch_index].to_bits(), expected_score.to_bits());
    }
}

#[test]

fn generated_batched_matmul_matches_dequantized_reference_across_pack_boundaries() {
    let pattern = [-8, -3, -1, 0, 1, 3, 7, 6, 5, 4, 2, -2, -4, -6, -7, -5];
    for batch in 1..=4_u32 {
        for rows in 1..=5_u32 {
            for cols in [1_u32, 7, 8, 9, 16, 17, 31, 32, 33] {
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
                let activations = (0..batch as usize)
                    .map(|batch_index| {
                        pattern
                            .iter()
                            .copied()
                            .cycle()
                            .skip(batch_index * 5 + 1)
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
                let activations_packed = pack_i4_matrix_rows(&activations);
                let actual = i4x8_batched_matmul_f32_scaled_cpu(
                    &weights_packed,
                    &activations_packed,
                    &row_scales,
                    &batch_scales,
                    batch,
                    rows,
                    cols,
                );
                let expected = activations
                    .iter()
                    .zip(batch_scales.iter().copied())
                    .flat_map(|(activation, batch_scale)| {
                        weights.iter().zip(row_scales.iter().copied()).map(
                            move |(row, row_scale)| {
                                row.iter()
                                    .zip(activation)
                                    .fold(0.0_f32, |acc, (&w, &x)| acc + w as f32 * x as f32)
                                    * row_scale
                                    * batch_scale
                            },
                        )
                    })
                    .collect::<Vec<_>>();

                assert_eq!(
                    actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    "batch={batch} rows={rows} cols={cols}"
                );
            }
        }
    }
}

#[test]
fn generated_batched_matmul_top1_matches_full_matmul_across_pack_boundaries() {
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
        let activations = (0..batch as usize)
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
        let activations_packed = pack_i4_matrix_rows(&activations);
        let logits = i4x8_batched_matmul_f32_scaled_cpu(
            &weights_packed,
            &activations_packed,
            &row_scales,
            &batch_scales,
            batch,
            rows,
            cols,
        );
        let (scores, indices) = i4x8_batched_matmul_top1_f32_scaled_cpu(
            &weights_packed,
            &activations_packed,
            &row_scales,
            &batch_scales,
            batch,
            rows,
            cols,
        );

        for batch_index in 0..batch as usize {
            let row_start = batch_index * rows as usize;
            let (expected_index, expected_score) = (0..rows as usize)
                .map(|row| (row as u32, logits[row_start + row]))
                .max_by(|(_, lhs), (_, rhs)| lhs.total_cmp(rhs))
                .expect("Fix: top1 generated test requires at least one row.");
            assert_eq!(
                indices[batch_index], expected_index,
                "batch={batch} rows={rows} cols={cols} batch_index={batch_index}"
            );
            assert_eq!(
                scores[batch_index].to_bits(),
                expected_score.to_bits(),
                "batch={batch} rows={rows} cols={cols} batch_index={batch_index}"
            );
        }
    }
}

