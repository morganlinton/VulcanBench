//! Weir relation import certificates test suite.

const CERTIFICATES: &str = include_str!("../../docs/optimization/WEIR_RELATION_IMPORT_CERTIFICATES.toml");

#[test]
fn weir_relation_import_certificates_preserve_witness_boundaries() {
    for required in [
        "endpoint_domains",
        "call_string_ids",
        "sanitizer_ids",
        "tuple_digest",
        "source_span_mapping",
        "malformed_bytes_policy",
        "witness_path_policy",
    ] {
        assert!(
            CERTIFICATES.contains(required),
            "Weir relation import certificate must expose {required}"
        );
    }

    assert!(CERTIFICATES.contains("fail_closed"));
    assert!(CERTIFICATES.contains("preserve_source_span_chain"));
    assert!(CERTIFICATES.contains("weir-to-vyre-taint-edge"));
}
