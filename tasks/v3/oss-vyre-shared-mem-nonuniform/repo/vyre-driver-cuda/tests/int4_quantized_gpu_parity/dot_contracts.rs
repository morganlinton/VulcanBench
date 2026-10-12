use super::*;

#[test]
fn cuda_dispatch_matches_packed_int4_dot_i32_oracle() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let lhs_pattern = [-8, -3, -1, 0, 1, 3, 7, 6, 5, 4, 2, -2, -4, -6, -7, -5];
    let rhs_pattern = [7, 5, 3, 1, -1, -3, -5, -7, 6, 4, 2, 0, -2, -4, -6, -8];

    for lane_count in [1_u32, 7, 8, 9, 16, 31, 32, 33, 65] {
        let lhs = lhs_pattern
            .iter()
            .copied()
            .cycle()
            .take(lane_count as usize)
            .collect::<Vec<_>>();
        let rhs = rhs_pattern
            .iter()
            .copied()
            .cycle()
            .take(lane_count as usize)
            .collect::<Vec<_>>();
        let lhs_packed = pack_i4x8(&lhs);
        let rhs_packed = pack_i4x8(&rhs);
        let program =
            vyre_primitives::math::quantized::i4x8_dot_i32("lhs", "rhs", "out", lane_count);
        let outputs = backend
            .dispatch(
                &program,
                &[pack_u32(&lhs_packed), pack_u32(&rhs_packed)],
                &DispatchConfig::default(),
            )
            .expect("Fix: CUDA must execute packed INT4 dot without CPU fallback.");
        let expected = dot_i32_oracle(&lhs_packed, &rhs_packed, lane_count);
        let actual = read_i32(&outputs[0]);

        assert_eq!(actual, expected, "lane_count={lane_count}");
    }
}

#[test]
fn cuda_dispatch_matches_packed_int4_scaled_dot_oracle() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let lhs_pattern = [-8, -3, -1, 0, 1, 3, 7, 6, 5, 4, 2, -2, -4, -6, -7, -5];
    let rhs_pattern = [7, 5, 3, 1, -1, -3, -5, -7, 6, 4, 2, 0, -2, -4, -6, -8];

    for lane_count in [1_u32, 7, 8, 9, 16, 31, 32, 33, 65] {
        let lhs = lhs_pattern
            .iter()
            .copied()
            .cycle()
            .take(lane_count as usize)
            .collect::<Vec<_>>();
        let rhs = rhs_pattern
            .iter()
            .copied()
            .cycle()
            .take(lane_count as usize)
            .collect::<Vec<_>>();
        let lhs_packed = pack_i4x8(&lhs);
        let rhs_packed = pack_i4x8(&rhs);
        let lhs_scale = 0.125_f32 + (lane_count % 4) as f32 * 0.0625;
        let rhs_scale = 0.25_f32 + (lane_count % 3) as f32 * 0.125;
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
            .expect("Fix: CUDA must execute fused packed INT4 scaled dot without CPU fallback.");
        let expected =
            dot_scaled_oracle(&lhs_packed, &rhs_packed, lhs_scale, rhs_scale, lane_count);
        let actual = read_f32(&outputs[0]);

        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "lane_count={lane_count}"
        );
    }
}

