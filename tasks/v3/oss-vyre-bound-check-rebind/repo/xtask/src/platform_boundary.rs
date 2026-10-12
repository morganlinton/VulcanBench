//! Platform source/documentation boundary lint.
//!
//! The tier system is meaningless if platform crate docs name downstream
//! consumers. This command scans Rust comments and Markdown in platform
//! crates for known consumer names and fails with file/line evidence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process;

const PLATFORM_ROOTS: &[&str] = &[
    "vyre-foundation",
    "vyre-primitives",
    "vyre-libs",
    "vyre-driver",
    "vyre-runtime",
    "vyre-self-substrate",
];

const FORBIDDEN_CONSUMERS: &[&str] = &["surgec", "weir", "gossan", "keyhog"];
const MAX_PLATFORM_BOUNDARY_FILE_BYTES: u64 = 16_777_216;

#[derive(Debug, Clone, Eq, PartialEq)]
struct Finding {
    path: PathBuf,
    line: usize,
    term: &'static str,
    text: String,
}

/// Run the platform-boundary lint.
pub(crate) fn run(args: &[String]) {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "USAGE:\n  cargo_full run --bin xtask -- platform-boundary\n\n\
             Scans platform crate Rust comments and Markdown for downstream consumer names."
        );
        return;
    }
    if args.len() > 2 {
        eprintln!("Fix: platform-boundary takes no positional arguments.");
        process::exit(2);
    }

    let root = vyre_workspace_root();
    let mut findings = Vec::new();
    let mut errors = Vec::new();
    for relative in PLATFORM_ROOTS {
        scan_tree(&root.join(relative), &root, &mut findings, &mut errors);
    }

    if !errors.is_empty() {
        eprintln!("platform-boundary: {} scan error(s):", errors.len());
        for error in errors {
            eprintln!("  {error}");
        }
        eprintln!("Fix: make all platform source/doc files readable before release.");
        process::exit(1);
    }
    if findings.is_empty() {
        println!("platform-boundary: platform docs/comments are consumer-neutral.");
        return;
    }

    eprintln!(
        "platform-boundary: {} consumer-name layering violation(s):",
        findings.len()
    );
    for finding in findings.iter().take(50) {
        eprintln!(
            "  {}:{} contains `{}`: {}",
            finding.path.display(),
            finding.line,
            finding.term,
            finding.text.trim()
        );
    }
    if findings.len() > 50 {
        eprintln!("  ... {} more", findings.len() - 50);
    }
    eprintln!(
        "Fix: replace downstream names with neutral platform/dataflow/frontend wording, or move the doc to the consumer-owned crate."
    );
    process::exit(1);
}

fn vyre_workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .expect("Fix: xtask must live directly under the Vyre workspace root.")
}

fn scan_tree(root: &Path, workspace: &Path, findings: &mut Vec<Finding>, errors: &mut Vec<String>) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                errors.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        if metadata.is_dir() {
            let entries = match fs::read_dir(&path) {
                Ok(entries) => entries,
                Err(error) => {
                    errors.push(format!("{}: {error}", path.display()));
                    continue;
                }
            };
            for entry in entries {
                match entry {
                    Ok(entry) => stack.push(entry.path()),
                    Err(error) => errors.push(format!("{}: {error}", path.display())),
                }
            }
            continue;
        }
        if !is_scanned_file(&path) {
            continue;
        }
        if metadata.len() > MAX_PLATFORM_BOUNDARY_FILE_BYTES {
            errors.push(format!(
                "{} exceeds {MAX_PLATFORM_BOUNDARY_FILE_BYTES} byte platform-boundary read cap",
                path.display()
            ));
            continue;
        }
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                errors.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        collect_findings(&path, workspace, &text, findings);
    }
}

fn is_scanned_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("rs" | "md")
    )
}

fn collect_findings(path: &Path, workspace: &Path, text: &str, findings: &mut Vec<Finding>) {
    let markdown = path.extension().and_then(|ext| ext.to_str()) == Some("md");
    for (line_index, line) in text.lines().enumerate() {
        if !markdown && !is_rust_comment_line(line) {
            continue;
        }
        for term in FORBIDDEN_CONSUMERS {
            if contains_word_case_insensitive(line, term) {
                findings.push(Finding {
                    path: path.strip_prefix(workspace).unwrap_or(path).to_path_buf(),
                    line: line_index + 1,
                    term,
                    text: line.to_string(),
                });
            }
        }
    }
}

fn is_rust_comment_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("*/")
}

fn contains_word_case_insensitive(line: &str, needle: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(offset) = lower[search_from..].find(needle) {
        let start = search_from + offset;
        let end = start + needle.len();
        if is_left_word_boundary(&lower, start) && is_right_word_boundary(&lower, end) {
            return true;
        }
        search_from = end;
    }
    false
}

fn is_left_word_boundary(text: &str, byte_index: usize) -> bool {
    if byte_index == 0 {
        return true;
    }
    is_non_word_byte(text.as_bytes()[byte_index - 1])
}

fn is_right_word_boundary(text: &str, byte_index: usize) -> bool {
    match text.as_bytes().get(byte_index) {
        None => true,
        Some(byte) => is_non_word_byte(*byte),
    }
}

fn is_non_word_byte(byte: u8) -> bool {
    !byte.is_ascii_alphanumeric() && byte != b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_consumer_names_in_comments_but_not_identifiers() {
        let mut findings = Vec::new();
        collect_findings(
            Path::new("vyre-libs/src/example.rs"),
            Path::new(""),
            "let weir_internal = 1;\n//! Weir owns this downstream wording\n// keyhog should not appear here",
            &mut findings,
        );
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].term, "weir");
        assert_eq!(findings[1].term, "keyhog");
    }

    #[test]
    fn scans_markdown_docs_for_consumer_names() {
        let mut findings = Vec::new();
        collect_findings(
            Path::new("vyre-primitives/README.md"),
            Path::new(""),
            "# Graph primitives\n\nThis platform doc mentions SurgeC and Gossan.",
            &mut findings,
        );

        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].term, "surgec");
        assert_eq!(findings[1].term, "gossan");
    }

    #[test]
    fn honors_word_boundaries() {
        assert!(!contains_word_case_insensitive("wearing a wire", "weir"));
        assert!(contains_word_case_insensitive("consumer: WEIR", "weir"));
    }
}
