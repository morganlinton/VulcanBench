//! Relation engine comparators test suite.

const COMPARATORS: &str = include_str!("../../docs/optimization/RELATION_ENGINE_COMPARATORS.toml");

#[test]
fn relation_engine_comparators_require_tuple_and_witness_parity() {
    for required in [
        "vyre_relation",
        "datafrog_style",
        "souffle_generated",
        "tuple_corpus_digest",
        "tuple_count",
        "witness_path_digest",
        "iteration_count",
        "output_relation_digest",
    ] {
        assert!(
            COMPARATORS.contains(required),
            "relation comparator registry must include {required}"
        );
    }

    assert!(COMPARATORS.contains("taint-flow-small"));
    assert!(COMPARATORS.contains("call-graph-large"));
}
