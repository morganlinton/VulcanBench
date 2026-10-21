//! Name-resolution tests for the nano-subset (`vyre_libs::parsing::rust::sema::resolve`).
//!
//! Truth assertions on the resolved binding table and use map, plus negative
//! cases for undefined names, scope escape, and unknown function calls.

#![forbid(unsafe_code)]

use std::collections::HashSet;

use vyre_libs::parsing::rust::lex::lexer::core::lex;
use vyre_libs::parsing::rust::parse::parse;
use vyre_libs::parsing::rust::sema::{resolve, BindingId, Resolution, RustSemaError};

fn resolve_src(src: &str) -> Result<Resolution, RustSemaError> {
    let bytes = src.as_bytes();
    let tokens = lex(bytes).expect("Fix: test corpus must lex");
    let module = parse(bytes, &tokens).expect("Fix: test corpus must parse");
    resolve(&module, bytes)
}

#[test]
fn resolves_params_and_lets_in_order() {
    let r = resolve_src("fn add(a: i32, b: i32) -> i32 { let c: i32 = a + b; return c; }").unwrap();
    let names: Vec<&str> = r.bindings.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["a", "b", "c"],
        "Fix: bindings must be params then lets, in declaration order"
    );
    assert_eq!(
        r.uses.len(),
        3,
        "Fix: every variable use must resolve to a binding"
    );
}

#[test]
fn records_mut_flag() {
    let r = resolve_src("fn f() { let mut x: i32 = 0; let y: i32 = x; }").unwrap();
    let x = r
        .bindings
        .iter()
        .find(|b| b.name == "x")
        .expect("Fix: x must be a binding");
    assert!(x.mutable, "Fix: `let mut x` must record mutable=true");
    let y = r
        .bindings
        .iter()
        .find(|b| b.name == "y")
        .expect("Fix: y must be a binding");
    assert!(!y.mutable, "Fix: `let y` must record mutable=false");
}

#[test]
fn rejects_undefined_name() {
    let err = resolve_src("fn f() -> i32 { return y; }").unwrap_err();
    match err {
        RustSemaError::UnresolvedName { name, .. } => assert_eq!(name, "y"),
        other => panic!("Fix: expected UnresolvedName for `y`, got {other:?}"),
    }
}

#[test]
fn rhs_sees_outer_scope_before_shadowing() {
    let r = resolve_src("fn f(x: i32) -> i32 { let x: i32 = x + 1; return x; }").unwrap();
    let x_ids: Vec<BindingId> = r
        .bindings
        .iter()
        .enumerate()
        .filter(|(_, b)| b.name == "x")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        x_ids.len(),
        2,
        "Fix: param x and let x are distinct bindings"
    );
    let targets: HashSet<BindingId> = r.uses.values().copied().collect();
    assert!(
        targets.contains(&0) && targets.contains(&1),
        "Fix: the initializer x must resolve to the param (0) and the return x to the let (1); got {targets:?}"
    );
}

#[test]
fn block_scoped_binding_does_not_escape() {
    let err = resolve_src("fn f() -> i32 { if true { let y: i32 = 1; }; return y; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::UnresolvedName { .. }),
        "Fix: a binding declared in an inner block must not be visible outside it; got {err:?}"
    );
}

#[test]
fn rejects_unknown_function_call() {
    let err = resolve_src("fn f() -> i32 { return g(); }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::UnknownFunction { .. }),
        "Fix: a call to an undeclared function must fail resolution; got {err:?}"
    );
}

#[test]
fn forward_function_reference_resolves() {
    let r = resolve_src("fn f() -> i32 { return g(); } fn g() -> i32 { return 1; }").unwrap();
    assert!(
        r.bindings.is_empty(),
        "Fix: neither function declares a binding"
    );
}
