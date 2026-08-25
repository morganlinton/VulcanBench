//! `cargo_full run --bin xtask -- dep-drift`  -  verify workspace-managed dependency pins stay aligned.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use toml::Value;

const MAX_DEP_DRIFT_MANIFEST_BYTES: u64 = 1_048_576;

pub(crate) fn run(_args: &[String]) {
    let workspace_root = std::env::current_dir().expect(
        "Fix: xtask must run from the vyre workspace; restore this invariant before continuing.",
    );
    let repo_root = workspace_root
        .ancestors()
        .nth(4)
        .map(Path::to_path_buf)
        .expect("Fix: vyre workspace must remain nested under libs/performance/matching/vyre.");

    let workspace_manifest = workspace_root.join("Cargo.toml");
    let workspace_text = read_text_bounded(&workspace_manifest).unwrap_or_else(|error| {
        panic!(
            "Fix: failed to read workspace manifest {}: {error}",
            workspace_manifest.display()
        );
    });
    let workspace_toml = parse_manifest(&workspace_manifest, &workspace_text);

    let managed = managed_dependency_versions(&workspace_toml);
    let mut manifests = BTreeSet::new();
    let mut failures = Vec::new();
    collect_manifests(&workspace_root, &mut manifests, &mut failures);
    collect_manifests(
        &repo_root.join("libs/performance/matching/vyre/vyre-grammar-gen"),
        &mut manifests,
        &mut failures,
    );
    manifests.remove(&workspace_manifest);

    for manifest in manifests {
        let text = read_text_bounded(&manifest).unwrap_or_else(|error| {
            panic!("Fix: failed to read {}: {error}", manifest.display());
        });
        let parsed = parse_manifest(&manifest, &text);
        collect_manifest_failures(&manifest, &parsed, &managed, &mut failures);
    }

    if failures.is_empty() {
        println!("dep-drift: all workspace-managed dependency pins are aligned");
    } else {
        eprintln!("dep-drift: detected {} drift issue(s):", failures.len());
        for failure in failures {
            eprintln!("  - {failure}");
        }
        eprintln!("Fix: align every pinned version with the workspace-managed dependency table.");
        std::process::exit(1);
    }
}

fn parse_manifest(path: &Path, text: &str) -> Value {
    let table: toml::Table = toml::from_str(text).unwrap_or_else(|error| {
        panic!("Fix: failed to parse manifest {}: {error}", path.display());
    });
    Value::Table(table)
}

fn managed_dependency_versions(workspace_toml: &Value) -> BTreeMap<String, String> {
    workspace_toml
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(Value::as_table)
        .map(|dependencies| {
            dependencies
                .iter()
                .filter_map(|(name, value)| {
                    explicit_version(value).map(|version| (name.clone(), version))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn collect_manifests(root: &Path, sink: &mut BTreeSet<PathBuf>, failures: &mut Vec<String>) {
    if !root.exists() {
        failures.push(format!(
            "manifest scan root `{}` does not exist",
            root.display()
        ));
        return;
    }
    if root.ends_with("target") || root.ends_with(".git") {
        return;
    }
    let manifest = root.join("Cargo.toml");
    if manifest.exists() {
        sink.insert(manifest);
    }
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => {
            failures.push(format!(
                "could not read manifest scan directory `{}`: {error}",
                root.display()
            ));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                failures.push(format!(
                    "could not read manifest scan entry in `{}`: {error}",
                    root.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if matches!(name, "target" | ".git") {
            continue;
        }
        collect_manifests(&path, sink, failures);
    }
}

fn collect_manifest_failures(
    manifest_path: &Path,
    manifest: &Value,
    managed: &BTreeMap<String, String>,
    failures: &mut Vec<String>,
) {
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        check_dependency_table(
            manifest_path,
            section,
            manifest.get(section).and_then(Value::as_table),
            managed,
            failures,
        );
    }

    if let Some(targets) = manifest.get("target").and_then(Value::as_table) {
        for (target_name, target_table) in targets {
            let target = target_table.as_table();
            for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
                check_dependency_table(
                    manifest_path,
                    &format!("target.{target_name}.{section}"),
                    target
                        .and_then(|table| table.get(section))
                        .and_then(Value::as_table),
                    managed,
                    failures,
                );
            }
        }
    }
}

fn check_dependency_table(
    manifest_path: &Path,
    section: &str,
    table: Option<&toml::map::Map<String, Value>>,
    managed: &BTreeMap<String, String>,
    failures: &mut Vec<String>,
) {
    let Some(table) = table else {
        return;
    };
    for (dependency, spec) in table {
        let Some(managed_version) = managed.get(dependency) else {
            continue;
        };
        let Some(pinned_version) = explicit_version(spec) else {
            continue;
        };
        if &pinned_version != managed_version {
            failures.push(format!(
                "{}: `{}` in [{}] pins `{}` but the workspace manages `{}`",
                manifest_path.display(),
                dependency,
                section,
                pinned_version,
                managed_version
            ));
        }
    }
}

fn explicit_version(value: &Value) -> Option<String> {
    match value {
        Value::String(version) => Some(version.clone()),
        Value::Table(table) => {
            if table
                .get("workspace")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                return None;
            }
            table
                .get("version")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        }
        _ => None,
    }
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_DEP_DRIFT_MANIFEST_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_DEP_DRIFT_MANIFEST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_DEP_DRIFT_MANIFEST_BYTES} byte dependency drift manifest read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}
