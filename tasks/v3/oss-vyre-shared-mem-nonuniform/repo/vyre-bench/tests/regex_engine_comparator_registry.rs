//! Regex engine comparator registry test suite.

const REGISTRY: &str = include_str!("../../docs/optimization/REGEX_ENGINE_COMPARATORS.toml");

const REQUIRED_ENGINES: &[&str] = &[
    "vyre",
    "hyperscan",
    "vectorscan",
    "re2",
    "pcre2_jit",
    "rust_regex",
];

const REQUIRED_FIELDS: &[&str] = &[
    "engine_id",
    "version_source",
    "syntax_profile",
    "flags",
    "corpus_digest",
    "output_parity",
    "resource_limits",
    "timing_source",
    "evidence_path",
];

#[test]
fn regex_engine_comparator_registry_covers_required_engines() {
    for engine in REQUIRED_ENGINES {
        assert!(
            REGISTRY.contains(&format!("engine_id = \"{engine}\"")),
            "Fix: regex engine comparator registry must include `{engine}`"
        );
    }
}

#[test]
fn regex_engine_comparator_registry_records_required_fields() {
    for field in REQUIRED_FIELDS {
        assert!(
            REGISTRY.contains(&format!("\"{field}\"")),
            "Fix: regex engine comparator registry must declare required field `{field}`"
        );
        assert!(
            REGISTRY.contains(&format!("{field} =")),
            "Fix: regex engine comparator registry must populate field `{field}`"
        );
    }
}

#[test]
fn regex_engine_comparator_registry_requires_parity_limits_and_timing() {
    let engine_rows = REGISTRY.matches("[[engine]]").count();
    assert_eq!(
        engine_rows,
        REQUIRED_ENGINES.len(),
        "Fix: regex comparator registry must have exactly one row per required engine"
    );

    assert_eq!(
        REGISTRY.matches("output_parity = \"required\"").count(),
        engine_rows,
        "Fix: every regex comparator row must require output parity"
    );
    assert_eq!(
        REGISTRY.matches("corpus_digest = \"registered:regex-comparator-core\"").count(),
        engine_rows,
        "Fix: every regex comparator row must use the registered comparator corpus digest"
    );
    assert_eq!(
        REGISTRY
            .matches("evidence_path = \"vyre-bench/tests/regex_engine_comparator_registry.rs\"")
            .count(),
        engine_rows,
        "Fix: every regex comparator row must point at this proof gate"
    );
    assert!(
        REGISTRY.contains("match_limit")
            && REGISTRY.contains("jit_stack_limit")
            && REGISTRY.contains("pattern_too_large")
            && REGISTRY.contains("construct_budget"),
        "Fix: comparator registry must record concrete resource-limit fields for peer and Vyre engines"
    );
}
