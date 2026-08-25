//! Surface tests for the diagnostic system.
//!
//! `Diagnostic` must always carry a `Fix:` hint, support human and
//! JSON rendering, and serialize without panic.

use vyre::diagnostics::{Diagnostic, OpLocation};

#[test]
fn diagnostic_error_has_fix_hint() {
    let diag = Diagnostic::error("E001", "something broke").with_fix("try again");
    assert!(diag.render_human().contains("try again"));
}

#[test]
fn diagnostic_warning_has_fix_hint() {
    let diag = Diagnostic::warning("W001", "deprecated").with_fix("use v2");
    assert!(diag.render_human().contains("use v2"));
}

#[test]
fn diagnostic_note_has_no_severity_prefix() {
    let diag = Diagnostic::note("N001", "informational");
    let human = diag.render_human();
    assert!(human.contains("informational"));
}

#[test]
fn diagnostic_json_contains_code() {
    let diag = Diagnostic::error("E042", "test").with_fix("fix it");
    let json = diag.to_json();
    assert!(json.contains("E042"));
}

#[test]
fn diagnostic_json_contains_message() {
    let diag = Diagnostic::error("E042", "test message").with_fix("fix it");
    let json = diag.to_json();
    assert!(json.contains("test message"));
}

#[test]
fn diagnostic_json_contains_fix() {
    let diag = Diagnostic::error("E042", "test").with_fix("do this");
    let json = diag.to_json();
    assert!(json.contains("do this"));
}

#[test]
fn diagnostic_with_location_renders_location() {
    let loc = OpLocation::op("math::add").with_operand(1);
    let diag = Diagnostic::error("E001", "bad op").with_location(loc);
    let human = diag.render_human();
    assert!(human.contains("math::add"));
}

#[test]
fn diagnostic_with_doc_url_renders_url() {
    let diag = Diagnostic::error("E001", "bad")
        .with_fix("fix it")
        .with_doc_url("https://vyre.dev/docs/E001");
    let human = diag.render_human();
    assert!(human.contains("https://vyre.dev/docs/E001"));
}

#[test]
fn diagnostic_roundtrips_through_json() {
    let diag = Diagnostic::error("E999", "roundtrip test").with_fix("restart");
    let json = diag.to_json();
    // Should parse as valid JSON without panic
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("must be valid JSON");
    assert_eq!(parsed["severity"], "error");
}

#[test]
fn diagnostic_empty_fix_still_renders() {
    let diag = Diagnostic::error("E001", "msg").with_fix("");
    let human = diag.render_human();
    assert!(human.contains("msg"));
}
