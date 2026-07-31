use crate::error::Error;

use super::{Diagnostic, OpLocation};

/// Split a legacy error message on the first `". Fix: "` delimiter.
pub(super) fn split_fix(full: String) -> (String, Option<String>) {
    if let Some((head, tail)) = full.split_once(". Fix: ") {
        let head = head.to_owned();
        let tail = tail.trim_end_matches('.').to_owned();
        (head, Some(tail))
    } else {
        (full, None)
    }
}

/// Map a legacy `Error` variant to a diagnostic code + op location.
fn classify(err: &Error) -> (&'static str, Option<OpLocation>) {
    match err {
        Error::InlineCycle { op_id } => ("E-INLINE-CYCLE", Some(OpLocation::op(op_id.clone()))),
        Error::InlineUnknownOp { op_id } => {
            ("E-INLINE-UNKNOWN-OP", Some(OpLocation::op(op_id.clone())))
        }
        Error::InlineNonInlinable { op_id } => (
            "E-INLINE-NON-INLINABLE",
            Some(OpLocation::op(op_id.clone())),
        ),
        Error::InlineArgCountMismatch { op_id, .. } => {
            ("E-INLINE-ARG-COUNT", Some(OpLocation::op(op_id.clone())))
        }
        Error::InlineNoOutput { op_id } => {
            ("E-INLINE-NO-OUTPUT", Some(OpLocation::op(op_id.clone())))
        }
        Error::InlineOutputCountMismatch { op_id, .. } => {
            ("E-INLINE-OUTPUT-COUNT", Some(OpLocation::op(op_id.clone())))
        }
        Error::WireFormatValidation { .. } => ("E-WIRE-VALIDATION", None),
        Error::Lowering { .. } => ("E-LOWERING", None),
        Error::Interp { .. } => ("E-INTERP", None),
        Error::Gpu { .. } => ("E-GPU", None),
        Error::DecodeConfig { .. } => ("E-DECODE-CONFIG", None),
        Error::Decode { .. } => ("E-DECODE", None),
        Error::Decompress { .. } => ("E-DECOMPRESS", None),
        Error::Dfa { .. } => ("E-DFA", None),
        Error::Dataflow { .. } => ("E-DATAFLOW", None),
        Error::Prefix { .. } => ("E-PREFIX", None),
        Error::Csr { .. } => ("E-CSR", None),
        Error::Serialization { .. } => ("E-SERIALIZATION", None),
        Error::RuleEval { .. } => ("E-RULE-EVAL", None),
        Error::VersionMismatch { .. } => ("E-WIRE-VERSION", None),
        Error::UnknownDialect { .. } => ("E-WIRE-UNKNOWN-DIALECT", None),
        Error::UnknownOp { dialect, op } => (
            "E-WIRE-UNKNOWN-OP",
            Some(OpLocation::op(format!("{dialect}.{op}"))),
        ),
        _ => ("E-UNKNOWN", None),
    }
}

impl From<&Error> for Diagnostic {
    fn from(err: &Error) -> Self {
        let (code, location) = classify(err);
        let (message, fix) = split_fix(err.to_string());
        let mut diag = Diagnostic::error(code, message);
        if let Some(fix) = fix {
            diag = diag.with_fix(fix);
        }
        if let Some(loc) = location {
            diag = diag.with_location(loc);
        }
        diag
    }
}

impl From<Error> for Diagnostic {
    fn from(err: Error) -> Self {
        Diagnostic::from(&err)
    }
}
