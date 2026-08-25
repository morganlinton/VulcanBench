//! Mutability borrow-rule tests (rustc E0596) for the nano-subset.
//!
//! Truth assertions on `vyre_libs::parsing::rust::sema::check_mutability`: a
//! `&mut` borrow of an immutable place is rejected; of a mutable place is
//! allowed. Verdicts match rustc's E0596 behavior on the same programs.

#![forbid(unsafe_code)]

use vyre_libs::parsing::rust::lex::lexer::core::lex;
use vyre_libs::parsing::rust::parse::parse;
use vyre_libs::parsing::rust::sema::{check_mutability, resolve, RustSemaError};

fn check_src(src: &str) -> Result<(), RustSemaError> {
    let bytes = src.as_bytes();
    let tokens = lex(bytes).expect("Fix: test corpus must lex");
    let module = parse(bytes, &tokens).expect("Fix: test corpus must parse");
    let resolution = resolve(&module, bytes).expect("Fix: test corpus must resolve");
    check_mutability(&module, &resolution)
}

#[test]
fn rejects_mut_borrow_of_immutable_binding() {
    let err = check_src("fn f() { let x: i32 = 0; let r: &mut i32 = &mut x; }").unwrap_err();
    match err {
        RustSemaError::CannotBorrowImmutableAsMutable { name, .. } => assert_eq!(name, "x"),
        other => panic!("Fix: expected E0596 on `x`, got {other:?}"),
    }
}

#[test]
fn allows_mut_borrow_of_mut_binding() {
    check_src("fn f() { let mut x: i32 = 0; let r: &mut i32 = &mut x; }")
        .expect("Fix: &mut of a `mut` binding must be allowed");
}

#[test]
fn rejects_mut_deref_of_shared_reference() {
    let err = check_src("fn f(r: &i32) { let m: &mut i32 = &mut *r; }").unwrap_err();
    match err {
        RustSemaError::CannotBorrowImmutableAsMutable { name, .. } => assert_eq!(name, "r"),
        other => panic!("Fix: expected E0596 on `*r`, got {other:?}"),
    }
}

#[test]
fn allows_mut_deref_of_mut_reference() {
    check_src("fn f(r: &mut i32) { let m: &mut i32 = &mut *r; }")
        .expect("Fix: &mut *r through a &mut reference must be allowed");
}

#[test]
fn allows_shared_borrow_of_immutable_binding() {
    check_src("fn f() { let x: i32 = 0; let r: &i32 = &x; }")
        .expect("Fix: a shared borrow of an immutable binding must be allowed");
}
