use std::borrow::Cow;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use super::serde_cow::{de_cow_static, de_opt_cow_static};

/// Severity of a [`Diagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Severity {
    /// A hard failure; the caller must not use the program.
    Error,
    /// A soft failure; the program is usable but something is off.
    Warning,
    /// An informational follow-up attached to another diagnostic.
    Note,
}

impl Severity {
    /// Short label suitable for human rendering.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        }
    }
}

/// Stable, machine-readable diagnostic code.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DiagnosticCode(#[serde(deserialize_with = "de_cow_static")] pub Cow<'static, str>);

impl DiagnosticCode {
    /// Construct a code from a static string.
    #[must_use]
    pub const fn new(code: &'static str) -> Self {
        Self(Cow::Borrowed(code))
    }

    /// The raw code string, for example `"E-INLINE-CYCLE"`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Location of a diagnostic inside a `Program`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpLocation {
    /// The op identifier, for example `"math.add"`.
    #[serde(deserialize_with = "de_cow_static")]
    pub op_id: Cow<'static, str>,
    /// Zero-based operand index, if the diagnostic is about a specific operand.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub operand_idx: Option<u32>,
    /// Attribute name, if the diagnostic is about a specific attribute.
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        deserialize_with = "de_opt_cow_static"
    )]
    pub attr_name: Option<Cow<'static, str>>,
}

impl OpLocation {
    /// Build a location that only identifies the op.
    #[must_use]
    pub fn op(op_id: impl Into<Cow<'static, str>>) -> Self {
        Self {
            op_id: op_id.into(),
            operand_idx: None,
            attr_name: None,
        }
    }

    /// Attach a specific operand index.
    #[must_use]
    pub fn with_operand(mut self, idx: u32) -> Self {
        self.operand_idx = Some(idx);
        self
    }

    /// Attach a specific attribute name.
    #[must_use]
    pub fn with_attr(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.attr_name = Some(name.into());
        self
    }
}

/// A structured diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity of the diagnostic.
    pub severity: Severity,
    /// Stable machine-readable code.
    pub code: DiagnosticCode,
    /// The primary human-readable message.
    #[serde(deserialize_with = "de_cow_static")]
    pub message: Cow<'static, str>,
    /// Optional op / operand / attribute location.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub location: Option<OpLocation>,
    /// Optional actionable fix the caller can apply.
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        deserialize_with = "de_opt_cow_static"
    )]
    pub suggested_fix: Option<Cow<'static, str>>,
    /// Optional documentation URL.
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        deserialize_with = "de_opt_cow_static"
    )]
    pub doc_url: Option<Cow<'static, str>>,
}

impl Diagnostic {
    /// Construct a new error-severity diagnostic.
    #[must_use]
    pub fn error(code: &'static str, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            severity: Severity::Error,
            code: DiagnosticCode::new(code),
            message: message.into(),
            location: None,
            suggested_fix: None,
            doc_url: None,
        }
    }

    /// Construct a new warning-severity diagnostic.
    #[must_use]
    pub fn warning(code: &'static str, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            severity: Severity::Warning,
            code: DiagnosticCode::new(code),
            message: message.into(),
            location: None,
            suggested_fix: None,
            doc_url: None,
        }
    }

    /// Construct a new note-severity diagnostic.
    #[must_use]
    pub fn note(code: &'static str, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            severity: Severity::Note,
            code: DiagnosticCode::new(code),
            message: message.into(),
            location: None,
            suggested_fix: None,
            doc_url: None,
        }
    }

    /// Attach an op location.
    #[must_use]
    pub fn with_location(mut self, loc: OpLocation) -> Self {
        self.location = Some(loc);
        self
    }

    /// Attach a suggested fix.
    #[must_use]
    pub fn with_fix(mut self, fix: impl Into<Cow<'static, str>>) -> Self {
        self.suggested_fix = Some(fix.into());
        self
    }

    /// Attach a documentation URL.
    #[must_use]
    pub fn with_doc_url(mut self, url: impl Into<Cow<'static, str>>) -> Self {
        self.doc_url = Some(url.into());
        self
    }

    /// Render the diagnostic as rustc-style human text.
    #[must_use]
    pub fn render_human(&self) -> String {
        let mut out = String::with_capacity(256);
        let _ = write!(
            out,
            "{}[{}]: {}",
            self.severity.label(),
            self.code,
            self.message
        );
        if let Some(loc) = &self.location {
            out.push_str("\n  --> op `");
            out.push_str(&loc.op_id);
            out.push('`');
            if let Some(idx) = loc.operand_idx {
                let _ = write!(out, " operand[{idx}]");
            }
            if let Some(attr) = &loc.attr_name {
                out.push_str(" attr `");
                out.push_str(attr);
                out.push('`');
            }
        }
        if let Some(fix) = &self.suggested_fix {
            out.push_str("\n  = help: ");
            out.push_str(fix);
        }
        if let Some(url) = &self.doc_url {
            out.push_str("\n  = note: ");
            out.push_str(url);
        }
        out
    }

    /// Serialize the diagnostic to a JSON string.
    #[must_use]
    pub fn to_json(&self) -> String {
        match serde_json::to_string(self) {
            Ok(json) => json,
            Err(e) => format!(
                r#"{{"error":"Diagnostic::to_json serialization failed","code":"{code}","message":"{message}","serde_error":"{serde_error}","fix":"Fix: inspect Diagnostic fields for non-serializable types; every field must implement Serialize."}}"#,
                code = self.code.as_str().replace('"', "\\\""),
                message = self.message.replace('"', "\\\""),
                serde_error = e.to_string().replace('"', "\\\""),
            ),
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render_human())
    }
}
