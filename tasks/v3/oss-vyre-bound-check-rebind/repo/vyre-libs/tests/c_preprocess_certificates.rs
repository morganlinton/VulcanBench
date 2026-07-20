//! C preprocess certificates test suite.

const CERTIFICATES: &str = include_str!("../../docs/optimization/C_PREPROCESS_CERTIFICATES.toml");

#[test]
fn c_preprocess_certificates_gate_gpu_eligibility() {
    for required in [
        "include_graph_digest",
        "macro_table_digest",
        "conditional_truth_map_digest",
        "disabled_byte_ranges",
        "gpu_eligibility",
        "certificate_digest",
    ] {
        assert!(
            CERTIFICATES.contains(required),
            "C preprocess certificate must expose {required}"
        );
    }

    assert!(CERTIFICATES.contains("blocked_by_unknown_condition"));
    assert!(CERTIFICATES.contains("GPU tokenization cannot claim eligibility without a matching certificate digest"));
}
