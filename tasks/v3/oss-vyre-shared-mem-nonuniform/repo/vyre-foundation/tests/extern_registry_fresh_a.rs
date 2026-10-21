//! Side-state leak probe A.
//!
//! Registers a dialect unique to this binary. Binary B must NOT see it.
//! This binary validates that its own registration is visible locally.

use vyre_foundation::extern_registry::{dialects, ops_in_dialect, ExternDialect, ExternOp};

inventory::submit! {
    ExternDialect::new("vyre-libs-fresh-a", "0.1.0", "https://example.invalid/fresh-a")
}

inventory::submit! {
    ExternOp::new("vyre-libs-fresh-a", "vyre-libs-fresh-a::probe")
}

#[test]
fn binary_a_sees_its_own_registration() {
    let names: Vec<_> = dialects().iter().map(|d| d.name).collect();
    assert!(
        names.contains(&"vyre-libs-fresh-a"),
        "Binary A must see its own registration"
    );
    assert_eq!(
        ops_in_dialect("vyre-libs-fresh-a").len(),
        1,
        "Binary A must see its own op"
    );
}
