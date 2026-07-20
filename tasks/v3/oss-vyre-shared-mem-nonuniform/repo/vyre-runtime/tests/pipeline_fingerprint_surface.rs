//! Surface tests for `PipelineFingerprint`.
//!
//! Fingerprints must be deterministic, hex-encodable, and stable for
//! cache key generation.

use vyre::ir::{BufferDecl, DataType, Node, Program};
use vyre_runtime::pipeline_cache::PipelineFingerprint;

#[test]
fn fingerprint_of_empty_program_is_32_bytes() {
    let prog = Program::empty();
    let fp = PipelineFingerprint::of(&prog);
    assert_eq!(fp.0.len(), 32);
}

#[test]
fn fingerprint_is_deterministic() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let a = PipelineFingerprint::of(&prog);
    let b = PipelineFingerprint::of(&prog);
    assert_eq!(a.0, b.0);
}

#[test]
fn fingerprint_changes_when_buffers_change() {
    let prog_a = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let prog_b = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(2)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let a = PipelineFingerprint::of(&prog_a);
    let b = PipelineFingerprint::of(&prog_b);
    assert_ne!(a.0, b.0);
}

#[test]
fn fingerprint_changes_when_nodes_change() {
    let prog_a = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let prog_b = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::barrier(), Node::Return],
    );
    let a = PipelineFingerprint::of(&prog_a);
    let b = PipelineFingerprint::of(&prog_b);
    assert_ne!(a.0, b.0);
}

#[test]
fn fingerprint_hex_is_64_chars() {
    let prog = Program::empty();
    let fp = PipelineFingerprint::of(&prog);
    let hex = fp.hex();
    assert_eq!(hex.len(), 64);
}

#[test]
fn fingerprint_hex_is_lowercase_hex() {
    let prog = Program::empty();
    let fp = PipelineFingerprint::of(&prog);
    let hex = fp.hex();
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn fingerprint_hex_is_deterministic() {
    let prog = Program::empty();
    let a = PipelineFingerprint::of(&prog).hex();
    let b = PipelineFingerprint::of(&prog).hex();
    assert_eq!(a, b);
}
