//! Boundary tests for the canonical vyre IR text format.
//!
//! `Program::to_text` and `Program::from_text` must round-trip for
//! every valid program and reject malformed inputs with structured
//! errors rather than panics.

use vyre::ir::{BufferDecl, DataType, Node, Program};
use vyre_foundation::serial::text::TEXT_FORMAT_HEADER;

#[test]
fn text_roundtrip_minimal_program() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let text = prog.to_text().expect("must encode to text");
    let decoded = Program::from_text(&text).expect("must decode from text");
    assert!(prog.structural_eq(&decoded));
}

#[test]
fn text_roundtrip_empty_program() {
    let prog = Program::empty();
    let text = prog.to_text().expect("must encode to text");
    let decoded = Program::from_text(&text).expect("must decode from text");
    assert!(prog.structural_eq(&decoded));
}

#[test]
fn text_starts_with_header() {
    let prog = Program::empty();
    let text = prog.to_text().expect("must encode");
    assert!(text.starts_with(TEXT_FORMAT_HEADER));
}

#[test]
fn text_contains_wire_bytes_line() {
    let prog = Program::empty();
    let text = prog.to_text().expect("must encode");
    assert!(text.contains("wire_bytes"));
}

#[test]
fn text_rejects_missing_header() {
    let err = Program::from_text("not_a_header\nwire_bytes 0\n").unwrap_err();
    assert!(err.message().contains("header"));
}

#[test]
fn text_rejects_empty_input() {
    let err = Program::from_text("").unwrap_err();
    // Empty input fails with MissingHeader because lines().next() is None
    assert!(err.fix_hint().contains("Fix:"));
}

#[test]
fn text_rejects_wrong_wire_bytes_line() {
    let text = format!("{TEXT_FORMAT_HEADER}\nnot_wire_bytes 42\n");
    let err = Program::from_text(&text).unwrap_err();
    assert!(err.message().contains("wire_bytes"));
}

#[test]
fn text_rejects_truncated_body() {
    let prog = Program::empty();
    let text = prog.to_text().expect("must encode");
    // Truncate after the header + wire_bytes line
    let truncated: String = text.lines().take(2).collect::<Vec<_>>().join("\n");
    let err = Program::from_text(&(truncated + "\n")).unwrap_err();
    assert!(
        err.message().contains("truncat")
            || err.message().contains("hex")
            || err.message().contains("Fix:")
    );
}

#[test]
fn text_rejects_malformed_hex() {
    let text = format!("{TEXT_FORMAT_HEADER}\nwire_bytes 1\nzz\n");
    let err = Program::from_text(&text).unwrap_err();
    assert!(err.message().contains("hex") || err.message().contains("Fix:"));
}

#[test]
fn text_rejects_negative_wire_bytes() {
    let text = format!("{TEXT_FORMAT_HEADER}\nwire_bytes -1\n");
    let err = Program::from_text(&text).unwrap_err();
    assert!(err.message().contains("wire_bytes") || err.message().contains("Fix:"));
}

#[test]
fn text_rejects_garbage_after_valid_body() {
    let prog = Program::empty();
    let text = prog.to_text().expect("must encode");
    let with_garbage = text + "garbage\n";
    let err = Program::from_text(&with_garbage).unwrap_err();
    // Extra bytes should be rejected as trailing data
    assert!(err.message().contains("Fix:") || err.message().contains("trailing"));
}

#[test]
fn text_error_contains_fix_hint() {
    let err = Program::from_text("bad").unwrap_err();
    assert!(err.fix_hint().contains("Fix:"));
}

#[test]
fn text_deterministic_for_same_program() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let text1 = prog.to_text().expect("must encode");
    let text2 = prog.to_text().expect("must encode");
    assert_eq!(text1, text2);
}
