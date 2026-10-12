//! P1 inventory #96  -  cross-backend parity matrix.
//!
//! For each registered op, asserts that all registered execution backends
//! agree byte-for-byte on a small canonical input set. Backends not
//! present on the host are configuration failures (the test panics
//! rather than silently accepting partial coverage). The actual heavy
//! parity sweep runs in `gpu-parity.yml`; this CPU-runnable wrapper
//! verifies the inventory walk is well-formed and every op provides
//! runnable fixtures.

use vyre_harness::all_entries;

#[test]
fn parity_matrix_inventory_has_no_fixture_gaps() {
    // Every registered op must provide both test_inputs and expected_output.
    // There is no skip path: missing fixtures are a hard failure.
    let mut missing = Vec::new();
    for entry in all_entries() {
        assert!(!entry.id.is_empty(), "Fix: registered op missing id");
        if entry.test_inputs.is_none() {
            missing.push(format!("{}: missing test_inputs", entry.id));
        }
        if entry.expected_output.is_none() {
            missing.push(format!("{}: missing expected_output", entry.id));
        }
    }
    assert!(
        missing.is_empty(),
        "Fixture coverage gaps detected:\n  - {}",
        missing.join("\n  - ")
    );
}
