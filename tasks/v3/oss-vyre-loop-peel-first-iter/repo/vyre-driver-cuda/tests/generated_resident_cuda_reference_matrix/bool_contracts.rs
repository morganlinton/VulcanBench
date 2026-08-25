use super::*;

#[test]
fn generated_resident_bool_scalar_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let lhs = generated_bool_values(0x1020_3040);
    let rhs = generated_bool_values(0xa5a5_5a5a);
    let lhs_bytes = bool_bytes(&lhs);
    let rhs_bytes = bool_bytes(&rhs);
    let mut checked_lanes = 0usize;

    for case in BOOL_BINARY_CASES {
        let program = resident_bool_binary_program(case);
        let outputs = resident_cuda_reference_outputs(
            &backend,
            &program,
            &[lhs_bytes.clone(), rhs_bytes.clone()],
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

    for case in BOOL_UNARY_CASES {
        let program = resident_bool_unary_program(case);
        let outputs = resident_cuda_reference_outputs(
            &backend,
            &program,
            std::slice::from_ref(&lhs_bytes),
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

    assert_eq!(
        checked_lanes,
        (BOOL_BINARY_CASES.len() + BOOL_UNARY_CASES.len()) * LANE_COUNT,
        "Fix: resident Bool generated matrix must compare every output lane."
    );
}

#[test]
fn generated_resident_bool_select_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let flag = generated_bool_values(0x3333_cccc);
    let lhs = generated_bool_values(0x1234_abcd);
    let rhs = generated_bool_values(0xdcba_4321);
    let inputs = vec![bool_bytes(&flag), bool_bytes(&lhs), bool_bytes(&rhs)];
    let program = resident_bool_select_program();
    let outputs = resident_cuda_reference_outputs(
        &backend,
        &program,
        &inputs,
        &[OUTPUT_BYTES],
        "resident_bool_select",
    );
    let checked_lanes = assert_u32_output_lanes(
        "resident_bool_select",
        LANE_COUNT,
        &outputs.resident_cuda,
        &outputs.reference,
    );
    assert_eq!(
        checked_lanes, LANE_COUNT,
        "Fix: resident Bool select generated matrix must compare every output lane."
    );
}

