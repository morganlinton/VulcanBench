//! Wgsl scan uniformity certificates test suite.

const CERTIFICATES: &str =
    include_str!("../../docs/optimization/WGSL_SCAN_UNIFORMITY_CERTIFICATES.toml");

#[test]
fn wgsl_scan_uniformity_certificates_record_required_fields() {
    for required in [
        "shader_id",
        "branch_class",
        "subgroup_call_site",
        "uniform_input_proof",
        "fallback_reason",
        "regex_subgroup_ballot_scan",
        "regex_subgroup_reduce_scan",
    ] {
        assert!(
            CERTIFICATES.contains(required),
            "Fix: WGSL scan uniformity certificates must include `{required}`"
        );
    }
}

#[test]
fn wgsl_scan_uniformity_certificates_gate_subgroup_call_sites() {
    for required in [
        "branch_class = \"uniform-over-subgroup\"",
        "branch_class = \"uniform-over-workgroup-entry\"",
        "subgroup_call_site = \"candidate_mask_ballot\"",
        "subgroup_call_site = \"candidate_count_reduce\"",
        "pattern_program_digest_is_uniform",
        "workgroup_route_id_is_uniform",
        "VYRE_WGSL_SUBGROUP_UNIFORMITY_MISSING",
    ] {
        assert!(
            CERTIFICATES.contains(required),
            "Fix: WGSL scan uniformity certificates must include `{required}`"
        );
    }
    assert_eq!(
        CERTIFICATES
            .matches("evidence_path = \"vyre-driver-wgpu/tests/wgsl_scan_uniformity_certificates.rs\"")
            .count(),
        CERTIFICATES.matches("[[certificate]]").count()
            + CERTIFICATES.matches("[[rejection]]").count(),
        "Fix: every WGSL uniformity row must point at this proof gate"
    );
}
