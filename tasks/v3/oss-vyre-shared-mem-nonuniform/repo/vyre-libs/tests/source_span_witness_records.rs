//! Source span witness records test suite.

const WITNESSES: &str = include_str!("../../docs/optimization/SOURCE_SPAN_WITNESS_RECORDS.toml");

#[test]
fn source_span_witness_records_connect_findings_and_diagnostics() {
    for required in [
        "original_file_digest",
        "byte_range",
        "transformed_node_id",
        "diagnostic_id",
        "reporter_field",
        "witness_chain_digest",
    ] {
        assert!(
            WITNESSES.contains(required),
            "source-span witness records must include {required}"
        );
    }

    assert!(WITNESSES.contains("finding.primary_span"));
    assert!(WITNESSES.contains("diagnostic.span"));
}
