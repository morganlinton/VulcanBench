//! Wire-format contracts for compiled DFA cache blobs.

#![cfg(feature = "matching")]

use vyre_primitives::matching::{dfa_compile, CompiledDfa, DfaWireError};

#[test]
fn dfa_wire_round_trips_all_tables() {
    let dfa = dfa_compile(&[b"AKIA".as_slice(), b"ghp_".as_slice()]);
    let bytes = dfa.to_bytes().expect("encode DFA wire blob");
    let back = CompiledDfa::from_bytes(&bytes).expect("decode DFA wire blob");

    assert_eq!(back.state_count, dfa.state_count);
    assert_eq!(back.transitions, dfa.transitions);
    assert_eq!(back.accept, dfa.accept);
    assert_eq!(back.output_offsets, dfa.output_offsets);
    assert_eq!(back.output_records, dfa.output_records);
}

#[test]
fn dfa_wire_rejects_bad_magic() {
    let mut bytes = dfa_compile(&[b"x".as_slice()])
        .to_bytes()
        .expect("encode DFA wire blob");
    bytes[0] = 0;

    assert!(matches!(
        CompiledDfa::from_bytes(&bytes),
        Err(DfaWireError::BadMagic)
    ));
}

#[test]
fn dfa_wire_rejects_version_mismatch() {
    let mut bytes = dfa_compile(&[b"x".as_slice()])
        .to_bytes()
        .expect("encode DFA wire blob");
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    bytes[4..8].copy_from_slice(&version.wrapping_add(1).to_le_bytes());

    assert!(matches!(
        CompiledDfa::from_bytes(&bytes),
        Err(DfaWireError::VersionMismatch { .. })
    ));
}

#[test]
fn dfa_wire_rejects_truncated_payload() {
    let bytes = dfa_compile(&[b"AKIA".as_slice()])
        .to_bytes()
        .expect("encode DFA wire blob");
    let cut = &bytes[..bytes.len() - 4];

    assert!(matches!(
        CompiledDfa::from_bytes(cut),
        Err(DfaWireError::Truncated { .. })
    ));
}

#[test]
fn dfa_wire_rejects_shape_mismatch_before_body_decode() {
    let mut bytes = dfa_compile(&[b"AKIA".as_slice()])
        .to_bytes()
        .expect("encode DFA wire blob");
    bytes[12..16].copy_from_slice(&0u32.to_le_bytes());

    assert!(matches!(
        CompiledDfa::from_bytes(&bytes),
        Err(DfaWireError::ShapeMismatch { .. })
    ));
}
