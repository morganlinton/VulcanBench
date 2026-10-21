//! Regex capture mode contracts test suite.

const CONTRACTS: &str =
    include_str!("../../docs/optimization/REGEX_CAPTURE_MODE_CONTRACTS.toml");

const REQUIRED_MODES: &[&str] = &[
    "noncapture",
    "count",
    "span",
    "named_capture",
    "repeated_capture",
    "group_extraction",
];

const REQUIRED_OUTPUT_FIELDS: &[&str] = &[
    "match_id",
    "pattern_id",
    "start",
    "end",
    "group_id",
    "group_name",
    "nullable",
];

#[test]
fn regex_capture_mode_contracts_cover_required_modes_and_fields() {
    for mode in REQUIRED_MODES {
        assert!(
            CONTRACTS.contains(&format!("mode_id = \"{mode}\"")),
            "Fix: regex capture mode contracts must include `{mode}`"
        );
    }
    for field in REQUIRED_OUTPUT_FIELDS {
        assert!(
            CONTRACTS.contains(&format!("\"{field}\"")),
            "Fix: regex capture mode contracts must declare output field `{field}`"
        );
    }
}

#[test]
fn regex_capture_mode_contracts_gate_extraction_on_verifier() {
    for required in [
        "mode_id = \"named_capture\"",
        "mode_id = \"repeated_capture\"",
        "mode_id = \"group_extraction\"",
        "verifier_required = true",
        "accelerator_eligible = false",
        "unmatched-group-null",
    ] {
        assert!(
            CONTRACTS.contains(required),
            "Fix: capture extraction contracts must include `{required}`"
        );
    }
}

#[test]
fn regex_capture_mode_contracts_keep_whole_match_modes_accelerator_eligible() {
    for required in [
        "mode_id = \"noncapture\"",
        "mode_id = \"count\"",
        "mode_id = \"span\"",
        "whole_match_only",
        "match_count_per_pattern",
        "whole_match_span",
        "accelerator_eligible = true",
    ] {
        assert!(
            CONTRACTS.contains(required),
            "Fix: whole-match capture modes must include `{required}`"
        );
    }
    let mode_rows = CONTRACTS.matches("[[mode]]").count();
    assert_eq!(
        CONTRACTS
            .matches("evidence_path = \"vyre-libs/tests/regex_capture_mode_contracts.rs\"")
            .count(),
        mode_rows,
        "Fix: every capture mode row must point at this proof gate"
    );
}
