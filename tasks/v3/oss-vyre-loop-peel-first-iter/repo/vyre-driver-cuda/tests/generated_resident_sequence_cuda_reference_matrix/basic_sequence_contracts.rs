use super::*;

#[test]
fn generated_resident_u32_sequence_matches_reference_on_live_cuda() {
    let backend = CudaBackendRegistration::new(live_backend());
    let input = generated_u32_values(0x1020_3040);
    let input_bytes = u32_bytes(&input);
    let first = u32_sequence_first_program();
    let second = u32_sequence_second_program();
    let expected_tmp = reference_outputs(
        &first,
        std::slice::from_ref(&input_bytes),
        "resident_u32_sequence_first",
    );
    let expected = reference_outputs(&second, &expected_tmp, "resident_u32_sequence_second");
    let actual = dispatch_two_step_sequence(
        &backend,
        &first,
        &second,
        &input_bytes,
        DataType::U32,
        "resident_u32_sequence",
    );
    let checked = assert_u32_output_lanes(
        "resident_u32_sequence",
        LANE_COUNT,
        std::slice::from_ref(&actual),
        &expected,
    );
    assert_eq!(
        checked, LANE_COUNT,
        "Fix: resident u32 sequence matrix must compare every output lane."
    );
}

#[test]
fn generated_resident_bool_sequence_matches_reference_on_live_cuda() {
    let backend = CudaBackendRegistration::new(live_backend());
    let input = generated_bool_values(0x3141_5926);
    let input_bytes = bool_bytes(&input);
    let first = bool_sequence_first_program();
    let second = bool_sequence_second_program();
    let expected_tmp = reference_outputs(
        &first,
        std::slice::from_ref(&input_bytes),
        "resident_bool_sequence_first",
    );
    let expected = reference_outputs(&second, &expected_tmp, "resident_bool_sequence_second");
    let actual = dispatch_two_step_sequence(
        &backend,
        &first,
        &second,
        &input_bytes,
        DataType::Bool,
        "resident_bool_sequence",
    );
    let checked = assert_u32_output_lanes(
        "resident_bool_sequence",
        LANE_COUNT,
        std::slice::from_ref(&actual),
        &expected,
    );
    assert_eq!(
        checked, LANE_COUNT,
        "Fix: resident Bool sequence matrix must compare every output lane."
    );
}

#[test]
fn generated_resident_f32_sequence_matches_reference_on_live_cuda() {
    let backend = CudaBackendRegistration::new(live_backend());
    let input = generated_f32_values(0x2718_2818);
    let input_bytes = f32_bytes(&input);
    let first = f32_sequence_first_program();
    let second = f32_sequence_second_program();
    let expected_tmp = reference_outputs(
        &first,
        std::slice::from_ref(&input_bytes),
        "resident_f32_sequence_first",
    );
    let expected = reference_outputs(&second, &expected_tmp, "resident_f32_sequence_second");
    let actual = dispatch_two_step_sequence(
        &backend,
        &first,
        &second,
        &input_bytes,
        DataType::F32,
        "resident_f32_sequence",
    );
    let checked = assert_f32_output_lanes(
        "resident_f32_sequence",
        LANE_COUNT,
        MAX_F32_ULP,
        std::slice::from_ref(&actual),
        &expected,
    );
    assert_eq!(
        checked, LANE_COUNT,
        "Fix: resident f32 sequence matrix must compare every output lane."
    );
}

#[test]
fn generated_resident_u32_sequence_compact_multi_range_readback_matches_reference_on_live_cuda() {
    let backend = CudaBackendRegistration::new(live_backend());
    let input = generated_u32_values(0x0bad_c0de);
    let input_bytes = u32_bytes(&input);
    let first = u32_sequence_first_program();
    let second = u32_sequence_second_program();
    let expected_tmp = reference_outputs(
        &first,
        std::slice::from_ref(&input_bytes),
        "resident_u32_sequence_compact_first",
    );
    let expected = reference_outputs(
        &second,
        &expected_tmp,
        "resident_u32_sequence_compact_second",
    );
    let ranges = compact_word_ranges();
    let actual = dispatch_two_step_sequence_read_ranges(
        &backend,
        &first,
        &second,
        &input_bytes,
        &ranges,
        DataType::U32,
        "resident_u32_sequence_compact_multi_range",
    );
    assert_compact_ranges_match(
        "resident_u32_sequence_compact_multi_range",
        &actual,
        &expected[0],
        &ranges,
    );
}

#[test]
fn generated_resident_bool_sequence_compact_multi_range_readback_matches_reference_on_live_cuda() {
    let backend = CudaBackendRegistration::new(live_backend());
    let input = generated_bool_values(0x5afe_b001);
    let input_bytes = bool_bytes(&input);
    let first = bool_sequence_first_program();
    let second = bool_sequence_second_program();
    let expected_tmp = reference_outputs(
        &first,
        std::slice::from_ref(&input_bytes),
        "resident_bool_sequence_compact_first",
    );
    let expected = reference_outputs(
        &second,
        &expected_tmp,
        "resident_bool_sequence_compact_second",
    );
    let ranges = compact_word_ranges();
    let actual = dispatch_two_step_sequence_read_ranges(
        &backend,
        &first,
        &second,
        &input_bytes,
        &ranges,
        DataType::Bool,
        "resident_bool_sequence_compact_multi_range",
    );
    assert_compact_ranges_match(
        "resident_bool_sequence_compact_multi_range",
        &actual,
        &expected[0],
        &ranges,
    );
}

#[test]
fn generated_resident_u32_sequence_overlapping_multi_range_readback_matches_reference_on_live_cuda()
{
    let backend = CudaBackendRegistration::new(live_backend());
    let input = generated_u32_values(0xf005_ba11);
    let input_bytes = u32_bytes(&input);
    let first = u32_sequence_first_program();
    let second = u32_sequence_second_program();
    let expected_tmp = reference_outputs(
        &first,
        std::slice::from_ref(&input_bytes),
        "resident_u32_sequence_overlap_first",
    );
    let expected = reference_outputs(
        &second,
        &expected_tmp,
        "resident_u32_sequence_overlap_second",
    );
    let ranges = overlapping_word_ranges();
    let actual = dispatch_two_step_sequence_read_ranges(
        &backend,
        &first,
        &second,
        &input_bytes,
        &ranges,
        DataType::U32,
        "resident_u32_sequence_overlapping_multi_range",
    );
    assert_compact_ranges_match(
        "resident_u32_sequence_overlapping_multi_range",
        &actual,
        &expected[0],
        &ranges,
    );
}

