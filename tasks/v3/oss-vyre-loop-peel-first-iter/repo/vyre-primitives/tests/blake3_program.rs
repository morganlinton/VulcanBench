//! Adversarial tests for BLAKE3 program construction.
//!
//! `blake3_g_program` and `blake3_round_program` emit IR programs that
//! must validate, round-trip through the wire format, and declare the
//! correct buffer shapes.

use vyre_foundation::ir::{BufferAccess, Program};
use vyre_primitives::hash::blake3::{blake3_g_program, blake3_round_program, MSG_SCHEDULE};

#[test]
fn blake3_g_program_has_three_buffers() {
    let prog = blake3_g_program("state", "message", "out");
    assert_eq!(prog.buffers().len(), 3);
}

#[test]
fn blake3_g_program_state_buffer_is_readonly() {
    let prog = blake3_g_program("state", "message", "out");
    let state = prog.buffer("state").expect("state buffer must exist");
    assert_eq!(state.access(), BufferAccess::ReadOnly);
    assert_eq!(state.count(), 16);
}

#[test]
fn blake3_g_program_message_buffer_is_readonly() {
    let prog = blake3_g_program("state", "message", "out");
    let msg = prog.buffer("message").expect("message buffer must exist");
    assert_eq!(msg.access(), BufferAccess::ReadOnly);
    assert_eq!(msg.count(), 2);
}

#[test]
fn blake3_g_program_out_buffer_is_readwrite() {
    let prog = blake3_g_program("state", "message", "out");
    let out = prog.buffer("out").expect("out buffer must exist");
    assert_eq!(out.access(), BufferAccess::ReadWrite);
    assert_eq!(out.count(), 16);
}

#[test]
fn blake3_g_program_wire_roundtrips() {
    let prog = blake3_g_program("state", "message", "out");
    let bytes = prog.to_wire().expect("blake3_g_program must encode");
    let decoded = Program::from_wire(&bytes).expect("blake3_g_program must decode");
    assert!(prog.structural_eq(&decoded));
}

#[test]
fn blake3_round_program_has_three_buffers() {
    let prog = blake3_round_program("state", "message", "out");
    assert_eq!(prog.buffers().len(), 3);
}

#[test]
fn blake3_round_program_buffers_are_count_16() {
    let prog = blake3_round_program("state", "message", "out");
    for buf in prog.buffers() {
        assert_eq!(buf.count(), 16, "buffer {} must have count 16", buf.name());
    }
}

#[test]
fn blake3_round_program_wire_roundtrips() {
    let prog = blake3_round_program("state", "message", "out");
    let bytes = prog.to_wire().expect("blake3_round_program must encode");
    let decoded = Program::from_wire(&bytes).expect("blake3_round_program must decode");
    assert!(prog.structural_eq(&decoded));
}

#[test]
fn msg_schedule_has_seven_rounds() {
    assert_eq!(MSG_SCHEDULE.len(), 7);
}

#[test]
fn msg_schedule_each_round_has_sixteen_words() {
    for (i, round) in MSG_SCHEDULE.iter().enumerate() {
        assert_eq!(round.len(), 16, "round {i} must have 16 words");
    }
}

#[test]
fn msg_schedule_is_a_permutation_each_round() {
    for (i, round) in MSG_SCHEDULE.iter().enumerate() {
        let mut sorted = round.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            (0..16).collect::<Vec<_>>(),
            "round {i} must be a permutation of 0..16"
        );
    }
}

#[test]
fn blake3_round_program_canonicalizes_to_same_hash() {
    let prog = blake3_round_program("state", "message", "out");
    let hash1 = prog
        .canonical_wire_hash()
        .expect("canonical hash must succeed");
    let hash2 = prog
        .canonical_wire_hash()
        .expect("canonical hash must succeed");
    assert_eq!(hash1, hash2, "canonical hash must be deterministic");
}
