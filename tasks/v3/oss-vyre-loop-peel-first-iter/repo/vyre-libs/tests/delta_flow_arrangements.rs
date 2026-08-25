//! Delta flow arrangements test suite.

const ARRANGEMENTS: &str = include_str!("../../docs/optimization/DELTA_FLOW_ARRANGEMENTS.toml");

#[test]
fn delta_flow_arrangements_record_signed_epochs_and_replay() {
    for required in [
        "epoch",
        "positive_deltas",
        "negative_deltas",
        "compaction_frontier",
        "replay_digest",
        "recompute_scope",
        "tuple_count_contract",
    ] {
        assert!(
            ARRANGEMENTS.contains(required),
            "delta arrangement registry must include {required}"
        );
    }

    assert!(ARRANGEMENTS.contains("replay-matches-full-recompute"));
}
