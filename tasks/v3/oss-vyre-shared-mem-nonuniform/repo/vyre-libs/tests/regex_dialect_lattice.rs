//! Regex dialect lattice test suite.

const DIALECT_LATTICE: &str =
    include_str!("../../docs/optimization/REGEX_DIALECT_LATTICE.toml");
const SCAN_CONFORMANCE_MATRIX: &str =
    include_str!("../../docs/optimization/SCAN_CONFORMANCE_MATRIX.toml");

const REQUIRED_DIALECTS: &[&str] = &[
    "vyre_regular",
    "re2",
    "rust_regex",
    "hyperscan",
    "vectorscan",
    "pcre2",
    "libcudf_regex",
];

const REQUIRED_EXACTNESS_CLASSES: &[&str] = &[
    "exact",
    "unsupported",
    "prefilter",
    "verifier",
    "host-reference",
];

const REQUIRED_CONSTRUCTS: &[(&str, &str)] = &[
    ("literal", "VYRE_SCAN_OK_LITERAL"),
    (
        "capture_groups",
        "VYRE_SCAN_CAPTURE_EXTRACTION_REQUIRES_VERIFIER",
    ),
    (
        "lookaround",
        "VYRE_SCAN_APPROXIMATED_LOOKAROUND_REQUIRES_VERIFIER",
    ),
    ("backreference", "VYRE_SCAN_UNSUPPORTED_BACKREFERENCE"),
    (
        "unicode_classes",
        "VYRE_SCAN_UNSUPPORTED_UNICODE_MODE_GPU",
    ),
    ("streaming_chunks", "VYRE_SCAN_OK_STREAMING_STATE"),
];

#[test]
fn scan_conformance_links_the_regex_dialect_lattice() {
    assert!(
        SCAN_CONFORMANCE_MATRIX
            .contains("dialect_lattice = \"docs/optimization/REGEX_DIALECT_LATTICE.toml\""),
        "Fix: scan conformance matrix must link the canonical regex dialect lattice"
    );
}

#[test]
fn regex_dialect_lattice_declares_required_dialects_and_exactness_classes() {
    for dialect in REQUIRED_DIALECTS {
        assert!(
            DIALECT_LATTICE.contains(&format!("\"{dialect}\"")),
            "Fix: regex dialect lattice must declare dialect `{dialect}`"
        );
        assert!(
            DIALECT_LATTICE.contains(&format!("{dialect} =")),
            "Fix: regex dialect lattice must record support for dialect `{dialect}` in construct rows"
        );
    }

    for exactness in REQUIRED_EXACTNESS_CLASSES {
        assert!(
            DIALECT_LATTICE.contains(&format!("\"{exactness}\"")),
            "Fix: regex dialect lattice must declare exactness class `{exactness}`"
        );
    }
}

#[test]
fn regex_dialect_lattice_covers_constructs_with_diagnostics_and_evidence() {
    for (construct, diagnostic) in REQUIRED_CONSTRUCTS {
        assert!(
            DIALECT_LATTICE.contains(&format!("id = \"{construct}\"")),
            "Fix: regex dialect lattice must include construct `{construct}`"
        );
        assert!(
            DIALECT_LATTICE.contains(&format!("diagnostic_code = \"{diagnostic}\"")),
            "Fix: regex dialect lattice construct `{construct}` must record diagnostic `{diagnostic}`"
        );
    }

    let construct_rows = DIALECT_LATTICE.matches("[[construct]]").count();
    let evidence_paths = DIALECT_LATTICE
        .matches("evidence_path = \"vyre-libs/tests/regex_dialect_lattice.rs\"")
        .count();
    assert_eq!(
        construct_rows, evidence_paths,
        "Fix: every regex dialect lattice construct must name this proof gate"
    );
}

#[test]
fn regex_dialect_lattice_records_reject_prefilter_verifier_and_streaming_routes() {
    for route in [
        "fallback_route = \"reject\"",
        "fallback_route = \"prefilter-plus-verifier\"",
        "fallback_route = \"host-reference\"",
        "fallback_route = \"streaming-state\"",
    ] {
        assert!(
            DIALECT_LATTICE.contains(route),
            "Fix: regex dialect lattice must include route `{route}`"
        );
    }

    assert!(
        DIALECT_LATTICE.contains("hyperscan = \"prefilter\"")
            && DIALECT_LATTICE.contains("pcre2 = \"exact\"")
            && DIALECT_LATTICE.contains("re2 = \"unsupported\""),
        "Fix: lookaround dialect row must distinguish Hyperscan prefilter, PCRE2 exact support, and RE2 rejection"
    );
}
