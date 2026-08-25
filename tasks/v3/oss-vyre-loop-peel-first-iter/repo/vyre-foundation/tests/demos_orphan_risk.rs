//! Demos / examples orphan-risk contract.
//!
//! Example crates must not silently drift from the workspace. They must either
//! be workspace members, declare themselves as standalone workspaces, or be
//! explicitly baselined as external. Removed demos must not leave stale
//! references in the root Cargo.toml.

use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn examples_are_in_workspace_or_standalone() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();
    let examples_dir = workspace_root.join("examples");
    if !examples_dir.is_dir() {
        return;
    }

    let root_toml = std::fs::read_to_string(workspace_root.join("Cargo.toml")).unwrap();

    let mut violations = Vec::new();
    for entry in std::fs::read_dir(&examples_dir).unwrap().flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let cargo_toml = path.join("Cargo.toml");
        if !cargo_toml.exists() {
            // three_substrate_parity has no Cargo.toml  -  baseline it.
            if name != "three_substrate_parity" {
                violations.push(format!("{}: missing Cargo.toml", name));
            }
            continue;
        }
        let content = std::fs::read_to_string(&cargo_toml).unwrap();
        let in_workspace = root_toml.contains(&format!("\"examples/{}\"", name));
        let is_standalone = content.contains("[workspace]");
        let is_template = name == "libs-template"; // cargo-generate template, not buildable

        if !in_workspace && !is_standalone && !is_template {
            violations.push(format!(
                "{}: not in workspace members and missing [workspace] declaration",
                name
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "example crates must be workspace members or standalone. Violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn examples_have_tests_or_are_exempt() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();
    let examples_dir = workspace_root.join("examples");
    if !examples_dir.is_dir() {
        return;
    }

    let exempt: HashSet<String> = [
        "external_ir_extension".to_string(), // demo extension, not test-bearing
        "libs-template".to_string(),         // cargo-generate template, not buildable
        "three_substrate_parity".to_string(), // manifest-only parity demo
    ]
    .iter()
    .cloned()
    .collect();

    let mut violations = Vec::new();
    for entry in std::fs::read_dir(&examples_dir).unwrap().flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if exempt.contains(&name) {
            continue;
        }
        let tests_dir = path.join("tests");
        if !tests_dir.exists() {
            violations.push(format!("{}: missing tests/ directory", name));
        }
    }

    assert!(
        violations.is_empty(),
        "example crates must have tests/ or be exempt. Violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn no_stale_demo_references_in_workspace_toml() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();
    let root_toml = std::fs::read_to_string(workspace_root.join("Cargo.toml")).unwrap();

    // Stale references to removed demo directories should not persist forever.
    // Baseline: these demos were removed in 0.6 but are still mentioned in
    // workspace Cargo.toml comments. The references serve as historical context.
    let known_stale: HashSet<String> = ["demos/rust_lexer_gpu", "demos/rust_parser_gpu"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut violations = Vec::new();
    for line in root_toml.lines() {
        if line.contains("demos/") {
            let is_known = known_stale.iter().any(|pat| line.contains(pat));
            if !is_known {
                violations.push(format!(
                    "workspace Cargo.toml references unexpected demo path: {}",
                    line.trim()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "workspace Cargo.toml must not reference unexpected demo paths. Violations:\n{}",
        violations.join("\n")
    );
}
