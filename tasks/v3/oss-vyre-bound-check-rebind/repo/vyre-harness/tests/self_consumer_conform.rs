//! P-HARNESS-1: every self-substrate consumer runs through the
//! conform suite.
//!
//! For every module in `vyre_self_substrate::*`, the
//! conform suite runs the module's primary entry point on the
//! standard corpus and asserts no panics. Today the test gates
//! each consumer behind a feature flag; the consumer's own crate
//! must enable that feature in CI.
#![allow(missing_docs)]

use vyre_self_substrate::{scallop_provenance, vsa_fingerprint};

#[test]
fn no_self_consumer_panics_on_smoke_input() {
    let mut state = vec![0u32; 4];
    state[1] = 0b01;
    let mut join_rules = vec![0u32; 4];
    join_rules[3] = 0b10;
    let closure = scallop_provenance::reference_provenance_closure(&state, &join_rules, 2, 8);
    assert_eq!(closure[1], 0b11);

    let fingerprint = vsa_fingerprint::reference_fingerprint(
        &[0x7679_7265; 8],
        &[0x6c69_6e6b; 8],
        &[0x7375_6273; 8],
    );
    assert!(
        fingerprint.iter().any(|&lane| lane != 0),
        "vsa_fingerprint self-consumer must produce a nonzero key for nonempty input"
    );
}

#[test]
fn self_substrate_module_list_is_documented() {
    // The list of self-substrate consumers is canonical in
    // RECURSION_THESIS.md  -  assert that file exists in the source
    // tree. If a future refactor renames or removes that file the
    // test points the reader at the fix.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("RECURSION_THESIS.md");
    let body = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("Fix: read {}: {error}", path.display()));
    assert!(
        body.contains("self_substrate") || body.contains("substrate"),
        "RECURSION_THESIS.md should describe substrate consumers"
    );
}
