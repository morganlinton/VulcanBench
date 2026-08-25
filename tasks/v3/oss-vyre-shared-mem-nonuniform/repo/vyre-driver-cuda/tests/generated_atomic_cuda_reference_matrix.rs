//! Generated live CUDA/reference differential matrix for atomic memory semantics.

mod common;
#[path = "common/generated_atomic_matrix.rs"]
mod generated_atomic_matrix;

use common::{
    assert_u32_output_lanes, cuda_reference_outputs, live_backend, u32_bytes,
    GENERATED_LANE_COUNT as LANE_COUNT,
};
use generated_atomic_matrix::{
    assert_two_u32_output_buffers, atomic_compare_exchange_return_value_program,
    atomic_compare_exchange_single_writer_program, atomic_exchange_single_writer_program,
    atomic_reduction_program, atomic_return_value_program, generated_atomic_values,
    generated_exchange_initial_values, generated_old_sentinel_values, ATOMIC_REDUCTION_CASES,
    ATOMIC_RETURN_CASES, BUCKET_COUNT,
};

#[test]
fn generated_atomic_reduction_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let mut checked_output_words = 0usize;

    for case in ATOMIC_REDUCTION_CASES {
        let program = atomic_reduction_program(case);
        let initial = vec![case.identity; LANE_COUNT];
        let values = generated_atomic_values(case.value_salt);
        let inputs = vec![u32_bytes(&initial), u32_bytes(&values)];
        let outputs = cuda_reference_outputs(&backend, &program, &inputs, case.name);
        checked_output_words += assert_u32_output_lanes(
            case.name,
            LANE_COUNT,
            &outputs.direct_cuda,
            &outputs.reference,
        );
        checked_output_words += assert_u32_output_lanes(
            case.name,
            LANE_COUNT,
            &outputs.compiled_cuda,
            &outputs.reference,
        );
    }

    assert_eq!(
        checked_output_words,
        ATOMIC_REDUCTION_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA atomic reduction matrix must compare every output lane across direct and compiled paths."
    );
}

#[test]
fn generated_atomic_exchange_single_writer_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let program = atomic_exchange_single_writer_program();
    let initial = generated_exchange_initial_values();
    let values = generated_atomic_values(0xabcdef01);
    let inputs = vec![u32_bytes(&initial), u32_bytes(&values)];
    let outputs =
        cuda_reference_outputs(&backend, &program, &inputs, "atomic_exchange_single_writer");
    let checked_output_words = assert_u32_output_lanes(
        "atomic_exchange_single_writer",
        BUCKET_COUNT,
        &outputs.direct_cuda,
        &outputs.reference,
    ) + assert_u32_output_lanes(
        "atomic_exchange_single_writer",
        BUCKET_COUNT,
        &outputs.compiled_cuda,
        &outputs.reference,
    );

    assert_eq!(
        checked_output_words,
        BUCKET_COUNT * 2,
        "Fix: generated CUDA atomic exchange matrix must compare every accumulator bucket across direct and compiled paths."
    );
}

#[test]
fn generated_atomic_compare_exchange_single_writer_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let initial = generated_exchange_initial_values();
    let values = generated_atomic_values(0x0bad_f00d);
    let mut checked_output_words = 0usize;

    for expected_matches in [true, false] {
        let case_name = if expected_matches {
            "atomic_compare_exchange_single_writer_match"
        } else {
            "atomic_compare_exchange_single_writer_miss"
        };
        let program = atomic_compare_exchange_single_writer_program(expected_matches);
        let inputs = vec![u32_bytes(&initial), u32_bytes(&values)];
        let outputs = cuda_reference_outputs(&backend, &program, &inputs, case_name);
        checked_output_words += assert_u32_output_lanes(
            case_name,
            BUCKET_COUNT,
            &outputs.direct_cuda,
            &outputs.reference,
        );
        checked_output_words += assert_u32_output_lanes(
            case_name,
            BUCKET_COUNT,
            &outputs.compiled_cuda,
            &outputs.reference,
        );
    }

    assert_eq!(
        checked_output_words,
        BUCKET_COUNT * 4,
        "Fix: generated CUDA compare-exchange matrix must compare every accumulator bucket for match and miss cases across direct and compiled paths."
    );
}

#[test]
fn generated_atomic_return_value_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let initial = generated_exchange_initial_values();
    let old_sentinel = generated_old_sentinel_values();
    let mut checked_output_words = 0usize;

    for case in ATOMIC_RETURN_CASES {
        let program = atomic_return_value_program(case);
        let values = generated_atomic_values(case.value_salt);
        let inputs = vec![
            u32_bytes(&initial),
            u32_bytes(&values),
            u32_bytes(&old_sentinel),
        ];
        let outputs = cuda_reference_outputs(&backend, &program, &inputs, case.name);
        checked_output_words += assert_two_u32_output_buffers(
            case.name,
            BUCKET_COUNT,
            &outputs.direct_cuda,
            &outputs.reference,
        );
        checked_output_words += assert_two_u32_output_buffers(
            case.name,
            BUCKET_COUNT,
            &outputs.compiled_cuda,
            &outputs.reference,
        );
    }

    assert_eq!(
        checked_output_words,
        ATOMIC_RETURN_CASES.len() * BUCKET_COUNT * 4,
        "Fix: generated CUDA atomic return-value matrix must compare accumulator and returned-old-value buffers across direct and compiled paths."
    );
}

#[test]
fn generated_atomic_compare_exchange_return_value_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let initial = generated_exchange_initial_values();
    let values = generated_atomic_values(0xcafe_babe);
    let old_sentinel = generated_old_sentinel_values();
    let mut checked_output_words = 0usize;

    for expected_matches in [true, false] {
        let case_name = if expected_matches {
            "atomic_compare_exchange_return_single_writer_match"
        } else {
            "atomic_compare_exchange_return_single_writer_miss"
        };
        let program = atomic_compare_exchange_return_value_program(expected_matches);
        let inputs = vec![
            u32_bytes(&initial),
            u32_bytes(&values),
            u32_bytes(&old_sentinel),
        ];
        let outputs = cuda_reference_outputs(&backend, &program, &inputs, case_name);
        checked_output_words += assert_two_u32_output_buffers(
            case_name,
            BUCKET_COUNT,
            &outputs.direct_cuda,
            &outputs.reference,
        );
        checked_output_words += assert_two_u32_output_buffers(
            case_name,
            BUCKET_COUNT,
            &outputs.compiled_cuda,
            &outputs.reference,
        );
    }

    assert_eq!(
        checked_output_words,
        BUCKET_COUNT * 8,
        "Fix: generated CUDA compare-exchange return-value matrix must compare accumulator and returned-old-value buffers for match and miss cases across direct and compiled paths."
    );
}
