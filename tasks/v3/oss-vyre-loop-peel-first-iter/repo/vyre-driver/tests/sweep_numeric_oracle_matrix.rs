//! Handwritten oracle matrix for backend-neutral numeric boundary helpers.
//!
//! Compares `numeric.rs` ratio, scaling, alignment, and launch-dimension helpers
//! against independent widened-arithmetic oracles across hostile seeds.

#![forbid(unsafe_code)]

use vyre_driver::numeric::{
    align_up_u64, align_up_usize, checked_ceil_div_u64, checked_compose_basis_points_u64,
    checked_dim_product_u32, checked_dim_product_u64, compose_basis_points_u32,
    ratio_basis_points_u64, ratio_basis_points_u64_wide, ratio_parts_per_million_u64,
    scale_u64_by_basis_points_floor_min, scale_u64_by_basis_points_round_clamped,
    BASIS_POINTS_DENOMINATOR,
};

const NUMERIC_CASES: u32 = 512;
const DIM_VALUES: [u32; 9] = [0, 1, 2, 3, 7, 32, 255, 65_535, u32::MAX];

#[test]
fn numeric_ratio_and_scaling_oracle_matrix_matches_independent_reference() {
    let mut assertions = 0usize;
    for seed in 0..NUMERIC_CASES {
        let part = hostile_u64(seed);
        let whole = hostile_u64(seed ^ 0xA5A5_A5A5).max(1);
        let left_bps = hostile_u32(seed ^ 0x11);
        let right_bps = hostile_u32(seed ^ 0x22);
        let base = hostile_u64(seed ^ 0x33);
        let scale_bps = hostile_u32(seed ^ 0x44);
        let zero_denom = hostile_u32(seed ^ 0x55);

        assert_eq!(
            ratio_basis_points_u64_wide(part, whole, u64::from(zero_denom), "wide", "oracle"),
            oracle_ratio_basis_points_u64_wide(part, whole, u64::from(zero_denom)),
            "Fix: ratio_basis_points_u64_wide seed={seed} must match the independent oracle."
        );
        assertions += 1;

        assert_eq!(
            ratio_basis_points_u64(part, whole, zero_denom, "narrow", "oracle"),
            oracle_ratio_basis_points_u64(part, whole, zero_denom),
            "Fix: ratio_basis_points_u64 seed={seed} must match the independent oracle."
        );
        assertions += 1;

        assert_eq!(
            ratio_parts_per_million_u64(part, whole, zero_denom, "ppm", "oracle"),
            oracle_ratio_parts_per_million_u64(part, whole, zero_denom),
            "Fix: ratio_parts_per_million_u64 seed={seed} must match the independent oracle."
        );
        assertions += 1;

        assert_eq!(
            compose_basis_points_u32(left_bps, right_bps, "compose", "oracle"),
            oracle_compose_basis_points_u32(left_bps, right_bps),
            "Fix: compose_basis_points_u32 seed={seed} must match the independent oracle."
        );
        assertions += 1;

        assert_eq!(
            checked_compose_basis_points_u64(u64::from(left_bps), u64::from(right_bps)),
            oracle_checked_compose_basis_points_u64(u64::from(left_bps), u64::from(right_bps)),
            "Fix: checked_compose_basis_points_u64 seed={seed} must match the independent oracle."
        );
        assertions += 1;

        assert_eq!(
            scale_u64_by_basis_points_round_clamped(base, scale_bps, base, 40_000, "round", "oracle"),
            oracle_scale_u64_by_basis_points_round_clamped(base, scale_bps, base, 40_000),
            "Fix: scale_u64_by_basis_points_round_clamped seed={seed} must match the independent oracle."
        );
        assertions += 1;

        assert_eq!(
            scale_u64_by_basis_points_floor_min(base, scale_bps.max(1), 1, "floor", "oracle"),
            oracle_scale_u64_by_basis_points_floor_min(base, scale_bps.max(1), 1),
            "Fix: scale_u64_by_basis_points_floor_min seed={seed} must match the independent oracle."
        );
        assertions += 1;

        let divisor = hostile_u64(seed ^ 0x66).max(1);
        assert_eq!(
            checked_ceil_div_u64(part, divisor),
            oracle_checked_ceil_div_u64(part, divisor),
            "Fix: checked_ceil_div_u64 seed={seed} must match the independent oracle."
        );
        assertions += 1;
    }
    assert_eq!(assertions, NUMERIC_CASES as usize * 8);
}

#[test]
fn numeric_alignment_oracle_matrix_matches_independent_reference() {
    let mut assertions = 0usize;
    for seed in 0..NUMERIC_CASES {
        let value = hostile_u64(seed);
        let alignment = 1_u64 << ((seed % 6) + 1);
        let min_value = value % alignment;

        assert_eq!(
            align_up_u64(value, alignment, min_value, "copy", "oracle").ok(),
            oracle_align_up_u64(value, alignment, min_value),
            "Fix: align_up_u64 seed={seed} must match the independent oracle."
        );
        assertions += 1;

        let value_usize = value as usize;
        let alignment_usize = alignment as usize;
        let min_usize = min_value as usize;
        assert_eq!(
            align_up_usize(value_usize, alignment_usize, min_usize, "copy", "oracle").ok(),
            oracle_align_up_usize(value_usize, alignment_usize, min_usize),
            "Fix: align_up_usize seed={seed} must match the independent oracle."
        );
        assertions += 1;
    }
    assert_eq!(assertions, NUMERIC_CASES as usize * 2);
}

