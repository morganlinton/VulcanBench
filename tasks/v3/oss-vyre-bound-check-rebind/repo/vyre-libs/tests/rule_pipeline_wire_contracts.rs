//! RulePipeline wire-cache contracts for the NFA mega-scan path.

#![cfg(feature = "matching-nfa")]

use vyre_foundation::serial::envelope::EnvelopeError;
use vyre_libs::scan::{build_rule_pipeline, MatchEngineCache, PipelineWireError, RulePipeline};

#[test]
fn rule_pipeline_wire_round_trips_basic_pipeline() {
    let pipe = build_rule_pipeline(&["abc", "def"], "input", "hits", 32);
    let bytes = pipe.to_bytes().expect("pipeline must encode");
    let back = RulePipeline::from_bytes(&bytes).expect("pipeline must decode");

    assert_eq!(back.plan.num_states, pipe.plan.num_states);
    assert_eq!(back.plan.input_len, pipe.plan.input_len);
    assert_eq!(back.plan.accept_states, pipe.plan.accept_states);
    assert_eq!(back.plan.accept_state_ids, pipe.plan.accept_state_ids);
    assert_eq!(back.transition_table, pipe.transition_table);
    assert_eq!(back.epsilon_table, pipe.epsilon_table);
}

#[test]
fn rule_pipeline_trait_wire_metadata_matches_encoded_envelope() {
    let pipe = build_rule_pipeline(&["abc", "def"], "input", "hits", 32);
    let bytes = pipe.to_bytes().expect("pipeline must encode");

    assert_eq!(
        <RulePipeline as MatchEngineCache>::WIRE_MAGIC,
        bytes[0..4],
        "MatchEngineCache magic must match the serialized RulePipeline envelope"
    );
    assert_eq!(
        <RulePipeline as MatchEngineCache>::WIRE_VERSION,
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        "MatchEngineCache version must match the serialized RulePipeline envelope"
    );
}

#[test]
fn rule_pipeline_wire_rejects_bad_magic() {
    let pipe = build_rule_pipeline(&["abc"], "input", "hits", 16);
    let mut bytes = pipe.to_bytes().expect("pipeline must encode");
    bytes[0] = 0;

    match RulePipeline::from_bytes(&bytes) {
        Err(PipelineWireError::WireFraming(EnvelopeError::BadMagic { .. })) => {}
        other => panic!("expected BadMagic, got {other:?}"),
    }
}

#[test]
fn rule_pipeline_wire_rejects_version_mismatch() {
    let pipe = build_rule_pipeline(&["abc"], "input", "hits", 16);
    let mut bytes = pipe.to_bytes().expect("pipeline must encode");
    bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());

    match RulePipeline::from_bytes(&bytes) {
        Err(PipelineWireError::WireFraming(EnvelopeError::VersionMismatch { .. })) => {}
        other => panic!("expected VersionMismatch, got {other:?}"),
    }
}
