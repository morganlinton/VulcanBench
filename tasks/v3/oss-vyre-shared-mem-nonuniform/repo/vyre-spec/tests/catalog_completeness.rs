//! Freeze test for the spec catalog completeness invariant.
//!
//! `catalog_is_complete` must return true; a false result means a
//! critical invariant was broken during a spec edit.

use vyre_spec::catalog_is_complete;

#[test]
fn catalog_is_complete_returns_true() {
    assert!(
        catalog_is_complete(),
        "vyre-spec catalog must be complete; a false here means a missing or duplicate invariant id"
    );
}

#[test]
fn catalog_completeness_is_idempotent() {
    let first = catalog_is_complete();
    let second = catalog_is_complete();
    assert_eq!(first, second, "catalog_is_complete must be deterministic");
}
