//! C frontend production-boundary tests.
//!
//! CPU/reference helpers are allowed only as explicit parity oracles behind
//! `#[cfg(test)]` or `feature = "cpu-parity"`. Production C frontend modules
//! must expose dispatchable GPU builders and semantic contracts, not fallback
//! helper APIs.

#![allow(deprecated)]
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn c_frontend_reference_modules_are_cfg_gated() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/parsing/c");
    let mut files = Vec::new();
    collect_rust_files(&root, &mut files);

    let mut violations = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).expect("C frontend source file must be readable");
        let lines = text.lines().collect::<Vec<_>>();
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if !is_reference_module_decl(trimmed) {
                continue;
            }
            if !has_test_or_cpu_parity_guard(&lines, idx) {
                violations.push(format!(
                    "{}:{}: reference module declaration must be cfg(test/cpu-parity): {}",
                    path.display(),
                    idx + 1,
                    trimmed
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "C frontend reference/oracle modules must be unreachable from production builds.\n{}",
        violations.join("\n")
    );
}

#[test]
fn c_frontend_reference_public_surface_is_cfg_gated() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/parsing/c");
    let mut files = Vec::new();
    collect_rust_files(&root, &mut files);

    let mut violations = Vec::new();
    for path in &files {
        if is_reference_only_path(path) || is_test_source_path(path) {
            continue;
        }
        let text = fs::read_to_string(path).expect("C frontend source file must be readable");
        let lines = text.lines().collect::<Vec<_>>();
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if !is_public_reference_surface(trimmed) {
                continue;
            }
            if !has_test_or_cpu_parity_guard(&lines, idx) {
                violations.push(format!(
                    "{}:{}: public reference/CPU surface must be cfg(test/cpu-parity): {}",
                    path.display(),
                    idx + 1,
                    trimmed
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "C frontend production modules must not expose CPU/reference APIs except behind \
         explicit parity cfgs.\n{}",
        violations.join("\n")
    );
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("C frontend source directory must be readable") {
        let entry = entry.expect("C frontend source entry must be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn is_reference_module_decl(line: &str) -> bool {
    (line.starts_with("mod ") || line.starts_with("pub mod "))
        && (line.contains("reference")
            || line.contains("cpu")
            || line.contains("oracle")
            || line.contains("ref_"))
}

fn is_public_reference_surface(line: &str) -> bool {
    let public_decl = line.starts_with("pub fn ")
        || line.starts_with("pub(crate) fn ")
        || line.starts_with("pub(super) fn ")
        || line.starts_with("pub use ")
        || line.starts_with("pub(crate) use ")
        || line.starts_with("pub(super) use ");
    public_decl
        && (line.contains("reference_")
            || line.contains("try_reference_")
            || line.contains("cpu_ref")
            || line.contains("_cpu")
            || line.contains("cpu_")
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

fn is_reference_only_path(path: &Path) -> bool {
    path.components().any(|component| {
        let Some(name) = component.as_os_str().to_str() else {
            return false;
        };
        name == "reference.rs" || name.starts_with("ref_")
    })
}

fn is_test_source_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "tests.rs" || name.ends_with("_tests.rs"))
}
