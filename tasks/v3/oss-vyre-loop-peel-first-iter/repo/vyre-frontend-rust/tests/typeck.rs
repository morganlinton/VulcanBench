//! Type-checking tests for the nano-subset (`vyre_libs::parsing::rust::sema::typeck`).
//!
//! Each construct has a positive case and a negative twin; verdicts match
//! rustc's E0308 / E0061 / E0614 behavior on the same programs.

#![forbid(unsafe_code)]

use vyre_libs::parsing::rust::lex::lexer::core::lex;
use vyre_libs::parsing::rust::parse::parse;
use vyre_libs::parsing::rust::sema::{resolve, typeck, RustSemaError};

fn typeck_src(src: &str) -> Result<(), RustSemaError> {
    let bytes = src.as_bytes();
    let tokens = lex(bytes).expect("Fix: test corpus must lex");
    let module = parse(bytes, &tokens).expect("Fix: test corpus must parse");
    let resolution = resolve(&module, bytes).expect("Fix: test corpus must resolve");
    typeck(&module, bytes, &resolution)
}

// ---- positive ----

#[test]
fn accepts_arithmetic_returning_i32() {
    typeck_src("fn add(a: i32, b: i32) -> i32 { return a + b * 2 / 1 - 0; }")
        .expect("Fix: i32 arithmetic must type-check");
}

#[test]
fn accepts_comparison_returning_bool() {
    typeck_src("fn lt(a: i32, b: i32) -> bool { return a < b; }")
        .expect("Fix: i32 < i32 -> bool must type-check");
}

#[test]
fn accepts_deref_of_reference() {
    typeck_src("fn read(r: &i32) -> i32 { let x: i32 = *r; return x; }")
        .expect("Fix: *(&i32) -> i32 must type-check");
}

#[test]
fn accepts_unit_function_without_return() {
    typeck_src("fn f() { let x: i32 = 1; }").expect("Fix: a unit function need not return a value");
}

#[test]
fn accepts_if_else_both_returning() {
    typeck_src("fn m(a: i32, b: i32) -> i32 { if a < b { return b; } else { return a; }; }")
        .expect("Fix: if/else that returns on both arms must type-check");
}

#[test]
fn accepts_call_with_matching_argument() {
    typeck_src("fn g(x: i32) -> i32 { return x; } fn f() -> i32 { return g(7); }")
        .expect("Fix: a call with a matching argument type must type-check");
}

// ---- negative twins ----

#[test]
fn rejects_return_type_mismatch() {
    let err = typeck_src("fn f() -> i32 { return true; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::TypeMismatch { .. }),
        "got {err:?}"
    );
    assert!(err.to_string().contains("expected `i32`") && err.to_string().contains("found `bool`"));
}

#[test]
fn rejects_let_type_mismatch() {
    let err = typeck_src("fn f() { let x: bool = 5; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::TypeMismatch { .. }),
        "got {err:?}"
    );
    assert!(err.to_string().contains("let binding"));
}

#[test]
fn rejects_arithmetic_on_bool() {
    let err = typeck_src("fn f() -> i32 { return true + 1; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::TypeMismatch { .. }),
        "got {err:?}"
    );
}

#[test]
fn rejects_deref_of_non_reference() {
    let err = typeck_src("fn f() -> i32 { let x: i32 = 0; return *x; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::CannotDeref { .. }),
        "got {err:?}"
    );
}

#[test]
fn rejects_non_boolean_if_condition() {
    let err = typeck_src("fn f() { if 1 { let x: i32 = 0; }; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::NonBooleanCondition { .. }),
        "got {err:?}"
    );
}

#[test]
fn rejects_wrong_argument_count() {
    let err = typeck_src("fn g(x: i32) -> i32 { return x; } fn f() -> i32 { return g(1, 2); }")
        .unwrap_err();
    assert!(
        matches!(
            err,
            RustSemaError::ArgCountMismatch {
                expected: 1,
                found: 2,
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn rejects_wrong_argument_type() {
    let err = typeck_src("fn g(x: i32) -> i32 { return x; } fn f() -> i32 { return g(true); }")
        .unwrap_err();
    assert!(
        matches!(err, RustSemaError::TypeMismatch { .. }),
        "got {err:?}"
    );
    assert!(err.to_string().contains("function argument"));
}

#[test]
fn rejects_missing_return_in_non_unit_function() {
    let err = typeck_src("fn f() -> i32 { let x: i32 = 0; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::MissingReturn { .. }),
        "got {err:?}"
    );
}