#[test]
fn numeric_dim_product_oracle_matrix_matches_wide_integer_reference() {
    let mut assertions = 0usize;
    for x in DIM_VALUES {
        for y in DIM_VALUES {
            for z in DIM_VALUES {
                let wide = u128::from(x) * u128::from(y) * u128::from(z);
                let expected_u64 = u64::try_from(wide).ok();
                let expected_u32 = u32::try_from(wide).ok();
                assert_eq!(
                    checked_dim_product_u64([x, y, z]),
                    expected_u64,
                    "Fix: checked_dim_product_u64 [{x}, {y}, {z}] must match the wide oracle."
                );
                assertions += 1;
                assert_eq!(
                    checked_dim_product_u32([x, y, z]),
                    expected_u32,
                    "Fix: checked_dim_product_u32 [{x}, {y}, {z}] must match the wide oracle."
                );
                assertions += 1;
            }
        }
    }
    assert_eq!(assertions, DIM_VALUES.len().pow(3) * 2);
}

fn oracle_ratio_basis_points_u64_wide(part: u64, whole: u64, denominator_zero_value: u64) -> u64 {
    if whole == 0 {
        return denominator_zero_value;
    }
    let value = (u128::from(part) * u128::from(BASIS_POINTS_DENOMINATOR)) / u128::from(whole);
    if value > u128::from(u64::MAX) {
        return u64::MAX;
    }
    value as u64
}

fn oracle_ratio_basis_points_u64(part: u64, whole: u64, denominator_zero_value: u32) -> u32 {
    let wide = oracle_ratio_basis_points_u64_wide(part, whole, u64::from(denominator_zero_value));
    if wide > u64::from(u32::MAX) {
        return u32::MAX;
    }
    wide as u32
}

fn oracle_ratio_parts_per_million_u64(part: u64, whole: u64, denominator_zero_value: u32) -> u32 {
    if whole == 0 {
        return denominator_zero_value;
    }
    let value = (u128::from(part) * 1_000_000) / u128::from(whole);
    if value > u128::from(u32::MAX) {
        return u32::MAX;
    }
    value as u32
}

fn oracle_compose_basis_points_u32(left: u32, right: u32) -> u32 {
    let value = (u128::from(left) * u128::from(right)) / u128::from(BASIS_POINTS_DENOMINATOR);
    if value > u128::from(u32::MAX) {
        return u32::MAX;
    }
    value as u32
}

fn oracle_checked_compose_basis_points_u64(left: u64, right: u64) -> Option<u64> {
    let value = (u128::from(left) * u128::from(right)) / u128::from(BASIS_POINTS_DENOMINATOR);
    u64::try_from(value).ok()
}

fn oracle_scale_u64_by_basis_points_round_clamped(
    base: u64,
    scale_bps: u32,
    zero_scale_value: u64,
    max_scale_bps: u32,
) -> u64 {
    if scale_bps == 0 {
        return zero_scale_value;
    }
    let clamped = if max_scale_bps == 0 {
        scale_bps
    } else {
        scale_bps.min(max_scale_bps)
    };
    let value = (u128::from(base) * u128::from(clamped) + u128::from(BASIS_POINTS_DENOMINATOR / 2))
        / u128::from(BASIS_POINTS_DENOMINATOR);
    if value > u128::from(u64::MAX) {
        return u64::MAX;
    }
    value as u64
}

fn oracle_scale_u64_by_basis_points_floor_min(base: u64, scale_bps: u32, min_value: u64) -> u64 {
    let value = (u128::from(base) * u128::from(scale_bps)) / u128::from(BASIS_POINTS_DENOMINATOR);
    if value > u128::from(u64::MAX) {
        return u64::MAX;
    }
    (value as u64).max(min_value)
}

fn oracle_checked_ceil_div_u64(value: u64, divisor: u64) -> Option<u64> {
    if divisor == 0 {
        return None;
    }
    if value == 0 {
        return Some(0);
    }
    ((value - 1) / divisor).checked_add(1)
}

fn oracle_align_up_u64(value: u64, alignment: u64, min_value: u64) -> Option<u64> {
    if alignment == 0 {
        return None;
    }
    let normalized = value.max(min_value);
    let remainder = normalized % alignment;
    if remainder == 0 {
        return Some(normalized);
    }
    normalized.checked_add(alignment - remainder)
}

fn oracle_align_up_usize(value: usize, alignment: usize, min_value: usize) -> Option<usize> {
    if alignment == 0 {
        return None;
    }
    let normalized = value.max(min_value);
    let remainder = normalized % alignment;
    if remainder == 0 {
        return Some(normalized);
    }
    normalized.checked_add(alignment - remainder)
}

fn hostile_u64(seed: u32) -> u64 {
    (seed as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .rotate_left((seed & 31) as u32)
        ^ u64::from(seed.rotate_left(13))
}

fn hostile_u32(seed: u32) -> u32 {
    seed.wrapping_mul(0x85EB_CA6B)
        .rotate_right((seed & 15) as u32)
        ^ seed.rotate_left(7)
}
