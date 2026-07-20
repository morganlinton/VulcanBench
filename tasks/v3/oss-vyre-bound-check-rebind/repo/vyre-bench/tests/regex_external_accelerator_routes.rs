//! Regex external accelerator routes test suite.

const ROUTES: &str =
    include_str!("../../docs/optimization/REGEX_EXTERNAL_ACCELERATOR_ROUTES.toml");

#[test]
fn regex_external_accelerator_routes_cover_dpu_and_fpga() {
    for required in [
        "accelerator_id = \"bluefield_dpu\"",
        "accelerator_id = \"fpga_offload\"",
        "compiled_rule_database",
        "automata_image",
        "artifact_digest_required = true",
    ] {
        assert!(
            ROUTES.contains(required),
            "Fix: external regex accelerator routes must include `{required}`"
        );
    }
}

#[test]
fn regex_external_accelerator_routes_require_parity_transfer_and_outputs() {
    for required in [
        "cpu_reference_parity_required = true",
        "rule_id_offset_length",
        "pattern_id_offset_length",
        "rule_upload_bytes",
        "image_upload_bytes",
        "match_bytes",
        "VYRE_SCAN_DPU_REGEX_UNSUPPORTED_FEATURE",
        "VYRE_SCAN_FPGA_REGEX_UNSUPPORTED_FEATURE",
    ] {
        assert!(
            ROUTES.contains(required),
            "Fix: external regex accelerator routes must include `{required}`"
        );
    }
    assert_eq!(
        ROUTES
            .matches("evidence_path = \"vyre-bench/tests/regex_external_accelerator_routes.rs\"")
            .count(),
        ROUTES.matches("[[accelerator]]").count(),
        "Fix: every external regex accelerator row must point at this proof gate"
    );
}
