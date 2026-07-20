use super::*;

#[test]
fn dot_cpu_matches_unpacked_reference() {
    let lhs = [-8, -4, -1, 0, 1, 2, 6, 7, 5, -7, 3, -3];
    let rhs = [7, -2, -1, 4, -8, 6, 2, 1, -5, 3, -4, 2];
    let lhs_packed = pack_i4x8_cpu(&lhs);
    let rhs_packed = pack_i4x8_cpu(&rhs);
    let expected = lhs
        .iter()
        .zip(rhs.iter())
        .fold(0i32, |acc, (&lhs, &rhs)| acc + lhs * rhs);

    assert_eq!(
        i4x8_dot_i32_cpu(&lhs_packed, &rhs_packed, lhs.len() as u32),
        expected
    );
}

#[test]
fn dot_cpu_missing_words_contribute_zero_lanes() {
    let lhs = pack_i4x8_cpu(&[7, -8, 3, -2]);

    assert_eq!(i4x8_dot_i32_cpu(&lhs, &[], 4), 0);
}

#[test]
fn generated_dot_matches_unpack_then_dot_for_all_offsets() {
    let lhs_pattern = [-8, -3, -1, 0, 1, 3, 7, 6, 5, 4, 2, -2, -4, -6, -7, -5];
    let rhs_pattern = [7, 5, 3, 1, -1, -3, -5, -7, 6, 4, 2, 0, -2, -4, -6, -8];
    for len in 0..=256 {
        let lhs = lhs_pattern
            .iter()
            .copied()
            .cycle()
            .take(len)
            .collect::<Vec<_>>();
        let rhs = rhs_pattern
            .iter()
            .copied()
            .cycle()
            .take(len)
            .collect::<Vec<_>>();
        let lhs_packed = pack_i4x8_cpu(&lhs);
        let rhs_packed = pack_i4x8_cpu(&rhs);
        let unpacked_lhs = unpack_i4x8_cpu(&lhs_packed, len as u32);
        let unpacked_rhs = unpack_i4x8_cpu(&rhs_packed, len as u32);
        let expected = unpacked_lhs
            .iter()
            .zip(unpacked_rhs.iter())
            .fold(0i32, |acc, (&lhs, &rhs)| {
                acc.wrapping_add(lhs.wrapping_mul(rhs))
            });

        assert_eq!(
            i4x8_dot_i32_cpu(&lhs_packed, &rhs_packed, len as u32),
            expected,
            "len={len}"
        );
    }
}

#[test]
fn scaled_dot_cpu_matches_dequantized_reference() {
    let lhs = [-8, -4, -1, 0, 1, 2, 6, 7, 5, -7, 3, -3];
    let rhs = [7, -2, -1, 4, -8, 6, 2, 1, -5, 3, -4, 2];
    let lhs_scale = 0.25_f32;
    let rhs_scale = 0.5_f32;
    let lhs_packed = pack_i4x8_cpu(&lhs);
    let rhs_packed = pack_i4x8_cpu(&rhs);
    let expected = lhs
        .iter()
        .zip(rhs.iter())
        .fold(0.0_f32, |acc, (&lhs, &rhs)| {
            acc + (lhs as f32 * lhs_scale) * (rhs as f32 * rhs_scale)
        });
    let actual = i4x8_dot_f32_scaled_cpu(
        &lhs_packed,
        &rhs_packed,
        lhs_scale,
        rhs_scale,
        lhs.len() as u32,
    );

    assert!(
        (actual - expected).abs() <= 0.000_001,
        "actual={actual} expected={expected}"
    );
}

#[test]
fn generated_scaled_dot_matches_i32_dot_scale_product() {
    let lhs_pattern = [-8, -3, -1, 0, 1, 3, 7, 6, 5, 4, 2, -2, -4, -6, -7, -5];
    let rhs_pattern = [7, 5, 3, 1, -1, -3, -5, -7, 6, 4, 2, 0, -2, -4, -6, -8];
    for len in 0..=256 {
        let lhs = lhs_pattern
            .iter()
            .copied()
            .cycle()
            .take(len)
            .collect::<Vec<_>>();
        let rhs = rhs_pattern
            .iter()
            .copied()
            .cycle()
            .take(len)
            .collect::<Vec<_>>();
        let lhs_packed = pack_i4x8_cpu(&lhs);
        let rhs_packed = pack_i4x8_cpu(&rhs);
        let lhs_scale = 0.125_f32 + (len % 7) as f32 * 0.03125;
        let rhs_scale = 0.25_f32 + (len % 5) as f32 * 0.0625;
        let expected =
            i4x8_dot_i32_cpu(&lhs_packed, &rhs_packed, len as u32) as f32 * lhs_scale * rhs_scale;

        assert_eq!(
            i4x8_dot_f32_scaled_cpu(&lhs_packed, &rhs_packed, lhs_scale, rhs_scale, len as u32)
                .to_bits(),
            expected.to_bits(),
            "len={len}"
        );
    }
}

