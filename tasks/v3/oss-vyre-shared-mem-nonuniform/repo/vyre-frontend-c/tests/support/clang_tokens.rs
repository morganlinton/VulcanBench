//! clang preprocessed-token oracle support.
//!
//! This module treats clang as the external oracle for token facts. clang not
//! being present is a release-host configuration failure, not a skipped test.

use std::path::Path;
use std::process::Command;

/// Structured clang source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangSourceLocation {
    /// Raw clang location payload.
    pub(crate) raw: String,
    /// Source file path or clang pseudo-file, such as `<built-in>`.
    pub(crate) file: String,
    /// One-based source line when clang reports one.
    pub(crate) line: Option<u32>,
    /// One-based source column when clang reports one.
    pub(crate) column: Option<u32>,
}

/// One diagnostic emitted while extracting preprocessed token facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangTokenDiagnostic {
    /// Diagnostic severity, such as `warning`, `error`, `fatal error`, or `note`.
    pub(crate) severity: String,
    /// Diagnostic message without the leading location/severity prefix.
    pub(crate) message: String,
    /// Diagnostic source location when clang reports one.
    pub(crate) location: Option<ClangSourceLocation>,
    /// Raw diagnostic line from clang.
    pub(crate) raw_line: String,
}

/// One token from `clang -E -Xclang -dump-tokens`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangPreprocessedToken {
    /// clang token kind spelling, such as `identifier` or `numeric_constant`.
    pub(crate) kind: String,
    /// Token spelling after preprocessing.
    pub(crate) spelling: String,
    /// Primary clang source location payload.
    pub(crate) location: String,
    /// Structured primary clang source location.
    pub(crate) source_location: ClangSourceLocation,
    /// Macro spelling location when clang reports one.
    pub(crate) spelling_location: Option<String>,
    /// Structured macro spelling location when clang reports one.
    pub(crate) macro_spelling_location: Option<ClangSourceLocation>,
    /// Included file that produced this token, when the token does not originate in the main file.
    pub(crate) include_origin: Option<String>,
    /// Whether clang marked this token as starting a logical line.
    pub(crate) at_start_of_line: bool,
    /// Whether clang marked this token as having leading space.
    pub(crate) has_leading_space: bool,
}

/// clang preprocessed-token oracle result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangPreprocessedTokenFacts {
    /// Token facts emitted by clang.
    pub(crate) tokens: Vec<ClangPreprocessedToken>,
    /// Diagnostics emitted while token facts were extracted.
    pub(crate) diagnostics: Vec<ClangTokenDiagnostic>,
}

/// Runs clang preprocessing and returns token facts.
pub(crate) fn clang_preprocessed_tokens(
    c_file: &Path,
) -> Result<Vec<ClangPreprocessedToken>, String> {
    let facts = clang_preprocessed_token_facts(c_file)?;
    if facts.tokens.is_empty() {
        return Err(format!(
            "clang token oracle produced no tokens for {}",
            c_file.display()
        ));
    }
    Ok(facts.tokens)
}

/// Runs clang preprocessing and returns token facts plus diagnostics.
pub(crate) fn clang_preprocessed_token_facts(
    c_file: &Path,
) -> Result<ClangPreprocessedTokenFacts, String> {
    clang_preprocessed_token_facts_with_extra_args(c_file, std::iter::empty::<&str>())
}

/// Runs clang preprocessing with additional compiler arguments and returns
/// token facts plus diagnostics.
pub(crate) fn clang_preprocessed_token_facts_with_extra_args<I, S>(
    c_file: &Path,
    extra_args: I,
) -> Result<ClangPreprocessedTokenFacts, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new("clang")
        .args(["-E", "-Xclang", "-dump-tokens", "-x", "c"])
        .args(extra_args)
        .arg(c_file)
        .output()
        .map_err(|e| format!("clang token oracle invocation failed: {e}"))?;
    let main_file = std::fs::canonicalize(c_file)
        .unwrap_or_else(|_| c_file.to_path_buf())
        .to_string_lossy()
        .into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut pending_token_line = String::new();
    for line in stderr.lines() {
        if !pending_token_line.is_empty() {
            pending_token_line.push('\n');
            pending_token_line.push_str(line);
            if let Some(token) = parse_dump_token_line(&pending_token_line, &main_file) {
                tokens.push(token);
                pending_token_line.clear();
            }
            continue;
        }
        if let Some(token) = parse_dump_token_line(line, &main_file) {
            tokens.push(token);
        } else if let Some(diagnostic) = parse_diagnostic_line(line) {
            diagnostics.push(diagnostic);
        } else if looks_like_split_dump_token_line(line) {
            pending_token_line.push_str(line);
        }
    }
    if !output.status.success() && diagnostics.is_empty() {
        return Err(format!(
            "clang token oracle exited {} without parseable diagnostics: {}",
            output.status,
            stderr.trim()
        ));
    }
    Ok(ClangPreprocessedTokenFacts {
        tokens,
        diagnostics,
    })
}

