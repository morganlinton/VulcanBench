//! Adversarial tests for DEFLATE stored-block inflate program construction.
//!
//! `inflate_stored` emits an IR program that must validate, round-trip
//! through the wire format, and declare correct buffer access modes.

use vyre_foundation::ir::{BufferAccess, Program};
use vyre_primitives::decode::inflate::inflate_stored;

#[test]
fn inflate_stored_has_three_buffers() {
    let prog = inflate_stored("input", "output", "len", 10);
    assert_eq!(prog.buffers().len(), 3);
}

#[test]
fn inflate_stored_input_is_readonly() {
    let prog = inflate_stored("input", "output", "len", 10);
    let buf = prog.buffer("input").expect("input buffer must exist");
    assert_eq!(buf.access(), BufferAccess::ReadOnly);
    assert_eq!(buf.count(), 10);
}

#[test]
fn inflate_stored_output_is_write_only() {
    let prog = inflate_stored("input", "output", "len", 10);
    let buf = prog.buffer("output").expect("output buffer must exist");
    assert!(buf.is_output());
    assert_eq!(buf.count(), 10);
}

#[test]
fn inflate_stored_len_is_readwrite() {
    let prog = inflate_stored("input", "output", "len", 10);
    let buf = prog.buffer("len").expect("len buffer must exist");
    assert_eq!(buf.access(), BufferAccess::ReadWrite);
    assert_eq!(buf.count(), 1);
}

#[test]
fn inflate_stored_wire_roundtrips() {
    let prog = inflate_stored("input", "output", "len", 10);
    let bytes = prog.to_wire().expect("inflate_stored must encode");
    let decoded = Program::from_wire(&bytes).expect("inflate_stored must decode");
    assert!(prog.structural_eq(&decoded));
}

#[test]
fn inflate_stored_workgroup_size_is_64_1_1() {
    let prog = inflate_stored("input", "output", "len", 10);
    assert_eq!(prog.workgroup_size(), [64, 1, 1]);
}

#[test]
fn inflate_stored_with_zero_input_len_is_constructible() {
    // The validator may reject this, but construction must succeed.
    let prog = inflate_stored("input", "output", "len", 0);
    assert_eq!(prog.buffer("input").unwrap().count(), 0);
    assert_eq!(prog.buffer("output").unwrap().count(), 0);
}

#[test]
fn inflate_stored_with_max_u32_input_len_is_constructible() {
    let prog = inflate_stored("input", "output", "len", u32::MAX);
    assert_eq!(prog.buffer("input").unwrap().count(), u32::MAX);
    assert_eq!(prog.buffer("output").unwrap().count(), u32::MAX);
}
