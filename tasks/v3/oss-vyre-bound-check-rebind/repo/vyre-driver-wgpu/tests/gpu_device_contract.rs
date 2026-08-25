//! Integration test crate for the containing Vyre package.

use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn adapter_enumeration_stays_inside_runtime_device_contract() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut findings = Vec::new();
    scan_tree(&crate_root.join("src"), &crate_root, &mut findings);
    scan_tree(&crate_root.join("tests"), &crate_root, &mut findings);

    assert!(
        findings.is_empty(),
        "WGPU adapter enumeration must stay centralized in runtime::device:\n{}",
        findings.join("\n")
    );
}

fn scan_tree(root: &Path, crate_root: &Path, findings: &mut Vec<String>) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let entries = fs::read_dir(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        for entry in entries {
            let entry = entry
                .unwrap_or_else(|err| panic!("failed to read entry in {}: {err}", path.display()));
            let path = entry.path();
            let file_type = entry
                .file_type()
                .unwrap_or_else(|err| panic!("failed to stat {}: {err}", path.display()));

            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                scan_file(crate_root, &path, findings);
            }
        }
    }
}

fn scan_file(crate_root: &Path, path: &Path, findings: &mut Vec<String>) {
    let relative = path
        .strip_prefix(crate_root)
        .expect("scanned path must be inside crate root");
    if is_runtime_device_contract(relative) {
        return;
    }

    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    for (line_index, line) in contents.lines().enumerate() {
        let forbidden = ["enumerate_adapters", "(wgpu::Backends::all())"].concat();
        if line.contains(&forbidden) {
            findings.push(format!(
                "{}:{}: {}",
                relative.display(),
                line_index + 1,
                line.trim()
            ));
        }
    }
}

fn is_runtime_device_contract(relative: &Path) -> bool {
    relative.starts_with("src/runtime/device")
}
