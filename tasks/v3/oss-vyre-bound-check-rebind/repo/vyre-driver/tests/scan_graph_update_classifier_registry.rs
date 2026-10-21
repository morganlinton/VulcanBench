//! Scan graph update classifier registry test suite.

const CLASSIFIER: &str =
    include_str!("../../docs/optimization/SCAN_GRAPH_UPDATE_CLASSIFIER.toml");

const REQUIRED_EDITS: &[&str] = &[
    "pattern_digest_same",
    "pattern_digest_changed",
    "haystack_same_shape",
    "haystack_shape_changed",
    "output_slab_resize",
    "verifier_topology_changed",
];

#[test]
fn scan_graph_update_classifier_registry_covers_required_edits() {
    for edit in REQUIRED_EDITS {
        assert!(
            CLASSIFIER.contains(&format!("edit_id = \"{edit}\"")),
            "Fix: scan graph update classifier must include edit `{edit}`"
        );
    }
}

#[test]
fn scan_graph_update_classifier_registry_distinguishes_replay_update_and_recapture() {
    for required in [
        "graph_action = \"replay\"",
        "graph_action = \"update_node_params\"",
        "graph_action = \"recapture\"",
        "topology_breaking = false",
        "topology_breaking = true",
        "VYRE_SCAN_GRAPH_VERIFIER_TOPOLOGY_CHANGED",
    ] {
        assert!(
            CLASSIFIER.contains(required),
            "Fix: scan graph update classifier must include `{required}`"
        );
    }
    assert_eq!(
        CLASSIFIER
            .matches("evidence_path = \"vyre-driver/tests/scan_graph_update_classifier_registry.rs\"")
            .count(),
        CLASSIFIER.matches("[[edit]]").count(),
        "Fix: every scan graph update row must point at this proof gate"
    );
}
