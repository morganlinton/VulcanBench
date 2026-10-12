use super::*;

#[test]
fn generated_resident_f32_comparison_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let lhs = generated_f32_values(0x55aa_1234);
    let rhs = generated_f32_values(0xaa55_4321);
    let lhs_bytes = f32_bytes(&lhs);
    let rhs_bytes = f32_bytes(&rhs);
    let mut checked_lanes = 0usize;

    for case in F32_COMPARE_CASES {
        let program = resident_f32_compare_program(case);
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

    assert_eq!(
        checked_lanes,
        F32_COMPARE_CASES.len() * LANE_COUNT,
        "Fix: resident f32 comparison matrix must compare every output lane, including NaN comparison lanes."
    );
}

#[test]
fn generated_resident_f32_binary_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let lhs = generated_f32_values(0x1357_9bdf);
    let mixed_rhs = generated_f32_values(0x2468_ace0);
    let nonzero_rhs = generated_f32_nonzero_values(0x0bad_f00d);
    let lhs_bytes = f32_bytes(&lhs);
    let mixed_rhs_bytes = f32_bytes(&mixed_rhs);
    let nonzero_rhs_bytes = f32_bytes(&nonzero_rhs);
    let mut checked_lanes = 0usize;

    for case in F32_BINARY_CASES {
        let rhs_bytes = match case.rhs {
            F32RhsKind::Mixed => &mixed_rhs_bytes,
            F32RhsKind::NonZero => &nonzero_rhs_bytes,
        };
        let program = resident_f32_binary_program(case);
        let outputs = resident_cuda_reference_outputs(
            &backend,
            &program,
            &[lhs_bytes.clone(), rhs_bytes.clone()],
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
        F32_BINARY_CASES.len() * LANE_COUNT,
        "Fix: resident f32 binary matrix must compare every output lane."
    );
}

#[test]
fn generated_resident_f32_unary_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let mixed = generated_f32_values(0xfeed_beef);
    let nonzero = generated_f32_nonzero_values(0xabcd_1234);
    let sqrt_domain = generated_f32_sqrt_domain_values(0xdec0_ded1);
    let mixed_bytes = f32_bytes(&mixed);
    let nonzero_bytes = f32_bytes(&nonzero);
    let sqrt_domain_bytes = f32_bytes(&sqrt_domain);
    let mut checked_lanes = 0usize;

    for case in F32_UNARY_CASES {
        let input_bytes = match case.inputs {
            F32InputKind::Mixed => &mixed_bytes,
            F32InputKind::NonZero => &nonzero_bytes,
            F32InputKind::SqrtDomain => &sqrt_domain_bytes,
        };
        let program = resident_f32_unary_program(case);
        let outputs = resident_cuda_reference_outputs(
            &backend,
            &program,
            std::slice::from_ref(input_bytes),
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
        F32_UNARY_CASES.len() * LANE_COUNT,
        "Fix: resident f32 unary matrix must compare every output lane."
    );
}

#[test]
fn generated_resident_f32_classification_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let input = generated_f32_classification_values();
    let input_bytes = f32_bytes(&input);
    let mut checked_lanes = 0usize;

    for case in F32_CLASSIFY_CASES {
        let program = resident_f32_classify_program(case);
        let outputs = resident_cuda_reference_outputs(
            &backend,
            &program,
            std::slice::from_ref(&input_bytes),
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
        F32_CLASSIFY_CASES.len() * LANE_COUNT,
        "Fix: resident f32 classification matrix must compare every output lane."
    );
}

#[test]
fn generated_resident_f32_fma_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let a = generated_f32_fma_values(LANE_COUNT, 0x1234_5678);
    let b = generated_f32_fma_values(LANE_COUNT, 0x9abc_def0);
    let c = generated_f32_fma_values(LANE_COUNT, 0x0fed_cba9);
    let inputs = vec![f32_bytes(&a), f32_bytes(&b), f32_bytes(&c)];
    let program = resident_fma_program();
    let outputs = resident_cuda_reference_outputs(
        &backend,
        &program,
        &inputs,
        &[OUTPUT_BYTES],
        "resident_f32_fma",
    );
    let checked_lanes = assert_f32_output_lanes(
        "resident_f32_fma",
        LANE_COUNT,
        MAX_F32_ULP,
        &outputs.resident_cuda,
        &outputs.reference,
    );
    assert_eq!(
        checked_lanes, LANE_COUNT,
        "Fix: resident FMA generated matrix must compare every output lane."
    );
}

