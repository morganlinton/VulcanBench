//! Workspace-level platform documentation boundary contract.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

const PLATFORM_CRATES: &[&str] = &[
    "vyre-core",
    "vyre-spec",
    "vyre-macros",
    "vyre-foundation",
    "vyre-primitives",
    "vyre-intrinsics",
    "vyre-libs",
    "vyre-reference",
    "vyre-driver",
    "vyre-driver-cuda",
    "vyre-driver-wgpu",
    "vyre-driver-spirv",
    "vyre-runtime",
];

const SELF_SUBSTRATE_PLATFORM_DIRS: &[&str] = &[
    "analysis",
    "data",
    "graph",
    "hardware",
    "logic",
    "math",
    "optimization",
    "optimizer",
    "scheduling",
    "telemetry",
];

fn forbidden_consumer_names() -> [&'static str; 4] {
    [
        concat!("we", "ir"),
        concat!("sur", "gec"),
        concat!("gos", "san"),
        concat!("key", "hog"),
    ]
}

#[test]
fn platform_crate_docs_and_comments_do_not_name_consumers() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .expect("vyre-self-substrate should live directly under the workspace root");
    let script = workspace.join("scripts/check_platform_consumer_docs.sh");

    let output = Command::new("bash")
        .arg(&script)
        .current_dir(workspace)
        .output()
        .expect("platform consumer-doc boundary script should execute");

    assert!(
        output.status.success(),
        "platform consumer-doc boundary failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).as_ref(),
        "",
        "platform consumer-doc boundary must be quiet on success"
    );
}

#[test]
fn docs_index_covers_every_public_markdown_document() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .expect("vyre-self-substrate should live directly under the workspace root");
    let script = workspace.join("scripts/check_docs_index.sh");

    let output = Command::new("bash")
        .arg(&script)
        .current_dir(workspace)
        .output()
        .expect("documentation index contract script should execute");

    assert!(
        output.status.success(),
        "documentation index contract failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).as_ref(),
        "",
        "documentation index contract must be quiet on success"
    );
}

#[test]
fn roadmap_status_and_changelog_are_separate_contracts() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .expect("vyre-self-substrate should live directly under the workspace root");
    let script = workspace.join("scripts/check_roadmap_status_split.sh");

    let output = Command::new("bash")
        .arg(&script)
        .current_dir(workspace)
        .output()
        .expect("roadmap/status split contract script should execute");

    assert!(
        output.status.success(),
        "roadmap/status split contract failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).as_ref(),
        "",
        "roadmap/status split contract must be quiet on success"
    );
}

#[test]
fn platform_crate_source_does_not_name_consumers() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .expect("vyre-self-substrate should live directly under the workspace root");
    let forbidden = forbidden_consumer_names();
    let mut source_files = Vec::new();

    for crate_name in PLATFORM_CRATES {
        collect_rust_sources(&workspace.join(crate_name).join("src"), &mut source_files);
    }
    collect_rust_sources(&manifest.join("src").join("lib.rs"), &mut source_files);
    for dir in SELF_SUBSTRATE_PLATFORM_DIRS {
        collect_rust_sources(&manifest.join("src").join(dir), &mut source_files);
    }
    source_files.sort();

    let mut violations = Vec::new();
    for source_file in source_files {
        let source = fs::read_to_string(&source_file)
            .unwrap_or_else(|err| panic!("{} must be readable: {err}", source_file.display()));
        let lower = source.to_lowercase();
        for name in forbidden {
            if lower.contains(name) {
                violations.push(format!("{} contains {name}", source_file.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "platform crate source must not name downstream consumers:\n{}",
        violations.join("\n")
    );
}

fn collect_rust_sources(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path.to_path_buf());
        }
        return;
    }
    if !path.is_dir() {
        return;
    }
    let entries = fs::read_dir(path).unwrap_or_else(|err| {
        panic!(
            "{} source directory must be readable: {err}",
            path.display()
        )
    });
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| {
            panic!(
                "{} source directory entry must be readable: {err}",
                path.display()
            )
        });
        let child = entry.path();
        if child.is_dir() {
            collect_rust_sources(&child, out);
        } else if child.extension().is_some_and(|extension| extension == "rs") {
            out.push(child);
        }
    }
}
