//! ROADMAP H10d  -  audit pass that walks every `pub fn analyze` in
//! `vyre-foundation/src/optimizer/passes/**` and asserts each pass
//! file with an `analyze` body that can return `PassAnalysis::SKIP`
//! also has at least one `#[test]` whose body asserts a `SKIP`
//! verdict. The check is text-level (no source introspection),
//! using the workspace-relative `CARGO_MANIFEST_DIR` to locate the
//! tree.
//!
//! ## Why this is a contract, not a hint
//!
//! A `pub fn analyze` that returns `SKIP` on a real input shape but
//! has no test exercising that path is a silent regression risk:
//! a future edit can make the SKIP gate false-negative (run the
//! transform on programs that should have been skipped) without any
//! test failing. The audit fails the build the moment a new pass
//! lands without its SKIP-arm test.
//!
//! ## What "exercises SKIP" looks like
//!
//! A pass file may exercise the SKIP path either by:
//! - matching `PassAnalysis::SKIP` on an `analyze` result, or
//! - calling `analyze` and asserting the value via the `analyze_skips_*`
//!   naming convention used across the existing pass suite.
//!
//! Both shapes are checked literally. If either marker appears anywhere
//! in the file (test or non-test code), the pass passes the audit.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn every_pass_with_skip_branch_has_a_test_that_exercises_it() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let passes_dir = manifest_dir.join("src/optimizer/passes");
    assert!(
        passes_dir.is_dir(),
        "Fix: src/optimizer/passes must exist relative to vyre-foundation crate root"
    );

    let mut violators: BTreeSet<String> = BTreeSet::new();
    walk(&passes_dir, &mut |path| {
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            return;
        }
        if path.file_name().and_then(|s| s.to_str()) == Some("mod.rs") {
            // mod.rs files re-export pass modules; the pass logic and
            // tests live in their own files which the walker will
            // visit separately.
            return;
        }
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                let rel = path.strip_prefix(&manifest_dir).unwrap_or(path);
                violators.insert(format!(
                    "{}: unreadable pass source: {error}",
                    rel.display()
                ));
                return;
            }
        };
        if !file_has_analyze_with_skip(&text) {
            return;
        }
        if file_exercises_skip_in_a_test(&text) {
            return;
        }
        let rel = path.strip_prefix(&manifest_dir).unwrap_or(path);
        violators.insert(rel.display().to_string());
    });

    assert!(
        violators.is_empty(),
        "Fix: every pass file with a `pub fn analyze` that can return `PassAnalysis::SKIP` must \
         have at least one #[test] that exercises the SKIP branch. Pass files missing the test \
         (count = {count}):\n{list}",
        count = violators.len(),
        list = violators
            .iter()
            .map(|v| format!("  - {v}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

fn walk(dir: &Path, f: &mut dyn FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, f);
        } else {
            f(&path);
        }
    }
}

/// True iff the file declares `pub fn analyze` AND the body of that
/// function references `PassAnalysis::SKIP` (or the same constant via
/// a `use` alias). The textual check is deliberately permissive:
/// any source file that mentions `PassAnalysis::SKIP` in proximity
/// to a `pub fn analyze` declaration is presumed to have a SKIP arm.
fn file_has_analyze_with_skip(text: &str) -> bool {
    text.contains("pub fn analyze") && text.contains("PassAnalysis::SKIP")
}

/// True iff the file has at least one `#[test]` block whose body
/// references `PassAnalysis::SKIP` or whose name matches the
/// `analyze_skip*` / `*skips*` convention used by sibling tests.
fn file_exercises_skip_in_a_test(text: &str) -> bool {
    let mut start = 0usize;
    while let Some(test_marker) = text[start..].find("#[test]") {
        let abs = start + test_marker;
        let mut search_window_end = text.len();
        if let Some(next) = text[abs + 1..].find("#[test]") {
            search_window_end = abs + 1 + next;
        }
        let block = &text[abs..search_window_end];
        if block.contains("PassAnalysis::SKIP")
            || block.contains("analyze_skip")
            || block.contains("_skips_")
            || block.contains("skips_program")
            || block.contains("skip_when")
        {
            return true;
        }
        start = abs + 1;
    }
    false
}
