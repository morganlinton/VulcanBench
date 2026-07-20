use super::*;

#[test]
fn conformance_tests_do_not_compile_out_gpu_gates() {
    let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut findings = Vec::new();
    scan_for_cfg_gated_gpu_tests(&tests_dir, &tests_dir, &mut findings);

    assert!(
        findings.is_empty(),
        "Fix: conformance GPU gates must fail loudly when GPU drivers are not linked; do not compile out tests/modules with cfg(feature = \"gpu\"):\n{}",
        findings.join("\n")
    );
}

#[test]
fn conformance_runner_wrong_output_pairs_have_replay_capsules_contract() {
    let source = repo_file("conform/vyre-conform-runner/src/main.rs");
    for required in [
        "replay_capsule: Option<ReplayCapsule>",
        "struct ReplayCapsule",
        "program_blake3",
        "witness_input_blake3",
        "reference_output_blake3",
        "backend_output_blake3",
        "first_replay_mismatch",
        "single_witness_case",
        "program.content_hash()",
        "build_replay_capsule(",
    ] {
        assert!(
            source.contains(required),
            "Fix: wrong-output conformance failures must keep structured replay capsule evidence; missing `{required}`."
        );
    }
    let production_source = source.split("#[cfg(test)]").next().unwrap_or(&source);
    let mismatch_count = production_source
        .matches("if let BufferParity::Mismatch(detail)")
        .count();
    let capsule_call_count = production_source
        .matches("replay_capsule: Some(build_replay_capsule(")
        .count();
    assert!(
        mismatch_count >= 2,
        "Fix: source contract expects both normal dispatch and convergence mismatch paths."
    );
    assert_eq!(
        capsule_call_count, mismatch_count,
        "Fix: every BufferParity mismatch path must attach a replay capsule instead of returning only prose."
    );
}

