//! Workspace package/path naming footgun detection.
//!
//! Path dependencies that escape the workspace root and directory-name vs
//! package-name mismatches confuse tooling and break hermetic builds.

use std::path::PathBuf;

#[test]
fn no_path_dependencies_outside_workspace() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();
    let root_toml = workspace_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&root_toml).unwrap();

    let mut violations = Vec::new();
    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if (trimmed.contains("path = \"../") || trimmed.contains("path = \"../../"))
            && !is_adjacent_dataflow_dependency(trimmed)
        {
            violations.push(format!("line {}: {}", line_no + 1, trimmed));
        }
    }

    assert!(
        violations.is_empty(),
        "workspace Cargo.toml must not reference paths outside the workspace (baseline known exceptions). Violations:\n{}",
        violations.join("\n")
    );
}

fn is_adjacent_dataflow_dependency(line: &str) -> bool {
    line.contains("path = \"../../../dataflow/")
}

#[test]
fn meta_crate_directory_naming_is_stable() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    // vyre-core/ directory hosts the meta-crate named "vyre".
    // This is intentional but must remain stable.
    let core_toml = workspace_root.join("vyre-core/Cargo.toml");
    let content = std::fs::read_to_string(&core_toml).unwrap();
    assert!(
        content.contains("name = \"vyre\""),
        "vyre-core directory must host the meta-crate named 'vyre'"
    );

    // Workspace dependency routing must keep the meta-crate name bound to
    // the vyre-core directory.
    let root_toml = workspace_root.join("Cargo.toml");
    let root_content = std::fs::read_to_string(&root_toml).unwrap();
    assert!(
        root_content.lines().any(
            |line| line.trim().starts_with("vyre = {") && line.contains("path = \"vyre-core\"")
        ),
        "workspace dependency routing must route 'vyre' to 'vyre-core' directory"
    );
}
