//! Wire-format contracts for high-level literal-set cache blobs.

#![allow(deprecated)]
#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
use vyre_foundation::serial::envelope::EnvelopeError;
#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
use vyre_libs::scan::{GpuLiteralSet, LiteralSetWireError};

#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
#[test]
fn literal_set_wire_round_trips_patterns_and_cpu_scan() {
    let original = GpuLiteralSet::compile(&[
        b"AKIA".as_slice(),
        b"ghp_".as_slice(),
        b"sk_live_".as_slice(),
    ]);
    let bytes = original.to_bytes().expect("encode literal-set wire blob");
    let back = GpuLiteralSet::from_bytes(&bytes).expect("decode literal-set wire blob");

    assert_eq!(back.pattern_offsets, original.pattern_offsets);
    assert_eq!(back.pattern_lengths, original.pattern_lengths);
    assert_eq!(back.pattern_bytes, original.pattern_bytes);
    assert_eq!(back.dfa.state_count, original.dfa.state_count);
    assert_eq!(
        back.program
            .buffers()
            .iter()
            .map(|buffer| buffer.name())
            .collect::<Vec<_>>(),
        vec![
            "haystack",
            "transitions",
            "output_offsets",
            "output_records",
            "pattern_lengths",
            "haystack_len",
            "match_count",
            "candidate_end_mask",
            "candidate_suffix2_mask",
            "candidate_suffix3_bloom",
            "matches"
        ],
        "literal-set wire decode must rebuild the suffix-prefiltered bounded-DFA dispatch surface"
    );

    let haystack = b"foo AKIA ghp_xxxx bar";
    assert_eq!(
        back.reference_scan(haystack),
        original.reference_scan(haystack)
    );
}

#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
#[test]
fn literal_set_wire_rejects_bad_magic() {
    let mut bytes = GpuLiteralSet::compile(&[b"x".as_slice()])
        .to_bytes()
        .expect("encode literal-set wire blob");
    bytes[0] = 0;

    assert!(matches!(
        GpuLiteralSet::from_bytes(&bytes),
        Err(LiteralSetWireError::WireFraming(
            EnvelopeError::BadMagic { .. }
        ))
    ));
}

#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
#[test]
fn literal_set_wire_accepts_v1_literal_compare_cache_by_rebuilding_program() {
    let original = GpuLiteralSet::compile(&[b"abc".as_slice(), b"bc".as_slice()]);
    let mut bytes = original.to_bytes().expect("encode literal-set wire blob");
    bytes[4..8].copy_from_slice(&1u32.to_le_bytes());

    let back = GpuLiteralSet::from_bytes(&bytes).expect("decode legacy literal-set wire blob");

    assert_eq!(back.pattern_lengths, original.pattern_lengths);
    assert_eq!(
        back.reference_scan(b"zabc"),
        original.reference_scan(b"zabc")
    );
    assert!(
        back.program
            .buffers()
            .iter()
            .any(|buffer| buffer.name() == "candidate_suffix3_bloom"),
        "legacy literal-set cache decode must rebuild the current suffix-prefiltered DFA-table program"
    );
}

#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
#[test]
fn literal_set_wire_accepts_v2_bounded_dfa_cache_by_rebuilding_program() {
    let original = GpuLiteralSet::compile(&[b"BEGIN".as_slice(), b"token".as_slice()]);
    let mut bytes = original.to_bytes().expect("encode literal-set wire blob");
    bytes[4..8].copy_from_slice(&2u32.to_le_bytes());

    let back = GpuLiteralSet::from_bytes(&bytes).expect("decode v2 literal-set wire blob");

    assert_eq!(
        back.reference_scan(b"BEGIN token"),
        original.reference_scan(b"BEGIN token")
    );
    assert!(
        back.program
            .buffers()
            .iter()
            .any(|buffer| buffer.name() == "candidate_suffix3_bloom"),
        "v2 literal-set cache decode must rebuild the current suffix-prefiltered DFA-table program"
    );
}

#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
#[test]
fn literal_set_wire_rejects_version_mismatch() {
    let mut bytes = GpuLiteralSet::compile(&[b"x".as_slice()])
        .to_bytes()
        .expect("encode literal-set wire blob");
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    bytes[4..8].copy_from_slice(&version.wrapping_add(1).to_le_bytes());

    assert!(matches!(
        GpuLiteralSet::from_bytes(&bytes),
        Err(LiteralSetWireError::WireFraming(
            EnvelopeError::VersionMismatch { .. }
        ))
    ));
}
