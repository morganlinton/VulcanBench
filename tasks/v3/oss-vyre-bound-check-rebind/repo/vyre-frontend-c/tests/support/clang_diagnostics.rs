//! clang diagnostic oracle support.

use std::path::Path;
use std::process::Command;

/// clang diagnostic source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangDiagnosticLocation {
    /// Source file path.
    pub(crate) file: String,
    /// One-based source line.
    pub(crate) line: u32,
    /// One-based source column.
    pub(crate) column: u32,
}

/// clang parseable fix-it span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangFixIt {
    /// Source file path.
    pub(crate) file: String,
    /// One-based start line.
    pub(crate) start_line: u32,
    /// One-based start column.
    pub(crate) start_column: u32,
    /// One-based end line.
    pub(crate) end_line: u32,
    /// One-based end column.
    pub(crate) end_column: u32,
    /// Replacement text.
    pub(crate) replacement: String,
}

/// clang diagnostic fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangDiagnosticFact {
    /// Zero-based diagnostic sequence index.
    pub(crate) sequence_index: usize,
    /// Severity, such as `warning`, `error`, or `fatal error`.
    pub(crate) severity: String,
    /// Stable category derived from clang option brackets when present, otherwise severity.
    pub(crate) category: String,
    /// Diagnostic message.
    pub(crate) message: String,
    /// Primary diagnostic location.
    pub(crate) location: ClangDiagnosticLocation,
    /// Fix-its attached after this diagnostic.
    pub(crate) fixits: Vec<ClangFixIt>,
    /// Raw diagnostic line.
    pub(crate) raw_line: String,
    /// Whether clang emitted this diagnostic after an earlier error/fatal error, proving recovery.
    pub(crate) recovered_after_error: bool,
}

/// Run clang and return diagnostics plus parseable fix-its.
pub(crate) fn clang_diagnostics(c_file: &Path) -> Result<Vec<ClangDiagnosticFact>, String> {
    let output = Command::new("clang")
        .args(["-fsyntax-only", "-fdiagnostics-parseable-fixits", "-x", "c"])
        .arg(c_file)
        .output()
        .map_err(|e| format!("clang diagnostics oracle invocation failed: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut diagnostics: Vec<ClangDiagnosticFact> = Vec::new();
    for line in stderr.lines() {
        if let Some(fixit) = parse_fixit_line(line) {
            if let Some(last) = diagnostics.last_mut() {
                last.fixits.push(fixit);
            }
        } else if let Some(mut diagnostic) = parse_diagnostic_line(line) {
            diagnostic.sequence_index = diagnostics.len();
            diagnostic.recovered_after_error = diagnostics
                .iter()
                .any(|existing| matches!(existing.severity.as_str(), "error" | "fatal error"));
            diagnostics.push(diagnostic);
        }
    }
    if !output.status.success() && diagnostics.is_empty() {
        return Err(format!(
            "clang diagnostics oracle exited {} without parseable diagnostics: {}",
            output.status,
            stderr.trim()
        ));
    }
    Ok(diagnostics)
}

fn parse_diagnostic_line(line: &str) -> Option<ClangDiagnosticFact> {
    for marker in [": fatal error: ", ": error: ", ": warning: ", ": note: "] {
        let Some(idx) = line.find(marker) else {
            continue;
        };
        let location = parse_location(&line[..idx])?;
        let severity = marker
            .trim_start_matches(": ")
            .trim_end_matches(": ")
            .to_string();
        let mut message = line[idx + marker.len()..].to_string();
        let category = if let Some(option_start) = message.rfind(" [-W") {
            let option = message[option_start + 2..]
                .trim_end_matches(']')
                .to_string();
            message.truncate(option_start);
            option
        } else {
            severity.clone()
        };
        return Some(ClangDiagnosticFact {
            sequence_index: 0,
            severity,
            category,
            message,
            location,
            fixits: Vec::new(),
            raw_line: line.to_string(),
            recovered_after_error: false,
        });
    }
    None
}

fn parse_location(raw: &str) -> Option<ClangDiagnosticLocation> {
    let mut pieces = raw.rsplitn(3, ':');
    let column = pieces.next()?.parse::<u32>().ok()?;
    let line = pieces.next()?.parse::<u32>().ok()?;
    let file = pieces.next()?.to_string();
    Some(ClangDiagnosticLocation { file, line, column })
}

fn parse_fixit_line(line: &str) -> Option<ClangFixIt> {
    let rest = line.strip_prefix("fix-it:\"")?;
    let (file, rest) = rest.split_once("\":{")?;
    let (span, rest) = rest.split_once("}:\"")?;
    let replacement = rest.strip_suffix('"')?.to_string();
    let (start, end) = span.split_once('-')?;
    let (start_line, start_column) = parse_line_col(start)?;
    let (end_line, end_column) = parse_line_col(end)?;
    Some(ClangFixIt {
        file: file.to_string(),
        start_line,
        start_column,
        end_line,
        end_column,
        replacement,
    })
}

fn parse_line_col(raw: &str) -> Option<(u32, u32)> {
    let (line, column) = raw.split_once(':')?;
    Some((line.parse().ok()?, column.parse().ok()?))
}
