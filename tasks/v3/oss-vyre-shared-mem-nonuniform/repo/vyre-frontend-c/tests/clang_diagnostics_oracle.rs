//! Tests for clang diagnostic fact extraction.

mod support;

use std::fs;

use support::clang_diagnostics::clang_diagnostics;

#[test]
fn clang_diagnostics_oracle_records_severity_location_and_fixits() {
    let path = std::env::temp_dir().join(format!(
        "vyrec-clang-diagnostics-oracle-{}.c",
        std::process::id()
    ));
    fs::write(&path, "int f(void) { int x = ; return x }\n").expect("test source must be writable");

    let diagnostics = clang_diagnostics(&path).expect("clang diagnostics must be parseable");
    fs::remove_file(&path).expect("test source must be removable");

    let expression_error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message == "expected expression")
        .expect("expected-expression diagnostic must be present");
    assert_eq!(expression_error.severity, "error");
    assert_eq!(expression_error.category, "error");
    assert_eq!(expression_error.sequence_index, 0);
    assert!(!expression_error.recovered_after_error);
    assert_eq!(expression_error.location.line, 1);
    assert_eq!(expression_error.location.column, 23);

    let semicolon_error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message == "expected ';' after return statement")
        .expect("missing-semicolon diagnostic must be present");
    assert_eq!(semicolon_error.severity, "error");
    assert_eq!(semicolon_error.sequence_index, 1);
    assert!(semicolon_error.recovered_after_error);
    assert_eq!(semicolon_error.location.line, 1);
    assert_eq!(semicolon_error.location.column, 33);
    assert_eq!(semicolon_error.fixits.len(), 1);
    assert_eq!(semicolon_error.fixits[0].replacement, ";");
    assert_eq!(semicolon_error.fixits[0].start_line, 1);
    assert_eq!(semicolon_error.fixits[0].start_column, 33);
}
