//! Pre-publish package graph evidence for the Vyre / Weir release train.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::release_train;

const MAX_JSON_BYTES: u64 = 8_388_608;
const MAX_MANIFEST_BYTES: u64 = 1_048_576;

#[derive(Debug, Serialize)]
struct PackageReadiness {
    schema_version: u32,
    release_train: ReleaseTrain,
    publish_order: Vec<PublishStep>,
    non_publish_release_surfaces: Vec<ReleaseSurface>,
    package_verify_passed: Vec<&'static str>,
    observed_package_failures: Vec<ObservedPackageFailure>,
    missing_metadata_packages: Vec<String>,
    extra_metadata_packages: Vec<String>,
    dependency_order_edges: Vec<DependencyEdge>,
    versioned_local_dependencies: Vec<VersionedLocalDependency>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ReleaseTrain {
    vyre: &'static str,
    weir: &'static str,
    vyrec: &'static str,
    cuda_release_path: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct PublishStep {
    package: &'static str,
    version: &'static str,
    manifest: &'static str,
}

#[derive(Debug, Serialize)]
struct ReleaseSurface {
    package: &'static str,
    version: &'static str,
    surface: &'static str,
    reason: &'static str,
}

#[derive(Debug, Serialize)]
struct ObservedPackageFailure {
    package: &'static str,
    command: &'static str,
    reason: &'static str,
}

#[derive(Debug, Serialize)]
struct DependencyEdge {
    package: String,
    dependency: String,
    dependency_version: String,
    manifest: String,
}

#[derive(Debug, Serialize)]
struct VersionedLocalDependency {
    package: String,
    dependency: String,
    version: String,
    manifest: String,
    source: &'static str,
}

fn publish_order() -> Vec<PublishStep> {
    vec![
    step("vyre-macros", release_train::vyre_version(), "vyre-macros/Cargo.toml"),
    step("vyre-spec", release_train::vyre_version(), "vyre-spec/Cargo.toml"),
    step("vyre-lints", release_train::vyre_version(), "vyre-lints/Cargo.toml"),
    step("vyre-foundation", release_train::vyre_version(), "vyre-foundation/Cargo.toml"),
    step("vyre-lower", release_train::vyre_version(), "vyre-lower/Cargo.toml"),
    step("vyre-emit-ptx", release_train::vyre_version(), "vyre-emit-ptx/Cargo.toml"),
    step("vyre-primitives", release_train::vyre_version(), "vyre-primitives/Cargo.toml"),
    step("vyre-reference", release_train::vyre_version(), "vyre-reference/Cargo.toml"),
    step(
        "vyre-self-substrate",
        release_train::vyre_version(),
        "vyre-self-substrate/Cargo.toml",
    ),
    step("vyre-driver", release_train::vyre_version(), "vyre-driver/Cargo.toml"),
    step("vyre-runtime", release_train::vyre_version(), "vyre-runtime/Cargo.toml"),
    step("vyre-emit-naga", release_train::vyre_version(), "vyre-emit-naga/Cargo.toml"),
    step("vyre-emit-metal", release_train::vyre_version(), "vyre-emit-metal/Cargo.toml"),
    step("vyre-driver-cuda", release_train::vyre_version(), "vyre-driver-cuda/Cargo.toml"),
    step("vyre-driver-wgpu", release_train::vyre_version(), "vyre-driver-wgpu/Cargo.toml"),
    step("vyre-driver-metal", release_train::vyre_version(), "vyre-driver-metal/Cargo.toml"),
    step("vyre-driver-spirv", release_train::vyre_version(), "vyre-driver-spirv/Cargo.toml"),
    step(
        "vyre-driver-reference",
        release_train::vyre_version(),
        "vyre-driver-reference/Cargo.toml",
    ),
    step("vyre", release_train::vyre_version(), "vyre-core/Cargo.toml"),
    step("vyre-harness", release_train::vyre_version(), "vyre-harness/Cargo.toml"),
    step("weir", release_train::weir_version(), "../../../dataflow/weir/Cargo.toml"),
    step("vyre-intrinsics", release_train::vyre_version(), "vyre-intrinsics/Cargo.toml"),
    step("vyre-libs", release_train::vyre_version(), "vyre-libs/Cargo.toml"),
    step("vyre-debug", release_train::vyre_version(), "vyre-debug/Cargo.toml"),
    step("vyre-aot", release_train::vyre_version(), "vyre-aot/Cargo.toml"),
    step("vyre-emit-spirv", release_train::vyre_version(), "vyre-emit-spirv/Cargo.toml"),
    ]
}

const fn step(package: &'static str, version: &'static str, manifest: &'static str) -> PublishStep {
    PublishStep {
        package,
        version,
        manifest,
    }
}

pub(crate) fn run(args: &[String]) {
    let output = match parse_output(args) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let vyre_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let metadata_path = vyre_root.join("release/evidence/metadata/metadata-matrix.json");
    let mut blockers = Vec::new();
    let metadata_packages = metadata_publishable_packages(&metadata_path, &mut blockers);
    let publish_order = publish_order();
    let ordered_packages = publish_order
        .iter()
        .map(|step| step.package.to_string())
        .collect::<BTreeSet<_>>();
    let missing_metadata_packages = metadata_packages
        .difference(&ordered_packages)
        .cloned()
        .collect::<Vec<_>>();
    let extra_metadata_packages = ordered_packages
        .difference(&metadata_packages)
        .cloned()
        .collect::<Vec<_>>();
    for package in &missing_metadata_packages {
        blockers.push(format!(
            "metadata publishable package `{package}` is missing from publish_order"
        ));
    }
    for package in &extra_metadata_packages {
        blockers.push(format!(
            "publish_order package `{package}` is not publishable in metadata matrix"
        ));
    }

    let order_index = publish_order
        .iter()
        .enumerate()
        .map(|(index, step)| (step.package, index))
        .collect::<BTreeMap<_, _>>();
    let mut dependency_order_edges = Vec::new();
    let mut versioned_local_dependencies = Vec::new();
    for (consumer_index, step) in publish_order.iter().enumerate() {
        let manifest = vyre_root.join(step.manifest);
        check_manifest_package(step, &manifest, &mut blockers);
        collect_dependency_edges(
            step,
            consumer_index,
            &manifest,
            &order_index,
            &mut dependency_order_edges,
            &mut versioned_local_dependencies,
            &mut blockers,
        );
    }

    dependency_order_edges.sort_by(|left, right| {
        left.package
            .cmp(&right.package)
            .then(left.dependency.cmp(&right.dependency))
    });
    versioned_local_dependencies.sort_by(|left, right| {
        left.package
            .cmp(&right.package)
            .then(left.dependency.cmp(&right.dependency))
    });

    let readiness = PackageReadiness {
        schema_version: 1,
        release_train: ReleaseTrain {
            vyre: release_train::vyre_version(),
            weir: release_train::weir_version(),
            vyrec: release_train::vyrec_train_version(),
            cuda_release_path: true,
        },
        publish_order,
        non_publish_release_surfaces: vec![
            ReleaseSurface {
                package: "vyre-frontend-c",
                version: release_train::vyre_frontend_c_version(),
                surface: "c-frontend",
                reason: "library release surface for beta Vyrec, intentionally not published as a standalone crate",
            },
            ReleaseSurface {
                package: "vyrec",
                version: release_train::vyrec_version(),
                surface: "parser-cli",
                reason: "beta compiler CLI release surface, intentionally not published to crates.io in this release",
            },
        ],
        package_verify_passed: release_train::package_verify_passed(),
        observed_package_failures: vec![
            ObservedPackageFailure {
                package: "vyre-lower@0.6.3",
                command: "cargo_full package --allow-dirty --manifest-path vyre-lower/Cargo.toml",
                reason: "crates.io does not yet contain vyre-foundation@0.6.3",
            },
            ObservedPackageFailure {
                package: "weir@0.1.0",
                command: "cargo_full package --allow-dirty --manifest-path libs/dataflow/weir/Cargo.toml",
                reason: "crates.io does not yet contain vyre@0.6.3",
            },
        ],
        missing_metadata_packages,
        extra_metadata_packages,
        dependency_order_edges,
        versioned_local_dependencies,
        blockers,
    };
    let json = match serde_json::to_string_pretty(&readiness) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize package readiness evidence: {error}");
            std::process::exit(1);
        }
    };
    if let Some(parent) = output.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("Fix: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    if let Err(error) = fs::write(&output, format!("{json}\n")) {
        eprintln!("Fix: failed to write `{}`: {error}", output.display());
        std::process::exit(1);
    }
    println!("package-readiness: wrote {}", output.display());
    if !readiness.blockers.is_empty() {
        std::process::exit(1);
    }
}

