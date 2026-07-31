//! Csr certificates test suite.

const CERTIFICATES: &str = include_str!("../../docs/optimization/CSR_CERTIFICATES.toml");

#[test]
fn csr_certificates_cover_shape_policy_and_exact_diagnostics() {
    for required in [
        "offset_monotonicity",
        "target_bounds",
        "duplicate_policy",
        "sort_policy",
        "reverse_edge_status",
        "owner_lane",
        "VYRE_CSR_UNSORTED_DUPLICATE_MISSING_REVERSE",
    ] {
        assert!(
            CERTIFICATES.contains(required),
            "CSR certificate registry must include {required}"
        );
    }
}
