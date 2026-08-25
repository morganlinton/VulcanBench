//! Regex streaming state ledger test suite.

const LEDGER: &str = include_str!("../../docs/optimization/REGEX_STREAMING_STATE_LEDGER.toml");

const REQUIRED_MODES: &[&str] = &["block", "streaming", "vectored"];

#[test]
fn regex_streaming_state_ledger_covers_required_modes() {
    for mode in REQUIRED_MODES {
        assert!(
            LEDGER.contains(&format!("stream_mode = \"{mode}\"")),
            "Fix: regex streaming state ledger must include mode `{mode}`"
        );
    }
}

#[test]
fn regex_streaming_state_ledger_records_state_bytes_and_boundary_policy() {
    for required in [
        "state_bytes = 0",
        "state_bytes = 128",
        "state_bytes = 64",
        "chunk_boundary_policy = \"single-buffer\"",
        "chunk_boundary_policy = \"carry-prefix-suffix-and-automata-state\"",
        "chunk_boundary_policy = \"ordered-buffer-vector\"",
    ] {
        assert!(
            LEDGER.contains(required),
            "Fix: regex streaming state ledger must include `{required}`"
        );
    }
}

#[test]
fn regex_streaming_state_ledger_records_finalization_offsets_and_reset() {
    for required in [
        "finalization = \"required\"",
        "finalization = \"required-after-last-segment\"",
        "reset_semantics = \"explicit-stream-reset\"",
        "absolute-offsets-from-stream-start",
        "absolute-offsets-over-concatenated-vector",
        "requires-som-state",
    ] {
        assert!(
            LEDGER.contains(required),
            "Fix: regex streaming state ledger must include `{required}`"
        );
    }
    let stream_rows = LEDGER.matches("[[stream]]").count();
    assert_eq!(
        LEDGER
            .matches("evidence_path = \"vyre-libs/tests/regex_streaming_state_ledger.rs\"")
            .count(),
        stream_rows,
        "Fix: every streaming state row must point at this proof gate"
    );
}