fn metadata_publishable_packages(path: &Path, blockers: &mut Vec<String>) -> BTreeSet<String> {
    let text = match read_text_bounded(path, MAX_JSON_BYTES) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "failed to read metadata matrix `{}`: {error}",
                path.display()
            ));
            return BTreeSet::new();
        }
    };
    let value = match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            blockers.push(format!(
                "failed to parse metadata matrix `{}`: {error}",
                path.display()
            ));
            return BTreeSet::new();
        }
    };
    value
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|package| {
            package
                .get("release_kind")
                .and_then(serde_json::Value::as_str)
                == Some("publishable-crate")
        })
        .filter_map(|package| package.get("name").and_then(serde_json::Value::as_str))
        .map(str::to_string)
        .collect()
}

fn check_manifest_package(step: &PublishStep, manifest: &Path, blockers: &mut Vec<String>) {
    let value = match read_manifest(manifest, blockers) {
        Some(value) => value,
        None => return,
    };
    let Some(package) = value.get("package").and_then(toml::Value::as_table) else {
        blockers.push(format!("{} has no [package] table", manifest.display()));
        return;
    };
    if package.get("name").and_then(toml::Value::as_str) != Some(step.package) {
        blockers.push(format!(
            "{} package.name does not match publish_order `{}`",
            manifest.display(),
            step.package
        ));
    }
    if package_version(package) != Some(step.version) {
        blockers.push(format!(
            "{} package.version does not match publish_order `{}`",
            manifest.display(),
            step.version
        ));
    }
    if package.get("publish").and_then(toml::Value::as_bool) == Some(false) {
        blockers.push(format!(
            "{} is publish=false but appears in publish_order",
            step.package
        ));
    }
}

