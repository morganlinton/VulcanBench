//! Integration test crate for the containing Vyre package.

use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn raw_cuda_device_probing_stays_inside_device_contract() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut findings = Vec::new();
    scan_tree(&crate_root.join("src"), &crate_root, &mut findings);
    scan_tree(&crate_root.join("tests"), &crate_root, &mut findings);

    assert!(
        findings.is_empty(),
        "CUDA driver init, device-count, and context creation must stay centralized in src/device.rs:\n{}",
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
    if relative == Path::new("src/device.rs") {
        return;
    }

    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let forbidden = [
        ["CudaContext", "::new"].concat(),
        ["result::init", "()"].concat(),
        ["result::device", "::get_count"].concat(),
    ];
    for (line_index, line) in contents.lines().enumerate() {
        if forbidden.iter().any(|pattern| line.contains(pattern)) {
            findings.push(format!(
                "{}:{}: {}",
                relative.display(),
                line_index + 1,
                line.trim()
            ));
        }
    }
}
