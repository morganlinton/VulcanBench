//! Regex bitstream program registry test suite.

const PROGRAMS: &str = include_str!("../../docs/optimization/REGEX_BITSTREAM_PROGRAMS.toml");

#[test]
fn regex_bitstream_program_registry_records_metrics_and_program_layout() {
    for required in [
        "program_digest",
        "match_parity",
        "divergence_counter",
        "verifier_parity",
        "unsupported_operator",
        "interleaved_bit_parallel_words",
        "class_id = \"gpu_fit_regular_subset\"",
    ] {
        assert!(
            PROGRAMS.contains(required),
            "Fix: regex bitstream program registry must include `{required}`"
        );
    }
}

#[test]
fn regex_bitstream_program_registry_records_refusals_and_parity() {
    assert!(
        PROGRAMS.contains("verifier_required = true")
            && PROGRAMS.contains("parity_required = true")
            && PROGRAMS.contains("VYRE_CUDA_BITSTREAM_UNSUPPORTED_OPERATOR")
            && PROGRAMS.contains("VYRE_CUDA_BITSTREAM_REGISTER_PRESSURE_EXCEEDED"),
        "Fix: regex bitstream registry must record verifier/parity and refusal diagnostics"
    );
    assert_eq!(
        PROGRAMS
            .matches("evidence_path = \"vyre-driver-cuda/tests/regex_bitstream_program_registry.rs\"")
            .count(),
        PROGRAMS.matches("[[program_class]]").count(),
        "Fix: every regex bitstream program row must point at this proof gate"
    );
}