fn looks_like_split_dump_token_line(line: &str) -> bool {
    line.contains("'\t") && !line.contains("Loc=<")
}

/// Convenience wrapper for tests that require clang as an external token oracle.
pub(crate) fn clang_preprocessed_tokens_required(c_file: &Path) -> Vec<ClangPreprocessedToken> {
    match clang_preprocessed_tokens(c_file) {
        Ok(tokens) => tokens,
        Err(why) => panic!(
            "clang_tokens: clang token oracle failed for {}: {why}. Fix: install clang or repair clang -E -Xclang -dump-tokens parity support.",
            c_file.display()
        ),
    }
}

fn parse_dump_token_line(line: &str, main_file: &str) -> Option<ClangPreprocessedToken> {
    let first_space = line.find(' ')?;
    let kind = &line[..first_space];
    let spelling_start = line[first_space..].find('\'')? + first_space + 1;
    let spelling_end = line.rfind("'\t")?;
    if spelling_end < spelling_start {
        return None;
    }
    let spelling = &line[spelling_start..spelling_end];
    let loc_start = line.find("Loc=<")? + "Loc=<".len();
    let loc_end = line.rfind('>')?;
    if loc_end < loc_start {
        return None;
    }
    let loc_payload = &line[loc_start..loc_end];
    let spelling_location = loc_payload.find("<Spelling=").and_then(|idx| {
        let after = idx + "<Spelling=".len();
        loc_payload[after..]
            .find('>')
            .map(|end| loc_payload[after..after + end].trim().to_string())
    });
    let location = loc_payload
        .split(" <Spelling=")
        .next()
        .unwrap_or(loc_payload)
        .trim()
        .to_string();
    let source_location = parse_source_location(&location);
    let macro_spelling_location = spelling_location.as_deref().map(parse_source_location);
    let include_origin = if !source_location.file.is_empty() && source_location.file != main_file {
        Some(source_location.file.clone())
    } else {
        None
    };
    Some(ClangPreprocessedToken {
        kind: kind.to_string(),
        spelling: spelling.to_string(),
        location,
        source_location,
        spelling_location,
        macro_spelling_location,
        include_origin,
        at_start_of_line: line.contains("[StartOfLine]"),
        has_leading_space: line.contains("[LeadingSpace]"),
    })
}

fn parse_diagnostic_line(line: &str) -> Option<ClangTokenDiagnostic> {
    for marker in [": fatal error: ", ": error: ", ": warning: ", ": note: "] {
        let Some(idx) = line.find(marker) else {
            continue;
        };
        let prefix = &line[..idx];
        let severity = marker
            .trim_start_matches(": ")
            .trim_end_matches(": ")
            .to_string();
        let message = line[idx + marker.len()..].to_string();
        return Some(ClangTokenDiagnostic {
            severity,
            message,
            location: parse_diagnostic_location(prefix),
            raw_line: line.to_string(),
        });
    }
    None
}

fn parse_diagnostic_location(raw: &str) -> Option<ClangSourceLocation> {
    let parsed = parse_source_location(raw);
    if parsed.line.is_some() && parsed.column.is_some() {
        Some(parsed)
    } else {
        None
    }
}

fn parse_source_location(raw: &str) -> ClangSourceLocation {
    let mut pieces = raw.rsplitn(3, ':');
    let column = pieces.next().and_then(|part| part.parse::<u32>().ok());
    let line = pieces.next().and_then(|part| part.parse::<u32>().ok());
    let file = pieces.next().unwrap_or(raw).to_string();
    ClangSourceLocation {
        raw: raw.to_string(),
        file,
        line,
        column,
    }
}
