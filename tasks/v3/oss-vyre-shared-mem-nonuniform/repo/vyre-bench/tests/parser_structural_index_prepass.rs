//! Parser structural index prepass test suite.

const PREPASS: &str = include_str!("../../docs/optimization/PARSER_STRUCTURAL_INDEX_PREPASS.toml");

#[test]
fn structural_index_prepass_records_chunk_safety_and_cost() {
    for required in [
        "structural_index_digest",
        "boundary_safety",
        "parse_reuse_evidence",
        "preparse_cost_ns",
        "ends-on-item-boundary",
        "ends-outside-comment-and-string",
    ] {
        assert!(
            PREPASS.contains(required),
            "structural index prepass must record {required}"
        );
    }

    assert!(PREPASS.contains("line_comment"));
    assert!(PREPASS.contains("block_comment"));
    assert!(PREPASS.contains("raw_string_literal"));
}
