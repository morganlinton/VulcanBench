use super::*;

#[test]
fn generated_resident_atomic_reduction_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let mut checked_lanes = 0usize;

    for case in RESIDENT_ATOMIC_CASES {
        let program = resident_atomic_reduction_program(case);
        let initial = vec![case.identity; LANE_COUNT];
        let values = generated_atomic_values(case.value_salt);
        let inputs = vec![u32_bytes(&initial), u32_bytes(&values)];
        let (resident_cuda, reference) =
            resident_in_place_reference_outputs(&backend, &program, &inputs, &[0], case.name);
        checked_lanes += assert_u32_output_lanes(case.name, LANE_COUNT, &resident_cuda, &reference);
    }

    assert_eq!(
        checked_lanes,
        RESIDENT_ATOMIC_CASES.len() * LANE_COUNT,
        "Fix: resident atomic generated matrix must compare every accumulator lane."
    );
}

#[test]
fn generated_resident_cast_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let u32_input = generated_u32_cast_values(LANE_COUNT);
    let i32_input = generated_i32_cast_values(LANE_COUNT);
    let f32_input = generated_f32_cast_values(LANE_COUNT);
    let bool_input = generated_bool_cast_values(LANE_COUNT);
    let mut checked_lanes = 0usize;

    for case in CAST_CASES {
        let input = match &case.input_type {
            DataType::U32 => u32_bytes(&u32_input),
            DataType::I32 => i32_bytes(&i32_input),
            DataType::F32 => f32_bytes(&f32_input),
            DataType::Bool => bool_bytes(&bool_input),
            _ => unreachable!("resident generated cast matrix only covers scalar storage types"),
        };
        let program = resident_cast_program(case);
        let outputs = resident_cuda_reference_outputs(
            &backend,
            &program,
            &[input],
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
        CAST_CASES.len() * LANE_COUNT,
        "Fix: resident cast generated matrix must compare every output lane."
    );
}

