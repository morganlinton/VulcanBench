//! Escape / dangling-reference tests (rustc E0597) for the nano-subset.
//!
//! Truth assertions on `vyre_libs::parsing::rust::sema::check_escape`: returning
//! a reference to a call-local value is rejected; returning a parameter-derived
//! reference is allowed. Verdicts match rustc's E0597 behavior.

#![forbid(unsafe_code)]

use vyre_libs::parsing::rust::lex::lexer::core::lex;
use vyre_libs::parsing::rust::parse::parse;
use vyre_libs::parsing::rust::sema::{check_escape, resolve, RustSemaError};

fn check_src(src: &str) -> Result<(), RustSemaError> {
    let bytes = src.as_bytes();
    let tokens = lex(bytes).expect("Fix: test corpus must lex");
    let module = parse(bytes, &tokens).expect("Fix: test corpus must parse");
    let resolution = resolve(&module, bytes).expect("Fix: test corpus must resolve");
    check_escape(&module, &resolution)
}

#[test]
fn rejects_returning_reference_to_local_let() {
    let err = check_src("fn f() -> &i32 { let x: i32 = 0; return &x; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::ReturnsReferenceToLocal { .. }),
        "got {err:?}"
    );
}

#[test]
fn rejects_returning_reference_to_value_parameter() {
    let err = check_src("fn f(x: i32) -> &i32 { return &x; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::ReturnsReferenceToLocal { .. }),
        "got {err:?}"
    );
}

#[test]
fn rejects_returning_local_borrow_indirectly() {
    let err =
        check_src("fn f() -> &i32 { let x: i32 = 0; let a: &i32 = &x; return a; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::ReturnsReferenceToLocal { .. }),
        "got {err:?}"
    );
}

#[test]
fn allows_returning_reference_parameter() {
    check_src("fn f(r: &i32) -> &i32 { return r; }")
        .expect("Fix: returning a reference parameter is sound");
}

#[test]
fn allows_returning_deref_borrow_of_parameter() {
    check_src("fn f(r: &i32) -> &i32 { return &*r; }")
        .expect("Fix: &*r through a reference parameter is sound");
}

#[test]
fn allows_returning_parameter_derived_let() {
    check_src("fn f(r: &i32) -> &i32 { let a: &i32 = r; return a; }")
        .expect("Fix: a let aliasing a reference parameter is sound");
}

#[test]
fn allows_value_return_without_reference() {
    check_src("fn f() -> i32 { let x: i32 = 0; return x; }")
        .expect("Fix: returning a value (not a reference) never escapes");
}
