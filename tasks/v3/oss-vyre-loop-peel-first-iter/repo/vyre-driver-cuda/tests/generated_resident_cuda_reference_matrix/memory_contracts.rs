use super::*;

#[test]
fn generated_resident_memory_permutation_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let u32_input = generated_atomic_values(0x3141_5926);
    let bool_input = generated_bool_values(0x2718_2818);
    let f32_input = generated_f32_values(0x1234_abcd);
    let mut checked_lanes = 0usize;

    for case in U32_MEMORY_CASES {
        let program = resident_memory_program(case);
        let outputs = resident_cuda_reference_outputs(
            &backend,
            &program,
            &[u32_bytes(&u32_input)],
            &[OUTPUT_BYTES],
            case.name,
        );
        checked_lanes += assert_u32_output_lanes(
            case.name,
            LANE_COUNT,
            &outputs.resident_cuda,
            &outputs.reference,
        );
    }

    for case in BOOL_MEMORY_CASES {
        let program = resident_memory_program(case);
        let outputs = resident_cuda_reference_outputs(
            &backend,
            &program,
            &[bool_bytes(&bool_input)],
            &[OUTPUT_BYTES],
            case.name,
        );
        checked_lanes += assert_u32_output_lanes(
            case.name,
            LANE_COUNT,
            &outputs.resident_cuda,
            &outputs.reference,
        );
    }

    for case in F32_MEMORY_CASES {
        let program = resident_memory_program(case);
        let outputs = resident_cuda_reference_outputs(
            &backend,
            &program,
            &[f32_bytes(&f32_input)],
            &[OUTPUT_BYTES],
            case.name,
        );
        checked_lanes += assert_f32_output_lanes(
            case.name,
            LANE_COUNT,
            MAX_F32_ULP,
            &outputs.resident_cuda,
            &outputs.reference,
        );
    }

    assert_eq!(
        checked_lanes,
        (U32_MEMORY_CASES.len() + BOOL_MEMORY_CASES.len() + F32_MEMORY_CASES.len()) * LANE_COUNT,
        "Fix: resident memory generated matrix must compare every output lane."
    );
}