fn collect_dependency_edges(
    step: &PublishStep,
    consumer_index: usize,
    manifest: &Path,
    order_index: &BTreeMap<&'static str, usize>,
    dependency_order_edges: &mut Vec<DependencyEdge>,
    versioned_local_dependencies: &mut Vec<VersionedLocalDependency>,
    blockers: &mut Vec<String>,
) {
    let value = match read_manifest(manifest, blockers) {
        Some(value) => value,
        None => return,
    };
    let workspace_dependencies = workspace_dependencies(manifest, blockers);
    for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(table) = value.get(table_name).and_then(toml::Value::as_table) else {
            continue;
        };
        for (dependency, spec) in table {
            let Some(local_path) =
                dependency_has_local_path(spec, &workspace_dependencies, dependency)
            else {
                continue;
            };
            let Some(version) = dependency_version(spec, &workspace_dependencies, dependency)
            else {
                blockers.push(format!(
                    "{} dependency `{dependency}` in [{table_name}] has local path `{local_path}` but no crates.io version",
                    manifest.display()
                ));
                continue;
            };
            versioned_local_dependencies.push(VersionedLocalDependency {
                package: step.package.to_string(),
                dependency: dependency.clone(),
                version: version.clone(),
                manifest: manifest.display().to_string(),
                source: if dependency_uses_workspace(spec) {
                    "workspace"
                } else {
                    "manifest"
                },
            });
            if table_name != "dev-dependencies" {
                if let Some(dependency_index) = order_index.get(dependency.as_str()) {
                    dependency_order_edges.push(DependencyEdge {
                        package: step.package.to_string(),
                        dependency: dependency.clone(),
                        dependency_version: version,
                        manifest: manifest.display().to_string(),
                    });
                    if *dependency_index >= consumer_index {
                        blockers.push(format!(
                            "publish_order puts `{}` before dependency `{dependency}`",
                            step.package
                        ));
                    }
                }
            }
        }
    }
}

