//! Parser recovery corpus registry test suite.

const CORPUS: &str = include_str!("../../docs/optimization/PARSER_RECOVERY_CORPUS.toml");

#[test]
fn parser_recovery_corpus_records_diagnostics_and_gpu_blockers() {
    for required in [
        "malformed_token_class",
        "recovery_span",
        "partial_tree_digest",
        "diagnostic_code",
        "gpu_blocker",
        "fail_closed",
        "recover_with_partial_tree",
    ] {
        assert!(
            CORPUS.contains(required),
            "parser recovery corpus must record {required}"
        );
    }

    assert!(CORPUS.contains("rust-unclosed-raw-string"));
    assert!(CORPUS.contains("c-unterminated-block-comment"));
    assert!(CORPUS.contains("python-mixed-indent"));
}
