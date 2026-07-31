//! Integration tests for self-substrate source boundary invariants.

use std::fs;
use std::path::{Path, PathBuf};

fn forbidden_consumer_names() -> [&'static str; 4] {
    [
        concat!("we", "ir"),
        concat!("sur", "gec"),
        concat!("gos", "san"),
        concat!("key", "hog"),
    ]
}

fn source_files_under(root: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(root).unwrap_or_else(|error| {
        panic!(
            "failed to read vyre-self-substrate source directory {}: {error}",
            root.display()
        )
    });

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to read vyre-self-substrate source directory entry under {}: {error}",
                root.display()
            )
        });
        let path = entry.path();
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!(
                "failed to classify vyre-self-substrate source path {}: {error}",
                path.display()
            )
        });
        if file_type.is_dir() {
            // Skip archived material that is intentionally historical.
            if path.ends_with("archive") || path.ends_with("release") {
                continue;
            }
            source_files_under(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn self_substrate_source_does_not_name_downstream_consumers() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut source_files = Vec::new();
    source_files_under(&source_root, &mut source_files);
    source_files.sort();

    let forbidden = forbidden_consumer_names();
    let mut violations = Vec::new();
    for source_file in source_files {
        let contents = fs::read_to_string(&source_file).unwrap_or_else(|error| {
            panic!(
                "failed to read vyre-self-substrate source file {}: {error}",
                source_file.display()
            )
        });
        for name in forbidden {
            if contents.contains(name) {
                violations.push(format!("{} contains {name}", source_file.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "vyre-self-substrate is a platform substrate crate and must not name downstream consumers:\n{}",
        violations.join("\n")
    );
}
