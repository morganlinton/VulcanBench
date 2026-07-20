use super::*;

#[test]
fn generated_resident_u32_scalar_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let lhs = generated_atomic_values(0x1020_3040);
    let rhs = generated_atomic_values(0xa5a5_5a5a);
    let lhs_bytes = u32_bytes(&lhs);
    let rhs_bytes = u32_bytes(&rhs);
    let mut checked_lanes = 0usize;

    for case in U32_BINARY_CASES {
        let program = resident_u32_binary_program(case);
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

    for case in U32_UNARY_CASES {
        let program = resident_u32_unary_program(case);
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
        (U32_BINARY_CASES.len() + U32_UNARY_CASES.len()) * LANE_COUNT,
        "Fix: resident u32 scalar generated matrix must compare every output lane."
    );
}

#[test]

fn generated_resident_i32_scalar_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let lhs = generated_i32_cast_values(LANE_COUNT);
    let rhs = generated_i32_cast_values(LANE_COUNT)
        .into_iter()
        .enumerate()
        .map(|(lane, value)| {
            let mixed = value ^ ((lane as i32).wrapping_mul(0x1f1f_0101));
            if mixed == 0 || mixed == -1 {
                ((lane as i32) & 0x3ff) + 1
            } else {
                mixed
            }
        })
        .collect::<Vec<_>>();
    let lhs_bytes = i32_bytes(&lhs);
    let rhs_bytes = i32_bytes(&rhs);
    let mut checked_lanes = 0usize;

    for case in I32_BINARY_CASES {
        let program = resident_i32_binary_program(case);
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

    for case in I32_UNARY_CASES {
        let program = resident_i32_unary_program(case);
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
        (I32_BINARY_CASES.len() + I32_UNARY_CASES.len()) * LANE_COUNT,
        "Fix: resident i32 scalar generated matrix must compare every output lane."
    );
}

