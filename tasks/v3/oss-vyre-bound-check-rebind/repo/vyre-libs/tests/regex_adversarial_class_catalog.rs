//! Regex adversarial class catalog test suite.

const CATALOG: &str = include_str!("../../docs/optimization/REGEX_ADVERSARIAL_CLASSES.toml");

const REQUIRED_CLASSES: &[(&str, &str)] = &[
    (
        "quadratic_rescan",
        "bounded literal-first scan uses guards to avoid quadratic rescans",
    ),
    ("empty_match", "empty-progress loops cannot livelock"),
    (
        "overlapping_suffix",
        "overlapping suffix matches preserve leftmost offsets",
    ),
    (
        "utf8_boundary",
        "UTF-8 boundary matching preserves valid Unicode offsets",
    ),
    (
        "weak_literal",
        "weak literal prefilters require verifier saturation evidence",
    ),
    (
        "nested_repeats",
        "nested repeat constructs exercise catastrophic expansion guards",
    ),
    (
        "nullable_loops",
        "nullable loop constructs exercise zero-progress guards",
    ),
    (
        "anchors",
        "anchor constructs exercise chunk and stream boundary semantics",
    ),
    (
        "lookarounds",
        "lookaround constructs exercise prefilter plus verifier exactness",
    ),
    (
        "backreferences",
        "backreference constructs exercise rejection diagnostics",
    ),
    (
        "unicode_classes",
        "Unicode class constructs exercise byte and scalar mode separation",
    ),
    (
        "huge_alternations",
        "huge alternation constructs exercise compile budget guards",
    ),
];

const REQUIRED_ROLES: &[&str] = &[
    "positive",
    "negative",
    "boundary",
    "adversarial",
    "baseline",
    "evasion",
    "resource_budget",
];

#[test]
fn regex_adversarial_class_catalog_has_required_roles() {
    for (class_id, _) in REQUIRED_CLASSES {
        for role in REQUIRED_ROLES {
            let needle = format!("class_id = \"{class_id}\"\nrole = \"{role}\"");
            assert!(
                CATALOG.contains(&needle),
                "Fix: regex adversarial class `{class_id}` must include `{role}` coverage"
            );
        }
    }
}

#[test]
fn regex_adversarial_class_catalog_has_construct_specific_classes() {
    for (class_id, risk_text) in REQUIRED_CLASSES {
        assert!(
            CATALOG.contains(&format!("class_id = \"{class_id}\"")),
            "Fix: regex adversarial catalog must include class `{class_id}`"
        );
        assert!(
            risk_text.contains("guard")
                || risk_text.contains("reject")
                || risk_text.contains("preserve")
                || risk_text.contains("verifier")
                || risk_text.contains("boundary")
                || risk_text.contains("separation")
                || risk_text.contains("budget")
                || risk_text.contains("livelock"),
            "Fix: regex adversarial class `{class_id}` must name the proof intent"
        );
    }
}

#[test]
fn regex_adversarial_class_catalog_rows_point_to_this_proof_gate() {
    let case_rows = CATALOG.matches("[[case]]").count();
    let evidence_rows = CATALOG
        .matches("evidence_path = \"vyre-libs/tests/regex_adversarial_class_catalog.rs\"")
        .count();
    assert_eq!(
        case_rows, evidence_rows,
        "Fix: every regex adversarial case must name this proof gate"
    );
}
