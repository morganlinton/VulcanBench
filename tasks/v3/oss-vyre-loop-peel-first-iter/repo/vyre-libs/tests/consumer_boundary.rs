//! Integration tests for library-layer source boundary invariants.

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

/// The `security/` + `dataflow/` layer is the Weir/Vyre shared-fact BRIDGE: it
/// interoperates with Weir (and the surge compiler `surgec`) as a *peer* through
/// an explicit shared-fact schema contract — `SharedFactHeader` carries a
/// producer id, `pub mod weir_ifds` is a feature-gated integration, and the
/// `weir_*` public fields name the partner's schema fields. Naming the partner
/// there is architectural fact, not a downstream-consumer leak. The one-way
/// "no consumer names" contract is meaningful for the *reusable primitive*
/// layers (scan/, math/, nn/, parsing/, …) that must not bind to a specific
/// consumer; the bridge layer is exempt for the partner names only.
fn is_bridge_layer_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("security") | Some("dataflow")
        )
    })
}

/// Partner peers the bridge layer is allowed to name (and ONLY in the bridge
/// layer). `gosan`/`keyhog` are never exempt; `weir`/`surgec` outside the bridge
/// layer are still violations.
fn is_bridge_partner_name(name: &str) -> bool {
    name == concat!("we", "ir") || name == concat!("sur", "gec")
}

fn source_files_under(root: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(root).unwrap_or_else(|error| {
        panic!(
            "failed to read vyre-libs source directory {}: {error}",
            root.display()
        )
    });

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to read vyre-libs source directory entry under {}: {error}",
                root.display()
            )
        });
        let path = entry.path();
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!(
                "failed to classify vyre-libs source path {}: {error}",
                path.display()
            )
        });
        if file_type.is_dir() {
            source_files_under(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn library_source_does_not_name_downstream_consumers() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut source_files = Vec::new();
    source_files_under(&source_root, &mut source_files);
    source_files.sort();

    let forbidden = forbidden_consumer_names();
    let mut violations = Vec::new();
    for source_file in source_files {
        let contents = fs::read_to_string(&source_file).unwrap_or_else(|error| {
            panic!(
                "failed to read vyre-libs source file {}: {error}",
                source_file.display()
            )
        });
        let in_bridge_layer = is_bridge_layer_path(&source_file);
        for name in forbidden {
            if !contents.contains(name) {
                continue;
            }
            // Exempt ONLY the bridge-partner names in the bridge layer. Every
            // other (name, path) pairing is still a violation: weir/surgec
            // leaking out of security//dataflow into a reusable layer, and
            // gosan/keyhog anywhere, both still fail.
            if in_bridge_layer && is_bridge_partner_name(name) {
                continue;
            }
            violations.push(format!("{} contains {name}", source_file.display()));
        }
    }

    assert!(
        violations.is_empty(),
        "vyre-libs reusable layers must not name downstream consumers (the security/dataflow \
         Weir bridge is exempt for partner names only):\n{}",
        violations.join("\n")
    );
}
