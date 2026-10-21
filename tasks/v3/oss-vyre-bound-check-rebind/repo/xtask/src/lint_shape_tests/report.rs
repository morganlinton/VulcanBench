// Markdown report generation for the shape-test audit. Plain `//`
// because this file is `include!()`-d into a `mod report {}` scope.

use std::fs;
use std::path::Path;

use super::Finding;

/// Write the audit report as a markdown table.
pub(crate) fn write_report(path: &Path, findings: &[Finding]) {
    let mut lines = Vec::new();
    lines.push("# Test Audit  -  Shape vs Truth Assertions\n".to_string());
    lines.push("| crate | test_module::test_name | classification | reason |".to_string());
    lines.push("|-------|------------------------|----------------|--------|".to_string());

    for f in findings {
        let module_test = if f.module_path.is_empty() {
            f.test_name.clone()
        } else {
            format!("{}::{}", f.module_path, f.test_name)
        };
        let file_line = format!("{}:{}", f.file.display(), f.line);
        let reason = f.reason.replace('|', "\\|");
        lines.push(format!(
            "| {} | {} | {} | {} ({}) |",
            f.crate_name,
            module_test,
            f.classification.as_str(),
            reason,
            file_line
        ));
    }

    lines.push(String::new());
    lines.push("## Legend\n".to_string());
    lines.push("- **SHAPE**  -  test only asserts structural properties (is_ok, is_err, non-empty, status-only, roundtrip).".to_string());
    lines.push("- **TRUTH**  -  test asserts at least one specific expected value.".to_string());
    lines.push("- **NO_ASSERTS**  -  test contains no assert*! macros.".to_string());

    fs::write(path, lines.join("\n")).unwrap_or_else(|e| {
        panic!("Fix: failed to write report {}: {e}", path.display());
    });
}
