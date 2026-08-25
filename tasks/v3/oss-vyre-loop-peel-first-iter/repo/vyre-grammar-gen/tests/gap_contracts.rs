//! Gap contracts for empty generated LR payload handling.

use vyre_grammar_gen::{decode_lr_from_bytes, wire::WireError};

#[test]
fn gap_contract_rejects_empty_generated_lr_payload() {
    let err = decode_lr_from_bytes(&[])
        .expect_err("gap contract must reject empty generated LR payloads");
    // TooShort is actionable: it reports need vs got so the caller knows why.
    assert!(
        matches!(err, WireError::TooShort { .. }),
        "gap contract must produce TooShort for empty input, got: {err:?}"
    );
}
