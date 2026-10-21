use super::*;

#[test]
fn generated_repeated_resident_u32_sequence_matches_reference_on_live_cuda() {
    let backend = CudaBackendRegistration::new(live_backend());
    let prefix = repeated_u32_prefix_program();
    let repeated = repeated_u32_step_program();
    let input = u32_bytes(&generated_u32_values(0xfeed_beef));
    let repeat_count = 5;
    let expected = repeated_reference_outputs(&prefix, &repeated, &input, repeat_count, "u32");
    let actual = dispatch_repeated_in_place_sequence(
        &backend,
        &prefix,
        &repeated,
        &input,
        repeat_count,
        DataType::U32,
        "repeated_resident_u32_sequence",
    );
    let checked = assert_u32_output_lanes(
        "repeated_resident_u32_sequence",
        LANE_COUNT,
        std::slice::from_ref(&actual),
        &expected,
    );
    assert_eq!(
        checked, LANE_COUNT,
        "Fix: repeated resident u32 sequence must compare every output lane."
    );
}

#[test]
fn generated_repeated_resident_bool_sequence_matches_reference_on_live_cuda() {
    let backend = CudaBackendRegistration::new(live_backend());
    let prefix = repeated_bool_prefix_program();
    let repeated = repeated_bool_step_program();
    let input = bool_bytes(&generated_bool_values(0xdec0_ded1));
    let repeat_count = 7;
    let expected = repeated_reference_outputs(&prefix, &repeated, &input, repeat_count, "bool");
    let actual = dispatch_repeated_in_place_sequence(
        &backend,
        &prefix,
        &repeated,
        &input,
        repeat_count,
        DataType::Bool,
        "repeated_resident_bool_sequence",
    );
    let checked = assert_u32_output_lanes(
        "repeated_resident_bool_sequence",
        LANE_COUNT,
        std::slice::from_ref(&actual),
        &expected,
    );
    assert_eq!(
        checked, LANE_COUNT,
        "Fix: repeated resident Bool sequence must compare every output lane."
    );
}

#[test]
fn generated_repeated_resident_f32_sequence_matches_reference_on_live_cuda() {
    let backend = CudaBackendRegistration::new(live_backend());
    let prefix = repeated_f32_prefix_program();
    let repeated = repeated_f32_step_program();
    let input = f32_bytes(&generated_f32_values(0xabcdef01));
    let repeat_count = 4;
    let expected = repeated_reference_outputs(&prefix, &repeated, &input, repeat_count, "f32");
    let actual = dispatch_repeated_in_place_sequence(
        &backend,
        &prefix,
        &repeated,
        &input,
        repeat_count,
        DataType::F32,
        "repeated_resident_f32_sequence",
    );
    let checked = assert_f32_output_lanes(
        "repeated_resident_f32_sequence",
        LANE_COUNT,
        MAX_F32_ULP,
        std::slice::from_ref(&actual),
        &expected,
    );
    assert_eq!(
        checked, LANE_COUNT,
        "Fix: repeated resident f32 sequence must compare every output lane."
    );
}

#[test]
fn generated_repeated_resident_u32_sequence_compact_multi_range_readback_matches_reference_on_live_cuda(
) {
    let backend = CudaBackendRegistration::new(live_backend());
    let prefix = repeated_u32_prefix_program();
    let repeated = repeated_u32_step_program();
    let input = u32_bytes(&generated_u32_values(0x51f7_beef));
    let repeat_count = 6;
    let expected = repeated_reference_outputs(
        &prefix,
        &repeated,
        &input,
        repeat_count,
        "u32_compact_multi_range",
    );
    let ranges = compact_word_ranges();
    let actual = dispatch_repeated_in_place_sequence_read_ranges(
        &backend,
        &prefix,
        &repeated,
        &input,
        repeat_count,
        &ranges,
        DataType::U32,
        "repeated_resident_u32_sequence_compact_multi_range",
    );
    assert_compact_ranges_match(
        "repeated_resident_u32_sequence_compact_multi_range",
        &actual,
        &expected[0],
        &ranges,
    );
}

#[test]
fn generated_repeated_resident_bool_sequence_compact_multi_range_readback_matches_reference_on_live_cuda(
) {
    let backend = CudaBackendRegistration::new(live_backend());
    let prefix = repeated_bool_prefix_program();
    let repeated = repeated_bool_step_program();
    let input = bool_bytes(&generated_bool_values(0xb001_b1a5));
    let repeat_count = 8;
    let expected = repeated_reference_outputs(
        &prefix,
        &repeated,
        &input,
        repeat_count,
        "bool_compact_multi_range",
    );
    let ranges = compact_word_ranges();
    let actual = dispatch_repeated_in_place_sequence_read_ranges(
        &backend,
        &prefix,
        &repeated,
        &input,
        repeat_count,
        &ranges,
        DataType::Bool,
        "repeated_resident_bool_sequence_compact_multi_range",
    );
    assert_compact_ranges_match(
        "repeated_resident_bool_sequence_compact_multi_range",
        &actual,
        &expected[0],
        &ranges,
    );
}

#[test]
fn generated_repeated_resident_bool_sequence_overlapping_multi_range_readback_matches_reference_on_live_cuda(
) {
    let backend = CudaBackendRegistration::new(live_backend());
    let prefix = repeated_bool_prefix_program();
    let repeated = repeated_bool_step_program();
    let input = bool_bytes(&generated_bool_values(0x0b00_1f15));
    let repeat_count = 9;
    let expected = repeated_reference_outputs(
        &prefix,
        &repeated,
        &input,
        repeat_count,
        "bool_overlapping_multi_range",
    );
    let ranges = overlapping_word_ranges();
    let actual = dispatch_repeated_in_place_sequence_read_ranges(
        &backend,
        &prefix,
        &repeated,
        &input,
        repeat_count,
        &ranges,
        DataType::Bool,
        "repeated_resident_bool_sequence_overlapping_multi_range",
    );
    assert_compact_ranges_match(
        "repeated_resident_bool_sequence_overlapping_multi_range",
        &actual,
        &expected[0],
        &ranges,
    );
}

