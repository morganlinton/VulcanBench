use super::*;

#[test]
fn parity_matrix_input_planner_tracks_dynamic_fixture_contract() {
    for path in ["conform/vyre-conform-runner/src/witness_plan.rs"] {
        let source = repo_file(path);
        assert!(
            source.contains("matching_fixture_bytes(")
                && source.contains("fixture_index")
                && source.contains("byte_len: Option<usize>"),
            "Fix: `{path}` must route backend witness inputs by logical fixture order with optional static byte lengths, not only raw Program::buffers indices."
        );
        assert!(
            source.contains("runtime-sized read-write buffer"),
            "Fix: `{path}` must reject omitted runtime-sized read-write buffers instead of silently zeroing an unknown byte length."
        );
        assert!(
            !source.contains("fixture_buffer_count"),
            "Fix: `{path}` must not infer read-write fixture presence from a raw fixture count; use per-buffer fixture matching."
        );
    }

    let main = repo_file("conform/vyre-conform-runner/src/main.rs");
    assert!(
        main.contains("WitnessInputPlan::for_program(program)")
            && main.contains("plan_witness_inputs_into(fixture_inputs, plan, backend_inputs)"),
        "Fix: CLI release conformance must route production witness planning through the shared witness_plan module."
    );

    let parity = repo_file("conform/vyre-conform-runner/tests/__split/parity_matrix_chunk1.rs");
    assert!(
        parity.contains("matching_fixture_bytes(")
            && parity.contains("fixture_index")
            && parity.contains("byte_len: Option<usize>")
            && parity.contains("runtime-sized read-write buffer")
            && !parity.contains("fixture_buffer_count"),
        "Fix: parity matrix harness must keep the logical fixture/static-length/read-write witness contract."
    );
}

#[test]
fn bundle_certificate_uses_shared_witness_input_planner() {
    let source = repo_file("conform/vyre-conform-runner/src/bundle_cert.rs");

    assert!(
        source.contains("WitnessInputPlan::for_program(program)")
            && source.contains("plan_witness_inputs_into(&witness.inputs, input_plan")
            && source.contains("WitnessPlanningFailed"),
        "Fix: bundle cert issue/verify must dispatch through the same planned logical witness stream as release conformance."
    );
    assert!(
        !source.contains("witness.inputs.iter().map(Vec::as_slice).collect"),
        "Fix: bundle cert backend verification must not feed raw witness buffers directly to dispatch_borrowed."
    );
}

#[test]
fn ulp_audit_input_planner_tracks_release_witness_contract() {
    let source = repo_file("conform/vyre-conform-runner/tests/ulp_audit.rs");
    let plan = repo_file("conform/vyre-conform-runner/tests/__split/ulp_audit_input_plan.rs");
    let split = repo_file("conform/vyre-conform-runner/tests/__split/ulp_audit_part1.rs");

    assert!(
        plan.contains("fn backend_dispatch_plan(")
            && plan.contains("matching_fixture_bytes(")
            && plan.contains("ReadWriteOrZero")
            && plan.contains("runtime-sized read-write buffer"),
        "Fix: ULP audit must use the same logical fixture/static-length/read-write witness planning contract as release conformance."
    );
    assert!(
        split.contains("backend_dispatch_plan(&program)")
            && split.contains("backend_inputs_from_fixture_into(inputs, &input_plan")
            && split.contains("backend_input_buffer_indices(&input_plan)"),
        "Fix: release_per_op_f32_ulp_audit must dispatch fixture and adversarial cases through the planned backend input stream."
    );
    assert!(
        !format!("{source}\n{plan}")
            .contains("BindingPlan")
            && !format!("{source}\n{plan}").contains("backend_input_map")
            && !format!("{source}\n{plan}").contains("fixture_len"),
        "Fix: ULP audit must not infer backend inputs from raw fixture counts or BindingPlan-only buffer indices."
    );
}
