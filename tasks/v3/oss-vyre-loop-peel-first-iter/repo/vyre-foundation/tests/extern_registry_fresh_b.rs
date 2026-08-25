//! Side-state leak probe B.
//!
//! Verifies that registrations from binary A do NOT leak into this binary.
//! If inventory::collect state leaks across test binaries, this test will fail.

use vyre_foundation::extern_registry::{dialects, ops_in_dialect, ExternDialect, ExternOp};

inventory::submit! {
    ExternDialect::new("vyre-libs-fresh-b", "0.1.0", "https://example.invalid/fresh-b")
}

inventory::submit! {
    ExternOp::new("vyre-libs-fresh-b", "vyre-libs-fresh-b::probe")
}

#[test]
fn binary_b_does_not_see_binary_a_state() {
    let names: Vec<_> = dialects().iter().map(|d| d.name).collect();

    assert!(
        !names.contains(&"vyre-libs-fresh-a"),
        "SIDE-STATE LEAK: Binary B sees 'vyre-libs-fresh-a' registered by binary A. \
         inventory::collect must be isolated per test binary / process."
    );

    assert!(
        names.contains(&"vyre-libs-fresh-b"),
        "Binary B must see its own registration"
    );

    assert!(
        ops_in_dialect("vyre-libs-fresh-a").is_empty(),
        "SIDE-STATE LEAK: ops_in_dialect found ops from binary A in binary B"
    );

    assert_eq!(
        ops_in_dialect("vyre-libs-fresh-b").len(),
        1,
        "Binary B must see its own op"
    );
}
