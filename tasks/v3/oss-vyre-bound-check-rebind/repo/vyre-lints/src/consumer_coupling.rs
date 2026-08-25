//! Consumer-name coupling guard.
//!
//! Platform crates and current public docs must describe capabilities, not
//! downstream products. The guard scans current Markdown, Rust comments,
//! Rust string literals, and path names. Historical archives, tests, examples,
//! and fixtures are intentionally exempt because they may preserve migration
//! context or consumer integration examples.

use crate::{paths::workspace_relative, Violation, ViolationKind};
use anyhow::Result;
use std::path::Path;

const CONSUMER_NAMES: &[&str] = &["weir", "surgec", "gossan", "keyhog", "flare-native"];

const EXEMPT_PATH_FRAGMENTS: &[&str] = &[
    "/docs/archive/",
    "/docs/legacy/",
    "/.internals/",
    "/tests/",
    "/benches/",
    "/examples/",
    "/fixtures/",
    "/target/",
];

pub fn scan_tree(root: &Path) -> Result<Vec<Violation>> {
    let mut all = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path();
        if !path.is_file() || !is_scanned_extension(path) {
            continue;
        }
        let workspace_rel = workspace_relative(path);
        if is_exempt_path(&workspace_rel) {
            continue;
        }
        if let Some((column, name)) = find_consumer_name_in_path(&workspace_rel) {
            all.push(Violation {
                file: workspace_rel.clone(),
                line: 1,
                column: column as u32,
                kind: ViolationKind::ConsumerCoupling,
                message: consumer_coupling_message(name, "path"),
            });
        }
        all.extend(scan_file(path, &workspace_rel)?);
    }
    Ok(all)
}

fn is_scanned_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("rs" | "md")
    )
}

fn is_exempt_path(workspace_rel: &str) -> bool {
    let wrapped = format!("/{workspace_rel}");
    EXEMPT_PATH_FRAGMENTS
        .iter()
        .any(|fragment| wrapped.contains(fragment))
}

fn scan_file(path: &Path, workspace_rel: &str) -> Result<Vec<Violation>> {
    let source = crate::read_source_bounded(path)?;
    let is_markdown = path.extension().and_then(|ext| ext.to_str()) == Some("md");
    let mut violations = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        for (column_offset, segment, context) in scanned_segments(line, is_markdown) {
            if let Some((column, name)) = find_consumer_name(segment) {
                violations.push(Violation {
                    file: workspace_rel.to_string(),
                    line: (line_idx + 1) as u32,
                    column: (column_offset + column) as u32,
                    kind: ViolationKind::ConsumerCoupling,
                    message: consumer_coupling_message(name, context),
                });
            }
        }
    }
    Ok(violations)
}

fn consumer_coupling_message(name: &str, context: &str) -> String {
    format!(
        "platform {context} mentions downstream consumer `{name}`. Fix: use a capability name such as dataflow, static analysis, scan, or consumer integration."
    )
}

fn scanned_segments(line: &str, is_markdown: bool) -> Vec<(usize, &str, &'static str)> {
    if is_markdown {
        return vec![(0, line, "markdown")];
    }
    if is_comment_line(line) {
        return vec![(0, line, "comment")];
    }
    rust_string_literal_segments(line)
        .into_iter()
        .map(|(offset, segment)| (offset, segment, "string literal"))
        .collect()
}

fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("*/")
}

fn rust_string_literal_segments(line: &str) -> Vec<(usize, &str)> {
    let bytes = line.as_bytes();
    let mut segments = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let Some(start_rel) = line[cursor..].find('"') else {
            break;
        };
        let start_quote = cursor + start_rel;
        if start_quote > 0 && bytes[start_quote - 1] == b'\'' {
            cursor = start_quote + 1;
            continue;
        }
        let content_start = start_quote + 1;
        let mut idx = content_start;
        let mut escaped = false;
        while idx < bytes.len() {
            let byte = bytes[idx];
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                segments.push((content_start, &line[content_start..idx]));
                cursor = idx + 1;
                break;
            }
            idx += 1;
        }
        if idx >= bytes.len() {
            break;
        }
    }
    segments
}

fn find_consumer_name(line: &str) -> Option<(usize, &'static str)> {
    let lower = line.to_ascii_lowercase();
    for name in CONSUMER_NAMES {
        let mut search_from = 0usize;
        while let Some(rel_idx) = lower[search_from..].find(name) {
            let idx = search_from + rel_idx;
            let end = idx + name.len();
            if is_start_boundary(lower.as_bytes(), idx) && is_end_boundary(lower.as_bytes(), end) {
                return Some((idx, *name));
            }
            search_from = end;
        }
    }
    None
}

fn find_consumer_name_in_path(path: &str) -> Option<(usize, &'static str)> {
    let lower = path.to_ascii_lowercase();
    for name in CONSUMER_NAMES {
        let mut search_from = 0usize;
        while let Some(rel_idx) = lower[search_from..].find(name) {
            let idx = search_from + rel_idx;
            let end = idx + name.len();
            if is_path_start_boundary(lower.as_bytes(), idx)
                && is_path_end_boundary(lower.as_bytes(), end)
            {
                return Some((idx, *name));
            }
            search_from = end;
        }
    }
    None
}

fn is_start_boundary(bytes: &[u8], idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    !bytes[idx - 1].is_ascii_alphanumeric() && bytes[idx - 1] != b'_'
}

fn is_end_boundary(bytes: &[u8], idx: usize) -> bool {
    if idx >= bytes.len() {
        return true;
    }
    !bytes[idx].is_ascii_alphanumeric() && bytes[idx] != b'_'
}

fn is_path_start_boundary(bytes: &[u8], idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    !bytes[idx - 1].is_ascii_alphanumeric()
}

fn is_path_end_boundary(bytes: &[u8], idx: usize) -> bool {
    if idx >= bytes.len() {
        return true;
    }
    !bytes[idx].is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_boundary_rejects_substrings() {
        assert!(find_consumer_name("// weir dataflow").is_some());
        assert!(find_consumer_name("// weird control flow").is_none());
        assert!(find_consumer_name("// keyhog-style coupling").is_some());
    }

    #[test]
    fn rust_string_literal_segments_ignore_identifiers_and_chars() {
        let segments = rust_string_literal_segments(
            "let keyhog_counter = 'k'; let label = \"surgec adapter\"; let raw = r#\"weir phase\"#;",
        );
        assert_eq!(segments.len(), 2);
        assert!(segments
            .iter()
            .any(|(_, segment)| *segment == "surgec adapter"));
        assert!(segments.iter().any(|(_, segment)| *segment == "weir phase"));
    }

    #[test]
    fn path_boundary_treats_underscore_as_separator() {
        assert_eq!(
            find_consumer_name_in_path("vyre-libs/src/security/surgec_bridge/mod.rs"),
            Some((23, "surgec"))
        );
        assert_eq!(find_consumer_name_in_path("docs/weird.md"), None);
    }
}
