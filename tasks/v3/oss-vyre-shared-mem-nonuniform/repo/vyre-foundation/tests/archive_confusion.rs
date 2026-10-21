//! Archive boundary contract.
//!
//! `.internals/archive/` must have a clear retention policy and not silently
//! accumulate unbounded material. New subdirectories or top-level files must
//! be baselined.

use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn archive_has_boundary_documentation() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let archive = manifest.parent().unwrap().join(".internals/archive");
    if !archive.is_dir() {
        return;
    }
    let readme = archive.join("README.md");
    assert!(
        readme.exists(),
        ".internals/archive/ must contain a README.md explaining retention policy and boundaries"
    );
}

#[test]
fn archive_subdirectories_are_baselined() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let archive = manifest.parent().unwrap().join(".internals/archive");
    if !archive.is_dir() {
        return;
    }

    let mut found = Vec::new();
    for entry in std::fs::read_dir(&archive).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.push(path.file_name().unwrap().to_string_lossy().to_string());
        }
    }
    found.sort();

    let known: HashSet<String> = [
        "audits-deprecated",
        "coordination",
        "vyre-core-docs-2026-04-18",
        // Audit cleanup A2 (2026-04-30): retired `.internals/planning/`
        // + `.internals/plans/` whose dated execution-trace contents
        // landed here. Durable design docs went to `docs/` instead.
        "2026-04",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let new_violations: Vec<String> = found.into_iter().filter(|v| !known.contains(v)).collect();

    assert!(
        new_violations.is_empty(),
        "new subdirectories in .internals/archive/ must be baselined. Violations: {:?}",
        new_violations
    );
}

#[test]
fn archive_top_level_files_are_baselined() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let archive = manifest.parent().unwrap().join(".internals/archive");
    if !archive.is_dir() {
        return;
    }

    let mut found = Vec::new();
    for entry in std::fs::read_dir(&archive).unwrap().flatten() {
        let path = entry.path();
        if path.is_file() {
            found.push(path.file_name().unwrap().to_string_lossy().to_string());
        }
    }
    found.sort();

    let known: HashSet<String> = ["README.md"].iter().map(|s| s.to_string()).collect();

    let new_violations: Vec<String> = found.into_iter().filter(|v| !known.contains(v)).collect();

    assert!(
        new_violations.is_empty(),
        "new top-level files in .internals/archive/ must be baselined. Violations: {:?}",
        new_violations
    );
}
