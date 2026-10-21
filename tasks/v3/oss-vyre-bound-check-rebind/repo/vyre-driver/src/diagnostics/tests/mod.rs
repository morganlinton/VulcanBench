use crate::error::Error;

use super::legacy::split_fix;
use super::*;

#[test]
fn severity_labels() {
    assert_eq!(Severity::Error.label(), "error");
    assert_eq!(Severity::Warning.label(), "warning");
    assert_eq!(Severity::Note.label(), "note");
}

#[test]
fn split_fix_basic() {
    let (msg, fix) =
        split_fix("IR inlining cycle at operation `foo`. Fix: do the thing.".to_owned());
    assert_eq!(msg, "IR inlining cycle at operation `foo`");
    assert_eq!(fix.as_deref(), Some("do the thing"));
}

#[test]
fn split_fix_absent() {
    let (msg, fix) = split_fix("no fix hint".to_owned());
    assert_eq!(msg, "no fix hint");
    assert!(fix.is_none());
}

#[test]
fn render_inline_cycle() {
    let err = Error::InlineCycle {
        op_id: "foo".to_owned(),
    };
    let diag = Diagnostic::from(&err);
    assert_eq!(diag.severity, Severity::Error);
    assert_eq!(diag.code.as_str(), "E-INLINE-CYCLE");
    assert!(diag.location.is_some());
    assert_eq!(diag.location.as_ref().unwrap().op_id.as_ref(), "foo");
    assert!(diag.suggested_fix.is_some());
    let rendered = diag.render_human();
    assert!(rendered.starts_with("error[E-INLINE-CYCLE]:"));
    assert!(rendered.contains("op `foo`"));
    assert!(rendered.contains("help:"));
}

#[test]
fn json_round_trip() {
    let diag = Diagnostic::error("E-TEST", "boom")
        .with_location(OpLocation::op("math.add").with_operand(1))
        .with_fix("do the thing")
        .with_doc_url("https://docs.vyre.dev/E-TEST");
    let j = diag.to_json();
    let back: Diagnostic = serde_json::from_str(&j).unwrap();
    assert_eq!(back, diag);
}

#[test]
fn every_error_variant_classifies() {
    let samples = [
        Error::InlineCycle { op_id: "a".into() },
        Error::InlineUnknownOp { op_id: "a".into() },
        Error::InlineNonInlinable { op_id: "a".into() },
        Error::InlineArgCountMismatch {
            op_id: "a".into(),
            expected: 1,
            got: 2,
        },
        Error::InlineNoOutput { op_id: "a".into() },
        Error::InlineOutputCountMismatch {
            op_id: "a".into(),
            got: 2,
        },
        Error::WireFormatValidation {
            message: "bad bytes".into(),
        },
        Error::Lowering {
            message: "bad lower".into(),
        },
        Error::Interp {
            message: "bad interp".into(),
        },
        Error::Gpu {
            message: "bad gpu".into(),
        },
        Error::DecodeConfig {
            message: "bad cfg".into(),
        },
        Error::Decode {
            message: "bad decode".into(),
        },
        Error::Decompress {
            message: "bad decomp".into(),
        },
        Error::Dfa {
            message: "bad dfa".into(),
        },
        Error::Dataflow {
            message: "bad dataflow".into(),
        },
        Error::Prefix {
            message: "bad prefix".into(),
        },
        Error::Csr {
            message: "bad csr".into(),
        },
        Error::Serialization {
            message: "bad ser".into(),
        },
        Error::RuleEval {
            message: "bad rule".into(),
        },
        Error::VersionMismatch {
            expected: 3,
            found: 1,
        },
        Error::UnknownDialect {
            name: "math".into(),
            requested: "1.0".into(),
        },
        Error::UnknownOp {
            dialect: "math".into(),
            op: "add".into(),
        },
    ];

    for err in samples {
        let diag = Diagnostic::from(&err);
        assert!(diag.code.as_str().starts_with("E-"));
        assert!(!diag.message.is_empty());
        assert_eq!(diag.severity, Severity::Error);
        let _ = diag.render_human();
        let _ = diag.to_json();
    }
}

#[test]
fn warning_and_note_constructors() {
    let w = Diagnostic::warning("W-DEPRECATED", "x is deprecated");
    assert_eq!(w.severity, Severity::Warning);
    assert!(w.render_human().starts_with("warning[W-DEPRECATED]:"));

    let n = Diagnostic::note("N-INFO", "fyi");
    assert_eq!(n.severity, Severity::Note);
    assert!(n.render_human().starts_with("note[N-INFO]:"));
}

#[test]
fn operand_and_attr_location_render() {
    let diag = Diagnostic::error("E-X", "boom")
        .with_location(OpLocation::op("math.add").with_operand(2).with_attr("mode"));
    let r = diag.render_human();
    assert!(r.contains("op `math.add` operand[2] attr `mode`"));
}
