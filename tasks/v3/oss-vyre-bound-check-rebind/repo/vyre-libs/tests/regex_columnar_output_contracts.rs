//! Regex columnar output contracts test suite.

const CONTRACTS: &str =
    include_str!("../../docs/optimization/REGEX_COLUMNAR_OUTPUT_CONTRACTS.toml");

const REQUIRED_SHAPES: &[&str] = &[
    "contains",
    "count",
    "first_span",
    "all_spans",
    "grouped_extraction",
];
const REQUIRED_FIELDS: &[&str] = &[
    "row_id",
    "pattern_id",
    "match_count",
    "span_start",
    "span_end",
    "group_id",
    "group_name",
    "null_state",
];

#[test]
fn regex_columnar_output_contracts_cover_shapes_and_fields() {
    for shape in REQUIRED_SHAPES {
        assert!(
            CONTRACTS.contains(&format!("shape_id = \"{shape}\"")),
            "Fix: columnar regex output contracts must include shape `{shape}`"
        );
    }
    for field in REQUIRED_FIELDS {
        assert!(
            CONTRACTS.contains(&format!("\"{field}\"")),
            "Fix: columnar regex output contracts must declare field `{field}`"
        );
    }
}

#[test]
fn regex_columnar_output_contracts_record_nulls_rows_and_spans() {
    for required in [
        "null-input-yields-null-output",
        "no-match-yields-null-span",
        "unmatched-group-yields-null",
        "one-output-row-per-input-row",
        "list-offsets-preserve-input-rows",
        "all-match-spans-in-row-order",
        "group-spans-in-match-order",
    ] {
        assert!(
            CONTRACTS.contains(required),
            "Fix: columnar regex output contracts must include `{required}`"
        );
    }
}

#[test]
fn regex_columnar_output_contracts_require_stable_layout_and_proof_gate() {
    let shape_rows = CONTRACTS.matches("[[shape]]").count();
    assert_eq!(
        CONTRACTS.matches("stable_binary_layout = true").count(),
        shape_rows,
        "Fix: every columnar regex output shape must require stable binary layout"
    );
    assert_eq!(
        CONTRACTS
            .matches("evidence_path = \"vyre-libs/tests/regex_columnar_output_contracts.rs\"")
            .count(),
        shape_rows,
        "Fix: every columnar regex output shape must point at this proof gate"
    );
}