fn workspace_dependencies(
    manifest: &Path,
    blockers: &mut Vec<String>,
) -> BTreeMap<String, toml::Value> {
    let Some(root) = workspace_root_for_manifest(manifest) else {
        return BTreeMap::new();
    };
    let value = match read_manifest(&root.join("Cargo.toml"), blockers) {
        Some(value) => value,
        None => return BTreeMap::new(),
    };
    value
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
        .map(|table| {
            table
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn workspace_root_for_manifest(manifest: &Path) -> Option<PathBuf> {
    for ancestor in manifest.ancestors().skip(1) {
        let candidate = ancestor.join("Cargo.toml");
        let Ok(text) = read_text_bounded(&candidate, MAX_MANIFEST_BYTES) else {
            continue;
        };
        let Ok(value) = toml::from_str::<toml::Value>(&text) else {
            continue;
        };
        if value.get("workspace").is_some() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

fn dependency_has_local_path(
    spec: &toml::Value,
    workspace_dependencies: &BTreeMap<String, toml::Value>,
    dependency: &str,
) -> Option<String> {
    if let Some(path) = spec.get("path").and_then(toml::Value::as_str) {
        return Some(path.to_string());
    }
    if dependency_uses_workspace(spec) {
        return workspace_dependencies
            .get(dependency)
            .and_then(|value| value.get("path"))
            .and_then(toml::Value::as_str)
            .map(str::to_string);
    }
    None
}

fn dependency_version(
    spec: &toml::Value,
    workspace_dependencies: &BTreeMap<String, toml::Value>,
    dependency: &str,
) -> Option<String> {
    spec.as_str()
        .map(str::to_string)
        .or_else(|| {
            spec.get("version")
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            dependency_uses_workspace(spec).then(|| {
                workspace_dependencies
                    .get(dependency)
                    .and_then(|value| {
                        value
                            .as_str()
                            .or_else(|| value.get("version").and_then(toml::Value::as_str))
                    })
                    .map(str::to_string)
            })?
        })
}

fn dependency_uses_workspace(spec: &toml::Value) -> bool {
    spec.get("workspace").and_then(toml::Value::as_bool) == Some(true)
}

fn package_version(package: &toml::value::Table) -> Option<&str> {
    package.get("version").and_then(toml::Value::as_str)
}

fn read_manifest(path: &Path, blockers: &mut Vec<String>) -> Option<toml::Value> {
    let text = match read_text_bounded(path, MAX_MANIFEST_BYTES) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "failed to read manifest `{}`: {error}",
                path.display()
            ));
            return None;
        }
    };
    match toml::from_str::<toml::Value>(&text) {
        Ok(value) => Some(value),
        Err(error) => {
            blockers.push(format!(
                "failed to parse manifest `{}`: {error}",
                path.display()
            ));
            None
        }
    }
}

fn read_text_bounded(path: &Path, max_bytes: u64) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(max_bytes.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} exceeds {max_bytes} byte read cap", path.display()),
        ));
    }
    Ok(text)
}

fn parse_output(args: &[String]) -> Result<PathBuf, String> {
    let mut output = None;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: --output requires a path.".to_string());
                };
                output = Some(PathBuf::from(path));
                index += 2;
            }
            "--help" | "-h" => {
                println!(
                    "USAGE:\n  cargo_full run --bin xtask -- package-readiness [--output PATH]\n\n\
                     Writes pre-publish package-order evidence."
                );
                std::process::exit(0);
            }
            other => return Err(format!("Fix: unknown package-readiness option `{other}`.")),
        }
    }
    Ok(output.unwrap_or_else(default_output))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/package/publish-readiness.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/package/publish-readiness.json"))
}
