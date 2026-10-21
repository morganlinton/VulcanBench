//! Parser edit delta contracts test suite.

const CONTRACTS: &str = include_str!("../../docs/optimization/PARSER_EDIT_DELTA_CONTRACTS.toml");

#[test]
fn parser_edit_delta_contracts_record_reuse_and_invalidation() {
    for field in [
        "old_tree_digest",
        "new_tree_digest",
        "changed_ranges",
        "reused_node_count",
        "invalidated_fact_ranges",
        "scan_impact",
        "compiler_impact",
    ] {
        assert!(
            CONTRACTS.contains(field),
            "edit-delta contract must expose {field}"
        );
    }

    assert!(CONTRACTS.contains("rust-small-edit-span-limited"));
    assert!(CONTRACTS.contains("c-macro-body-invalidates-expansion"));
    assert!(CONTRACTS.contains("invalidated fact ranges cover every changed source range"));
}
