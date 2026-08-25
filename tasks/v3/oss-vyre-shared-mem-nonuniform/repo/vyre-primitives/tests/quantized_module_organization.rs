//! Organization and generated edge coverage for packed INT4 primitives.

#[test]
fn quantized_module_is_split_by_responsibility() {
    let root = include_str!("../src/math/quantized.rs");
    let cpu = include_str!("../src/math/quantized/cpu.rs");
    let programs = include_str!("../src/math/quantized/programs.rs");
    let helpers = include_str!("../src/math/quantized/program_helpers.rs");
    let tests = include_str!("../src/math/quantized/tests/mod.rs");

    assert!(
        root.contains("mod cpu;")
            && root.contains("mod program_helpers;")
            && root.contains("mod programs;")
            && root.contains("pub use cpu::{")
            && root.contains("pub use programs::{"),
        "Fix: math::quantized root must remain an API facade over focused sibling modules."
    );
    for (name, source) in [
        ("quantized.rs", root),
        ("quantized/cpu.rs", cpu),
        ("quantized/programs.rs", programs),
        ("quantized/program_helpers.rs", helpers),
        ("quantized/tests/mod.rs", tests),
    ] {
        assert!(
            source.starts_with("//!"),
            "Fix: {name} needs concrete module-level docs."
        );
        assert!(
            source.lines().count() < 900,
            "Fix: {name} should stay below the quantized responsibility ceiling."
        );
    }
}

#[cfg(all(feature = "math", feature = "cpu-parity"))]
#[test]
fn generated_i4_cpu_oracle_matrix_preserves_packing_and_top1_semantics() {
    use vyre_primitives::math::quantized::{
        i4_packed_words, i4x8_batched_matmul_f32_scaled_cpu,
        i4x8_batched_matmul_top1_f32_scaled_cpu, i4x8_dot_i32_cpu, pack_i4x8_cpu, unpack_i4x8_cpu,
        I4_LANES_PER_WORD,
    };

    let lane_cases = [0usize, 1, 2, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65];
    let pattern = [-8, -7, -3, -1, 0, 1, 2, 3, 4, 7];
    for lane_count in lane_cases {
        let lanes = pattern
            .iter()
            .copied()
            .cycle()
            .take(lane_count)
            .collect::<Vec<_>>();
        let packed = pack_i4x8_cpu(&lanes);
        assert_eq!(
            packed.len() as u32,
            i4_packed_words(lane_count as u32),
            "Fix: i4 packed word count must be ceil(lanes / {I4_LANES_PER_WORD})."
        );
        assert_eq!(
            unpack_i4x8_cpu(&packed, lane_count as u32),
            lanes,
            "Fix: packed INT4 lanes must round-trip exactly for lane_count={lane_count}."
        );
        let dot = i4x8_dot_i32_cpu(&packed, &packed, lane_count as u32);
        let expected_dot = lanes.iter().map(|lane| lane * lane).sum::<i32>();
        assert_eq!(dot, expected_dot);
    }

    let weights = vec![
        vec![7, 6, 5, 4, 3, 2, 1, 0, -1],
        vec![-8, -7, -6, -5, -4, -3, -2, -1, 0],
        vec![1, -1, 1, -1, 1, -1, 1, -1, 1],
    ];
    let activations = vec![
        vec![1, 1, 1, 1, 1, 1, 1, 1, 1],
        vec![-1, -1, -1, -1, -1, -1, -1, -1, -1],
    ];
    let weights_packed = weights
        .iter()
        .flat_map(|row| pack_i4x8_cpu(row))
        .collect::<Vec<_>>();
    let activations_packed = activations
        .iter()
        .flat_map(|row| pack_i4x8_cpu(row))
        .collect::<Vec<_>>();
    let row_scales = [0.5_f32, 0.25, 1.0];
    let batch_scales = [1.0_f32, 0.5];
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

    for batch in 0..activations.len() {
        let offset = batch * weights.len();
        let (expected_index, expected_score) = (0..weights.len())
            .map(|row| (row as u32, logits[offset + row]))
            .max_by(|(_, lhs), (_, rhs)| lhs.total_cmp(rhs))
            .expect("Fix: generated top1 fixture must have rows.");
        assert_eq!(indices[batch], expected_index);
        assert_eq!(scores[batch].to_bits(), expected_score.to_bits());
    }
}
