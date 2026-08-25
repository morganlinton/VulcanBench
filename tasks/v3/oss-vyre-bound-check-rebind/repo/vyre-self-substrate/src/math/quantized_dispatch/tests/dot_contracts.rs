use super::*;
use super::generated_contracts::generated_i4_values;

#[test]
fn i4x8_dot_f32_scaled_via_dispatches_signed_boundary_accumulators() {
    let lhs_values = [-8, -7, -1, 0, 1, 2, 6, 7];
    let rhs_values = [7, 6, 2, 1, 0, -1, -7, -8];
    let lhs = pack_i4x8_cpu(&lhs_values);
    let rhs = pack_i4x8_cpu(&rhs_values);
    let lhs_scale = 0.125;
    let rhs_scale = 0.25;

    let out = i4x8_dot_f32_scaled_via(
        &QuantizedDotDispatcher,
        &lhs,
        &rhs,
        lhs_scale,
        rhs_scale,
        lhs_values.len() as u32,
    )
    .expect("Fix: CUDA parity tests require backend dispatch; skip test if GPU unavailable, do not panic - fake dispatcher computes scaled INT4 dot");
    let expected =
        i4x8_dot_f32_scaled_cpu(&lhs, &rhs, lhs_scale, rhs_scale, lhs_values.len() as u32);

    assert_eq!(out.to_bits(), expected.to_bits());
}

#[test]
fn i4x8_dot_f32_scaled_via_reuses_cached_program_for_same_lane_shape() {
    let lhs8 = pack_i4x8_cpu(&[-8, -1, 0, 7, 3, -3, 6, -6]);
    let rhs8 = pack_i4x8_cpu(&[7, 1, -1, -8, 2, -2, 5, -5]);
    let lhs9 = pack_i4x8_cpu(&[-8, -1, 0, 7, 3, -3, 6, -6, 4]);
    let rhs9 = pack_i4x8_cpu(&[7, 1, -1, -8, 2, -2, 5, -5, 3]);
    let mut scratch = QuantizedDotGpuScratch::default();
    let mut out = Vec::with_capacity(1);

    i4x8_dot_f32_scaled_via_with_scratch_into(
        &QuantizedDotDispatcher,
        &lhs8,
        &rhs8,
        0.5,
        0.25,
        8,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - first dot shape succeeds");
    i4x8_dot_f32_scaled_via_with_scratch_into(
        &QuantizedDotDispatcher,
        &lhs8,
        &rhs8,
        0.25,
        0.5,
        8,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - same dot shape succeeds");
    assert_eq!(
        scratch.program_cache.builds(),
        1,
        "Fix: repeated same-shape INT4 dot dispatch must reuse the primitive Program."
    );

    i4x8_dot_f32_scaled_via_with_scratch_into(
        &QuantizedDotDispatcher,
        &lhs9,
        &rhs9,
        0.25,
        0.5,
        9,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - changed dot shape succeeds");
    assert_eq!(
        scratch.program_cache.builds(),
        2,
        "Fix: INT4 dot dispatch should rebuild the primitive Program only when lane_count changes."
    );
}

#[test]
fn generated_i4x8_dot_hot_warm_cache_survives_alternating_shapes() {
    let mut scratch = QuantizedDotGpuScratch::default();
    let mut out = Vec::with_capacity(1);

    for seed in 0..8192u32 {
        let lane_count = if seed % 2 == 0 { 8 } else { 16 };
        let lhs_values = generated_i4_values(lane_count as usize, seed ^ 0x9e37_79b9);
        let rhs_values = generated_i4_values(lane_count as usize, seed ^ 0x85eb_ca6b);
        let lhs = pack_i4x8_cpu(&lhs_values);
        let rhs = pack_i4x8_cpu(&rhs_values);
        let lhs_scale = 0.03125 * f32::from((seed % 7) as u8 + 1);
        let rhs_scale = 0.015625 * f32::from((seed % 5) as u8 + 1);

        i4x8_dot_f32_scaled_via_with_scratch_into(
            &QuantizedDotDispatcher,
            &lhs,
            &rhs,
            lhs_scale,
            rhs_scale,
            lane_count,
            &mut scratch,
            &mut out,
        )
        .expect("Fix: CUDA parity tests require backend dispatch; skip test if GPU unavailable, do not panic - generated alternating INT4 dot dispatch should succeed");

        let expected = i4x8_dot_f32_scaled_cpu(&lhs, &rhs, lhs_scale, rhs_scale, lane_count);
        assert_eq!(
            out[0].to_bits(),
            expected.to_bits(),
            "seed={seed} lane_count={lane_count}"
        );
    }

    assert_eq!(
            scratch.program_cache.builds(),
            2,
            "Fix: alternating two INT4 dot lane shapes must stay in the hot/warm ProgramCache instead of rebuilding every dispatch."
        );
}

#[test]

fn i4x8_dot_f32_scaled_via_rejects_bad_shape_before_dispatch() {
    let err = i4x8_dot_f32_scaled_via(&QuantizedDotDispatcher, &[0], &[0], 1.0, 1.0, 0)
        .expect_err("zero lanes must fail");
    assert!(err.to_string().contains("lane_count > 0"));

    let err = i4x8_dot_f32_scaled_via(&QuantizedDotDispatcher, &[], &[0], 1.0, 1.0, 8)
        .expect_err("missing lhs packed word must fail");
    assert!(err.to_string().contains("packed lengths"));
}

#[test]
fn i4x8_dot_f32_scaled_via_rejects_malformed_backend_outputs() {
    let lhs = pack_i4x8_cpu(&[-1, 2, 3, -4, 5, -6, 7, -8]);
    let rhs = pack_i4x8_cpu(&[7, -6, 5, -4, 3, -2, 1, 0]);
    let no_outputs = MalformedDotDispatcher { outputs: vec![] };
    let err = i4x8_dot_f32_scaled_via(&no_outputs, &lhs, &rhs, 1.0, 1.0, 8)
        .expect_err("missing output must fail");
    assert!(err.to_string().contains("exactly one output"));

    let short_output = MalformedDotDispatcher {
        outputs: vec![vec![0; 3]],
    };
    let err = i4x8_dot_f32_scaled_via(&short_output, &lhs, &rhs, 1.0, 1.0, 8)
        .expect_err("short output must fail");
    assert!(err.to_string().contains("expected 4 output bytes"));
}

