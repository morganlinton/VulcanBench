//! Conflicting-borrow tests (rustc E0499 / E0502) for the nano-subset.
//!
//! Truth assertions on `vyre_libs::parsing::rust::sema::check_conflicts`: two
//! live mutable borrows of one place, or a mutable plus a live shared borrow,
//! are rejected; two shared borrows and NLL-dead borrows are allowed. Verdicts
//! match rustc on straight-line programs.

#![forbid(unsafe_code)]

use vyre_libs::parsing::rust::lex::lexer::core::lex;
use vyre_libs::parsing::rust::parse::parse;
use vyre_libs::parsing::rust::sema::{check_conflicts, resolve, RustSemaError};

fn check_src(src: &str) -> Result<(), RustSemaError> {
    let bytes = src.as_bytes();
    let tokens = lex(bytes).expect("Fix: test corpus must lex");
    let module = parse(bytes, &tokens).expect("Fix: test corpus must parse");
    let resolution = resolve(&module, bytes).expect("Fix: test corpus must resolve");
    check_conflicts(&module, &resolution)
}

#[test]
fn rejects_two_live_mutable_borrows() {
    let err = check_src("fn f() { let mut x: i32 = 0; let a: &mut i32 = &mut x; let b: &mut i32 = &mut x; let c: i32 = *a + *b; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::MultipleMutableBorrows { .. }),
        "got {err:?}"
    );
}

#[test]
fn rejects_mutable_while_shared_live() {
    let err = check_src("fn f() { let mut x: i32 = 0; let a: &i32 = &x; let b: &mut i32 = &mut x; let c: i32 = *a; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::MutableAndSharedBorrow { .. }),
        "got {err:?}"
    );
}

#[test]
fn allows_two_shared_borrows() {
    check_src(
        "fn f() { let x: i32 = 0; let a: &i32 = &x; let b: &i32 = &x; let c: i32 = *a + *b; }",
    )
    .expect("Fix: two shared borrows of the same place must be allowed");
}

#[test]
fn allows_sequential_non_overlapping_mutable_borrows() {
    check_src("fn f() { let mut x: i32 = 0; let a: &mut i32 = &mut x; let p: i32 = *a; let b: &mut i32 = &mut x; let q: i32 = *b; }")
        .expect("Fix: sequential non-overlapping &mut borrows must be allowed (NLL)");
}

#[test]
fn allows_unused_first_mutable_borrow() {
    check_src("fn f() { let mut x: i32 = 0; let a: &mut i32 = &mut x; let b: &mut i32 = &mut x; let c: i32 = *b; }")
        .expect("Fix: a never-used borrow is dead immediately (NLL)");
}

#[test]
fn rejects_conflict_live_across_a_branch_point() {
    // a and b are both created before the `if`, then used in separate arms; at
    // b's creation a is still live -> E0499 (matches rustc).
    let err = check_src("fn f() { let mut x: i32 = 0; let a: &mut i32 = &mut x; let b: &mut i32 = &mut x; if true { let p: i32 = *a; } else { let q: i32 = *b; }; }").unwrap_err();
    assert!(
        matches!(err, RustSemaError::MultipleMutableBorrows { .. }),
        "got {err:?}"
    );
}

#[test]
fn allows_borrows_confined_to_exclusive_branches() {
    // Each &mut x lives only within its own arm; they are never co-live.
    check_src("fn f() { let mut x: i32 = 0; if true { let a: &mut i32 = &mut x; let p: i32 = *a; } else { let b: &mut i32 = &mut x; let q: i32 = *b; }; }")
        .expect("Fix: borrows confined to mutually exclusive branches must be allowed");
}
