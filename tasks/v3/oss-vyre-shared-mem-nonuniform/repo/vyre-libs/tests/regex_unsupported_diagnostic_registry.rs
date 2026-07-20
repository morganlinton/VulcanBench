//! Regex unsupported diagnostic registry test suite.

const REGISTRY: &str =
    include_str!("../../docs/optimization/REGEX_UNSUPPORTED_DIAGNOSTICS.toml");

const REQUIRED_DIAGNOSTICS: &[(&str, &str, bool)] = &[
    ("backreference", "VYRE_SCAN_UNSUPPORTED_BACKREFERENCE", false),
    (
        "lookaround",
        "VYRE_SCAN_APPROXIMATED_LOOKAROUND_REQUIRES_VERIFIER",
        true,
    ),
    (
        "unicode_classes_gpu",
        "VYRE_SCAN_UNSUPPORTED_UNICODE_MODE_GPU",
        true,
    ),
    (
        "capture_extraction",
        "VYRE_SCAN_CAPTURE_EXTRACTION_REQUIRES_VERIFIER",
        true,
    ),
    (
        "huge_alternation",
        "VYRE_SCAN_UNSUPPORTED_HUGE_ALTERNATION_BUDGET",
        false,
    ),
    (
        "nested_repeats",
        "VYRE_SCAN_UNSUPPORTED_NESTED_REPEAT_BUDGET",
        false,
    ),
];

const REQUIRED_FIELDS: &[&str] = &[
    "construct_id",
    "diagnostic_code",
    "syntax_span_policy",
    "dialect_expectation",
    "safe_fallback",
    "verifier_required",
    "remediation",
    "evidence_path",
];

#[test]
fn regex_unsupported_diagnostic_registry_covers_required_constructs() {
    for (construct, code, verifier_required) in REQUIRED_DIAGNOSTICS {
        assert!(
            REGISTRY.contains(&format!("construct_id = \"{construct}\"")),
            "Fix: unsupported regex diagnostic registry must include `{construct}`"
        );
        assert!(
            REGISTRY.contains(&format!("diagnostic_code = \"{code}\"")),
            "Fix: unsupported regex diagnostic `{construct}` must use `{code}`"
        );
        assert!(
            REGISTRY.contains(&format!("verifier_required = {verifier_required}")),
            "Fix: unsupported regex diagnostic `{construct}` must record verifier_required={verifier_required}"
        );
    }
}

#[test]
fn regex_unsupported_diagnostic_registry_records_operator_fields() {
    for field in REQUIRED_FIELDS {
        assert!(
            REGISTRY.contains(&format!("\"{field}\"")),
            "Fix: unsupported regex diagnostic registry must declare required field `{field}`"
        );
        assert!(
            REGISTRY.contains(&format!("{field} =")),
            "Fix: unsupported regex diagnostic registry must populate field `{field}`"
        );
    }

    let row_count = REGISTRY.matches("[[diagnostic]]").count();
    assert_eq!(
        row_count,
        REQUIRED_DIAGNOSTICS.len(),
        "Fix: unsupported regex diagnostic registry must have one row per required diagnostic"
    );
    assert_eq!(
        REGISTRY
            .matches("evidence_path = \"vyre-libs/tests/regex_unsupported_diagnostic_registry.rs\"")
            .count(),
        row_count,
        "Fix: every unsupported regex diagnostic row must point at this proof gate"
    );
}

#[test]
fn regex_unsupported_diagnostic_registry_names_span_fallback_and_remediation() {
    for required in [
        "exact_construct_span",
        "class_span",
        "capture_group_span",
        "alternation_span",
        "repeat_subtree_span",
        "prefilter-plus-verifier",
        "host-reference-or-verifier",
        "whole-match-plus-capture-verifier",
        "reject-or-split-pattern-set",
    ] {
        assert!(
            REGISTRY.contains(required),
            "Fix: unsupported regex diagnostic registry must include `{required}`"
        );
    }
    assert!(
        REGISTRY.matches("remediation = ").count() == REQUIRED_DIAGNOSTICS.len(),
        "Fix: every unsupported regex diagnostic must include remediation text"
    );
}
