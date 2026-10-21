//! Workspace-wide contract that GPU tests must fail loudly instead of silently skipping.

use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn gpu_required_tests_fail_loudly_instead_of_silently_skipping() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let vyre_root = manifest_dir
        .parent()
        .expect("vyre-foundation must live under the vyre workspace");

    let mut findings = Vec::new();
    scan_tree(vyre_root, &mut findings);
    for root in adjacent_dataflow_crate_roots(vyre_root) {
        scan_tree(&root, &mut findings);
    }

    assert!(
        findings.is_empty(),
        "GPU tests must fail loudly on adapter/probe failure instead of silently skipping:\n{}",
        findings.join("\n")
    );
}

#[test]
fn production_paths_do_not_convert_unsupported_gpu_features_into_none() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let vyre_root = manifest_dir
        .parent()
        .expect("vyre-foundation must live under the vyre workspace");
    let mut roots = vec![
        vyre_root.join("vyre-driver/src"),
        vyre_root.join("vyre-driver-wgpu/src"),
        vyre_root.join("vyre-runtime/src"),
    ];
    roots.extend(
        adjacent_dataflow_crate_roots(vyre_root)
            .into_iter()
            .map(|root| root.join("src")),
    );
    let mut findings = Vec::new();
    for root in roots {
        scan_production_tree_for_unsupported_feature_none(&root, &mut findings);
    }

    assert!(
        findings.is_empty(),
        "Production GPU paths must not hide unsupported resident/GPU features by returning None:\n{}",
        findings.join("\n")
    );
}

fn adjacent_dataflow_crate_roots(vyre_root: &Path) -> Vec<PathBuf> {
    let dataflow_root = vyre_root.join("../../../dataflow");
    assert!(
        dataflow_root.is_dir(),
        "adjacent dataflow root is missing at {}",
        dataflow_root.display()
    );
    let entries = fs::read_dir(&dataflow_root).unwrap_or_else(|err| {
        panic!(
            "failed to read adjacent dataflow root {}: {err}",
            dataflow_root.display()
        )
    });
    let mut roots = Vec::new();
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| {
            panic!(
                "failed to read entry in adjacent dataflow root {}: {err}",
                dataflow_root.display()
            )
        });
        let root = entry.path();
        if root.join("src").is_dir() {
            roots.push(root);
        }
    }
    assert!(
        !roots.is_empty(),
        "adjacent dataflow root {} must contain at least one crate src/ tree",
        dataflow_root.display()
    );
    roots
}

fn scan_tree(root: &Path, findings: &mut Vec<String>) {
    if !root.exists() {
        return;
    }

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
                if path.file_name().and_then(|name| name.to_str()) != Some("target") {
                    stack.push(path);
                }
                continue;
            }

            if file_type.is_file() && is_test_rust_file(root, &path) {
                scan_file(root, &path, findings);
            }
        }
    }
}

fn scan_production_tree_for_unsupported_feature_none(root: &Path, findings: &mut Vec<String>) {
    if !root.exists() {
        return;
    }

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
                if path.file_name().and_then(|name| name.to_str()) != Some("target") {
                    stack.push(path);
                }
                continue;
            }

            if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                scan_production_file_for_unsupported_feature_none(root, &path, findings);
            }
        }
    }
}

fn scan_production_file_for_unsupported_feature_none(
    root: &Path,
    path: &Path,
    findings: &mut Vec<String>,
) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let compact = contents
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let hidden_none = ["UnsupportedFeature", "=>", "return", "Ok", "(None)"].concat();
    if compact.contains(&hidden_none) {
        let relative = path.strip_prefix(root).unwrap_or(path);
        findings.push(format!(
            "{}: converts UnsupportedFeature into Ok(None); return a loud error instead",
            relative.display()
        ));
    }
}

fn is_test_rust_file(root: &Path, path: &Path) -> bool {
    if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return false;
    }

    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    name.ends_with("_test.rs")
        || path
            .strip_prefix(root)
            .expect("scanned path must be under scan root")
            .components()
            .any(|component| component.as_os_str() == "tests")
}

fn scan_file(root: &Path, path: &Path, findings: &mut Vec<String>) {
    if is_loudness_gate_source(path) {
        return;
    }

    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));

    scan_contents(root, path, &contents, findings);
}

fn is_loudness_gate_source(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("gpu_test_loudness_workspace.rs")
}

fn is_silent_gpu_skip(line: &str) -> bool {
    let mentions_gpu_absence =
        line.contains("no gpu") || line.contains("gpu unavailable") || line.contains("no adapter");
    let prints_skip =
        (line.contains("println!") || line.contains("eprintln!")) && mentions_gpu_absence;
    let returns_on_probe_error =
        line.contains("is_err()") && (line.contains("return ok(())") || line.contains("return;"));
    let comments_skip_return =
        mentions_gpu_absence && (line.contains("return ok(())") || line.contains("return;"));

    prints_skip || returns_on_probe_error || comments_skip_return
}

#[test]
fn loud_probe_token_does_not_mask_silent_gpu_skip_later_in_file() {
    let source = [
        "fn helper() { require_gpu(); }\n",
        "fn bad() {\n",
        "    if acquire_adapter().is_err() { return ",
        "Ok",
        "(()); }\n",
        "}\n",
    ]
    .concat();
    let mut findings = Vec::new();
    let root = Path::new("synthetic");
    let path = root.join("tests/gpu_mask.rs");

    scan_contents(root, &path, &source, &mut findings);

    assert_eq!(
        findings.len(),
        1,
        "a loud probe token must not exempt later silent no-GPU skip paths"
    );
}

fn scan_contents(root: &Path, path: &Path, contents: &str, findings: &mut Vec<String>) {
    for (line_index, line) in contents.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        if is_silent_gpu_skip(&lower) {
            let relative = path.strip_prefix(root).unwrap_or(path);
            findings.push(format!(
                "{}:{}: {}",
                relative.display(),
                line_index + 1,
                line.trim()
            ));
        }
    }
}
