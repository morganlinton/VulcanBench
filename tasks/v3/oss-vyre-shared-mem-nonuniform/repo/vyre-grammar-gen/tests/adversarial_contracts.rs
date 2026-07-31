//! Adversarial contracts for corrupt generated grammar payloads.

use vyre_grammar_gen::{decode_dfa_from_bytes, wire::WireError};

#[test]
fn adversarial_contract_rejects_corrupt_generated_blob() {
    let err = decode_dfa_from_bytes(b"not-a-generated-grammar-blob")
        .expect_err("adversarial contract must reject corrupt generated blobs");
    // BadMagic is actionable: it reports the 4 bytes that were found so the
    // caller can compare against the expected `SGGC` magic.
    assert!(
        matches!(err, WireError::BadMagic(_) | WireError::TooShort { .. }),
        "adversarial contract must produce a structured WireError, got: {err:?}"
    );
}
