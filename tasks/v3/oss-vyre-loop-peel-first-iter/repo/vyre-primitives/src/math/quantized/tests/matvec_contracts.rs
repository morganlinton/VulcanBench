use super::*;

#[test]
fn matvec_cpu_matches_dequantized_reference() {
    let weights = vec![
        vec![-8, -4, -1, 0, 1, 2, 6, 7, 5],
        vec![7, 5, 3, 1, -1, -3, -5, -7, 6],
        vec![0, 1, 0, -1, 2, -2, 3, -3, 4],
    ];
    let x = [0.5_f32, -1.0, 2.0, 0.25, -0.5, 1.5, -2.0, 3.0, 0.75];
    let scales = [0.125_f32, 0.25, 0.5];
    let packed = pack_i4_matrix_rows(&weights);
    let actual = i4x8_matvec_f32_scaled_cpu(&packed, &x, &scales, 3, 9);
    let expected = weights
        .iter()
        .zip(scales)
        .map(|(row, scale)| {
            row.iter()
                .zip(x)
                .fold(0.0_f32, |acc, (&w, x)| acc + w as f32 * x)
                * scale
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
}

#[test]
fn generated_matvec_matches_dequantized_reference_across_pack_boundaries() {
    let pattern = [-8, -3, -1, 0, 1, 3, 7, 6, 5, 4, 2, -2, -4, -6, -7, -5];
    for rows in 1..=8_u32 {
        for cols in [1_u32, 7, 8, 9, 16, 17, 31, 32, 33, 65] {
            let weights = (0..rows as usize)
                .map(|row| {
                    pattern
                        .iter()
                        .copied()
                        .cycle()
                        .skip(row)
                        .take(cols as usize)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let x = (0..cols)
                .map(|col| (col % 11) as f32 * 0.125 - 0.5)
                .collect::<Vec<_>>();
            let scales = (0..rows)
                .map(|row| 0.125_f32 + row as f32 * 0.0625)
                .collect::<Vec<_>>();
            let packed = pack_i4_matrix_rows(&weights);
            let actual = i4x8_matvec_f32_scaled_cpu(&packed, &x, &scales, rows, cols);
            let expected = weights
                .iter()
                .zip(scales.iter().copied())
                .map(|(row, scale)| {
                    row.iter()
                        .zip(x.iter().copied())
                        .fold(0.0_f32, |acc, (&w, x)| acc + w as f32 * x)
                        * scale
                })
                .collect::<Vec<_>>();

            assert_eq!(
                actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "rows={rows} cols={cols}"
            );
        }
    }
}

#[test]
fn batched_matvec_cpu_matches_repeated_matvec_reference() {
    let weights = vec![
        vec![1, 2, 3, 4, -1, -2, -3, -4, 5],
        vec![4, 3, 2, 1, -4, -3, -2, -1, -5],
        vec![-8, -7, -6, -5, -4, -3, -2, -1, 0],
    ];
    let x_batches = [
        1.0_f32, -0.5, 0.25, 2.0, -1.5, 0.75, 1.25, -2.0, 0.5, -1.0, 0.5, -0.25, -2.0, 1.5, -0.75,
        -1.25, 2.0, -0.5,
    ];
    let scales = [0.5_f32, 0.25, 0.125];
    let packed = pack_i4_matrix_rows(&weights);
    let actual = i4x8_batched_matvec_f32_scaled_cpu(&packed, &x_batches, &scales, 2, 3, 9);
    let mut expected = Vec::new();
    for x in x_batches.chunks_exact(9) {
        expected.extend(i4x8_matvec_f32_scaled_cpu(&packed, x, &scales, 3, 9));
    }

    assert_eq!(
        actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
}

#[test]
fn generated_batched_matvec_matches_repeated_matvec_across_pack_boundaries() {
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
                            .skip(row * 2)
                            .take(cols as usize)
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let x_batches = (0..batch * cols)
                    .map(|index| (index % 13) as f32 * 0.125 - 0.75)
                    .collect::<Vec<_>>();
                let scales = (0..rows)
                    .map(|row| 0.125_f32 + row as f32 * 0.0625)
                    .collect::<Vec<_>>();
                let packed = pack_i4_matrix_rows(&weights);
                let actual = i4x8_batched_matvec_f32_scaled_cpu(
                    &packed, &x_batches, &scales, batch, rows, cols,
                );
                let mut expected = Vec::new();
                for x in x_batches.chunks_exact(cols as usize) {
                    expected.extend(i4x8_matvec_f32_scaled_cpu(&packed, x, &scales, rows, cols));
                }

                assert_eq!(
                    actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    "batch={batch} rows={rows} cols={cols}"
                );
            }
        }
    }
}

