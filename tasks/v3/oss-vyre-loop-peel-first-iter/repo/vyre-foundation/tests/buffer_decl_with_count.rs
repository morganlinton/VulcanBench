//! Adversarial tests for zero-count BufferDecl construction.
//!
//! A zero-length static buffer is a validation failure on every shipped
//! backend. Constructors keep host code non-panicking; validation owns the
//! dispatch boundary.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType};

#[test]
fn with_count_zero_preserves_runtime_sized_encoding() {
    let buf = BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32).with_count(0);
    assert_eq!(buf.count(), 0);
}

#[test]
fn with_count_one_succeeds_and_stores_exactly_one() {
    let buf = BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1);
    assert_eq!(buf.count(), 1);
}

#[test]
fn with_count_max_u32_succeeds() {
    // Adversarial: very large counts are legal. If the field were ever
    // narrowed to a smaller type or if an over-zealous upper-bound check
    // were added, this test would fail.
    let buf =
        BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32).with_count(u32::MAX);
    assert_eq!(buf.count(), u32::MAX);
}

#[test]
fn workgroup_zero_count_is_constructible_for_validator_rejection() {
    let buf = BufferDecl::workgroup("scratch", 0, DataType::U32);
    assert_eq!(buf.count(), 0);
    assert_eq!(buf.access(), BufferAccess::Workgroup);
}

#[test]
fn workgroup_positive_count_succeeds() {
    let buf = BufferDecl::workgroup("scratch", 64, DataType::U32);
    assert_eq!(buf.count(), 64);
    assert_eq!(buf.access(), BufferAccess::Workgroup);
}

#[test]
fn post_construction_mutation_to_zero_is_caught_by_validator_not_constructor() {
    // The Program-level validator catches the zero case before dispatch.
    let mut buf = BufferDecl::workgroup("scratch", 64, DataType::U32);
    buf.count = 0;
    assert_eq!(buf.count(), 0, "direct field mutation is permitted today");
    // The validator would reject this program. We don't construct one
    // here because the finding is specifically about the constructor
    // guard surface; validator coverage is exercised elsewhere in the
    // validate tests.
}
