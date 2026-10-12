//! Primitive production-boundary tests.
//!
//! Primitive CPU/reference helpers are parity oracles, not release-path APIs.
//! They must stay behind `#[cfg(test)]` or explicit `feature = "cpu-parity"`.

use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn primitive_cpu_reference_surfaces_are_cfg_gated() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&root, &mut files);

    let mut violations = Vec::new();
    for path in &files {
        if is_test_source_path(path) {
            continue;
        }
        let text = fs::read_to_string(path).expect("primitive source file must be readable");
        let lines = text.lines().collect::<Vec<_>>();
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if !is_public_cpu_reference_surface(trimmed) {
                continue;
            }
            if !has_test_or_cpu_parity_guard(&lines, idx) {
                violations.push(format!(
                    "{}:{}: public CPU/reference helper must be cfg(test/cpu-parity): {}",
                    path.display(),
                    idx + 1,
                    trimmed
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "vyre-primitives must not expose CPU/reference helper APIs in production builds.\n{}",
        violations.join("\n")
    );
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("primitive source directory must be readable") {
        let entry = entry.expect("primitive source entry must be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn is_public_cpu_reference_surface(line: &str) -> bool {
    let public_decl = line.starts_with("pub fn ")
        || line.starts_with("pub(crate) fn ")
        || line.starts_with("pub(super) fn ")
        || line.starts_with("pub use ")
        || line.starts_with("pub(crate) use ")
        || line.starts_with("pub(super) use ");
    public_decl
        && (line.contains("cpu_ref")
            || line.contains("_cpu")
            || line.contains("cpu_")
            || line.contains("reference_")
            || line.contains("oracle"))
}

fn has_test_or_cpu_parity_guard(lines: &[&str], idx: usize) -> bool {
    let mut remaining_attrs = 8usize;
    let mut cursor = idx;
    while cursor > 0 && remaining_attrs > 0 {
        cursor -= 1;
        let prior = lines[cursor].trim();
        if prior.is_empty() {
            continue;
        }
        if !prior.starts_with("#[") {
            break;
        }
        if prior.contains("cfg(test)")
            || prior.contains("feature = \"cpu-parity\"")
            || prior.contains("feature=\"cpu-parity\"")
        {
            return true;
        }
        remaining_attrs -= 1;
    }
    false
}

fn is_test_source_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "tests.rs" || name.ends_with("_tests.rs"))
}
