//! Crate metadata release evidence for Vyre and Weir.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::release_train;
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
struct MetadataMatrix {
    schema_version: u32,
    publishable_package_count: usize,
    vyre_package_count: usize,
    weir_package_count: usize,
    parser_release_surface_count: usize,
    non_publishable_release_surface_count: usize,
    internal_tooling_count: usize,
    root_patch_section_count: usize,
    required_release_surfaces: Vec<&'static str>,
    missing_required_release_surfaces: Vec<String>,
    packages: Vec<PackageMetadata>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PackageMetadata {
    name: String,
    manifest: String,
    version: Option<String>,
    description: Option<String>,
    license: Option<String>,
    readme: Option<String>,
    repository: Option<String>,
    example_count: usize,
    example_files: Vec<String>,
    readme_example_count: usize,
    has_runnable_example: bool,
    has_api_referencing_example: bool,
    publish: Option<bool>,
    release_kind: &'static str,
    release_group: &'static str,
    release_surface: &'static str,
    expected_version: Option<&'static str>,
    publish_policy: &'static str,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct RequiredReleaseSurface {
    name: &'static str,
    expected_version: &'static str,
    release_kind: Option<&'static str>,
    release_surface: &'static str,
}

const MAX_MANIFEST_BYTES: u64 = 1_048_576;
const MAX_README_BYTES: u64 = 2_097_152;

fn required_release_surfaces() -> Vec<RequiredReleaseSurface> {
    vec![
    RequiredReleaseSurface {
        name: "vyre",
        expected_version: release_train::vyre_version(),
        release_kind: Some("publishable-crate"),
        release_surface: "vyre-engine",
    },
    RequiredReleaseSurface {
        name: "vyre-driver-cuda",
        expected_version: release_train::vyre_version(),
        release_kind: Some("publishable-crate"),
        release_surface: "cuda-backend",
    },
    RequiredReleaseSurface {
        name: "vyre-driver-wgpu",
        expected_version: release_train::vyre_version(),
        release_kind: Some("publishable-crate"),
        release_surface: "wgpu-backend",
    },
    RequiredReleaseSurface {
        name: "weir",
        expected_version: release_train::weir_version(),
        release_kind: Some("publishable-crate"),
        release_surface: "dataflow-analysis",
    },
    RequiredReleaseSurface {
        name: "vyrec",
        expected_version: release_train::vyrec_version(),
        release_kind: Some("non-publishable-release-surface"),
        release_surface: "parser-cli",
    },
    RequiredReleaseSurface {
        name: "vyre-frontend-c",
        expected_version: release_train::vyre_frontend_c_version(),
        release_kind: Some("non-publishable-release-surface"),
        release_surface: "c-frontend",
    },
    ]
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
    let santh_root = vyre_root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| vyre_root.clone());
    let mut packages = Vec::new();
    let mut metadata_blockers = Vec::new();
    let scan_roots = [vyre_root.clone(), santh_root.join("libs/dataflow/weir")];
    for root in &scan_roots {
        let workspace_package = load_workspace_package(root, &mut metadata_blockers);
        collect_packages(
            root,
            workspace_package.as_ref(),
            &mut packages,
            &mut metadata_blockers,
        );
    }
    let santh_workspace_package = load_workspace_package(&santh_root, &mut metadata_blockers);
    collect_packages(
        &santh_root.join("tools/vyrec"),
        santh_workspace_package.as_ref(),
        &mut packages,
        &mut metadata_blockers,
    );
    let (root_patch_section_count, patch_blockers) = root_patch_section_count(&[
        vyre_root.join("Cargo.toml"),
        santh_root.join("libs/dataflow/weir/Cargo.toml"),
        santh_root.join("tools/vyrec/Cargo.toml"),
    ]);
    packages.sort_by(|left, right| left.name.cmp(&right.name));
    let required_release_surfaces = required_release_surfaces()
        .into_iter()
        .map(|surface| surface.name)
        .collect::<Vec<_>>();
    let missing_required_release_surfaces = missing_required_release_surfaces(&packages);
    let mut blockers: Vec<String> = packages
        .iter()
        .flat_map(|package| {
            package
                .blockers
                .iter()
                .map(|blocker| format!("{}: {blocker}", package.name))
        })
        .collect();
    blockers.extend(metadata_blockers);
    blockers.extend(patch_blockers);
    blockers.extend(
        missing_required_release_surfaces
            .iter()
            .map(|surface| format!("missing required release surface `{surface}`")),
    );
    if root_patch_section_count > 0 {
        blockers.push(format!(
            "release manifests contain {root_patch_section_count} [patch.crates-io] section(s); remove root patches before publishing"
        ));
    }
    let publishable_package_count = packages
        .iter()
        .filter(|package| package.release_kind == "publishable-crate")
        .count();
    let vyre_package_count = packages
        .iter()
        .filter(|package| package.release_group == "vyre")
        .count();
    let weir_package_count = packages
        .iter()
        .filter(|package| package.release_group == "weir")
        .count();
    let parser_release_surface_count = packages
        .iter()
        .filter(|package| matches!(package.release_surface, "parser-cli" | "c-frontend"))
        .count();
    let internal_tooling_count = packages
        .iter()
        .filter(|package| package.release_kind == "internal-tooling")
        .count();
    let non_publishable_release_surface_count = packages
        .iter()
        .filter(|package| package.release_kind == "non-publishable-release-surface")
        .count();
    let matrix = MetadataMatrix {
        schema_version: 1,
        publishable_package_count,
        vyre_package_count,
        weir_package_count,
        parser_release_surface_count,
        non_publishable_release_surface_count,
        internal_tooling_count,
        root_patch_section_count,
        required_release_surfaces,
        missing_required_release_surfaces,
        packages,
        blockers,
    };
    let json = match serde_json::to_string_pretty(&matrix) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize metadata matrix: {error}");
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
    println!("metadata-matrix: wrote {}", output.display());
    if !matrix.blockers.is_empty() {
        std::process::exit(1);
    }
}

fn root_patch_section_count(manifests: &[PathBuf]) -> (usize, Vec<String>) {
    let mut blockers = Vec::new();
    let count = manifests
        .iter()
        .map(
            |manifest| match read_text_bounded(manifest, MAX_MANIFEST_BYTES) {
                Ok(text) => text
                    .lines()
                    .filter(|line| line.trim() == "[patch.crates-io]")
                    .count(),
                Err(error) => {
                    blockers.push(format!(
                        "failed to read manifest for patch scan `{}`: {error}",
                        manifest.display()
                    ));
                    0
                }
            },
        )
        .sum();
    (count, blockers)
}

fn collect_packages(
    root: &Path,
    workspace_package: Option<&toml::value::Table>,
    packages: &mut Vec<PackageMetadata>,
    blockers: &mut Vec<String>,
) {
    for entry in WalkDir::new(root).into_iter().filter_entry(|entry| {
        let name = entry.file_name().to_string_lossy();
        !matches!(
            name.as_ref(),
            "target"
                | "target-codex"
                | "target_tests"
                | ".git"
                | ".cargo-target"
                | "release"
                | "examples"
        )
    }) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                blockers.push(format!(
                    "failed to walk metadata root `{}`: {error}",
                    error
                        .path()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| root.display().to_string())
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) != Some("Cargo.toml") {
            continue;
        }
        match parse_package(path, workspace_package) {
            Ok(Some(package)) => packages.push(package),
            Ok(None) => {}
            Err(error) => blockers.push(error),
        }
    }
}

fn load_workspace_package(root: &Path, blockers: &mut Vec<String>) -> Option<toml::value::Table> {
    let manifest = root.join("Cargo.toml");
    let text = match read_text_bounded(&manifest, MAX_MANIFEST_BYTES) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "failed to read workspace package manifest `{}`: {error}",
                manifest.display()
            ));
            return None;
        }
    };
    let value = match toml::from_str::<toml::Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            blockers.push(format!(
                "failed to parse workspace package manifest `{}`: {error}",
                manifest.display()
            ));
            return None;
        }
    };
    value
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(toml::Value::as_table)
        .cloned()
}

fn parse_package(
    path: &Path,
    workspace_package: Option<&toml::value::Table>,
) -> Result<Option<PackageMetadata>, String> {
    let text = read_text_bounded(path, MAX_MANIFEST_BYTES).map_err(|error| {
        format!(
            "failed to read package manifest `{}`: {error}",
            path.display()
        )
    })?;
    let value = toml::from_str::<toml::Value>(&text).map_err(|error| {
        format!(
            "failed to parse package manifest `{}`: {error}",
            path.display()
        )
    })?;
    let Some(table) = value.get("package").and_then(toml::Value::as_table) else {
        return Ok(None);
    };
    let Some(name) = table
        .get("name")
        .and_then(toml::Value::as_str)
        .map(str::to_string)
    else {
        return Err(format!(
            "package manifest `{}` is missing package.name",
            path.display()
        ));
    };
    let version = inherited_string(table, workspace_package, "version");
    let description = inherited_string(table, workspace_package, "description");
    let license = inherited_string(table, workspace_package, "license");
    let readme = inherited_string(table, workspace_package, "readme");
    let repository = inherited_string(table, workspace_package, "repository");
    let publish = table.get("publish").and_then(toml::Value::as_bool);
    let release_kind = release_kind(&name, publish);
    let release_group = release_group(path, release_kind);
    let release_surface = release_surface(&name, release_group, release_kind);
    let expected_version = expected_version(&name, release_group, release_kind);
    let publish_policy = if release_kind == "internal-tooling" {
        "publish=false allowed for release tooling that is not a crates.io artifact"
    } else if release_kind == "non-publishable-release-surface" {
        "publish=false allowed for release-surface crates intentionally kept out of crates.io"
    } else {
        "publishable release crate"
    };
    let mut blockers = Vec::new();
    let internal_tooling = release_kind == "internal-tooling";
    if !internal_tooling && version.as_ref().is_none_or(|value| value.trim().is_empty()) {
        blockers.push("missing package.version".to_string());
    }
    if !internal_tooling
        && description
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
    {
        blockers.push("missing package.description".to_string());
    }
    if !internal_tooling && license.as_ref().is_none_or(|value| value.trim().is_empty()) {
        blockers.push("missing package.license".to_string());
    }
    if !internal_tooling
        && repository
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
    {
        blockers.push("missing package.repository".to_string());
    } else if repository
        .as_ref()
        .is_some_and(|value| !value.starts_with("https://"))
    {
        blockers.push("package.repository must be an https URL".to_string());
    }
    if !internal_tooling && readme.as_ref().is_none_or(|value| value.trim().is_empty()) {
        blockers.push("missing package.readme".to_string());
    }
    if release_kind == "publishable-crate" && publish == Some(false) {
        blockers.push("package.publish=false blocks release packaging".to_string());
    }
    if let Some(expected) = expected_version {
        if version.as_deref() != Some(expected) {
            blockers.push(format!(
                "package.version must be `{expected}` for {release_group} release"
            ));
        }
    }
    if let Some(readme) = readme.as_ref() {
        let readme_path = path.parent().unwrap_or_else(|| Path::new(".")).join(readme);
        if !readme_path.exists() {
            blockers.push(format!("readme `{readme}` does not exist"));
        } else {
            match read_text_bounded(&readme_path, MAX_README_BYTES) {
                Ok(text) if text.trim().is_empty() => {
                    blockers.push(format!("readme `{readme}` is empty"));
                }
                Ok(_) => {}
                Err(error) => blockers.push(format!("readme `{readme}` is unreadable: {error}")),
            }
        }
    }
    let examples = package_examples(path, &name, readme.as_deref());
    blockers.extend(examples.blockers);
    let example_count = examples.example_files.len() + examples.readme_example_count;
    if !internal_tooling && example_count == 0 {
        blockers.push(
            "missing examples: add examples/*.rs or README Rust/TOML/shell usage blocks"
                .to_string(),
        );
    }
    if release_kind == "publishable-crate" && !examples.has_runnable_example {
        blockers.push(
            "publishable release crate needs at least one runnable examples/*.rs file".to_string(),
        );
    }
    if release_kind == "publishable-crate" && !examples.has_api_referencing_example {
        blockers.push("publishable release crate needs at least one example that references the crate API or crate identity".to_string());
    }
    Ok(Some(PackageMetadata {
        name,
        manifest: path.display().to_string(),
        version,
        description,
        license,
        readme,
        repository,
        example_count,
        example_files: examples.example_files,
        readme_example_count: examples.readme_example_count,
        has_runnable_example: examples.has_runnable_example,
        has_api_referencing_example: examples.has_api_referencing_example,
        publish,
        release_kind,
        release_group,
        release_surface,
        expected_version,
        publish_policy,
        blockers,
    }))
}

struct PackageExamples {
    example_files: Vec<String>,
    readme_example_count: usize,
    has_runnable_example: bool,
    has_api_referencing_example: bool,
    blockers: Vec<String>,
}

fn package_examples(manifest: &Path, package_name: &str, readme: Option<&str>) -> PackageExamples {
    let root = manifest.parent().unwrap_or_else(|| Path::new("."));
    let mut blockers = Vec::new();
    let mut example_files = Vec::new();
    let examples_dir = root.join("examples");
    match fs::read_dir(&examples_dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        blockers.push(format!(
                            "failed to read examples directory entry under `{}`: {error}",
                            examples_dir.display()
                        ));
                        continue;
                    }
                };
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                    example_files.push(path);
                }
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => blockers.push(format!(
            "failed to read examples directory `{}`: {error}",
            examples_dir.display()
        )),
    }
    example_files.sort();
    let has_runnable_example =
        example_files
            .iter()
            .any(|path| match read_text_bounded(path, MAX_README_BYTES) {
                Ok(text) => text.contains("fn main(") || text.contains("fn main ()"),
                Err(error) => {
                    blockers.push(format!(
                        "example `{}` is unreadable: {error}",
                        path.display()
                    ));
                    false
                }
            });
    let crate_name = package_name.replace('-', "_");
    let has_api_referencing_example =
        example_files
            .iter()
            .any(|path| match read_text_bounded(path, MAX_README_BYTES) {
                Ok(text) => {
                    path.file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(|stem| stem.contains(&crate_name))
                        || text.contains(&crate_name)
                        || text.contains("::")
                        || text.contains("Command::new")
                }
                Err(error) => {
                    blockers.push(format!(
                        "example `{}` is unreadable: {error}",
                        path.display()
                    ));
                    false
                }
            });
    let readme_example_count = readme
        .map(
            |readme| match read_text_bounded(&root.join(readme), MAX_README_BYTES) {
                Ok(text) => {
                    text.matches("```rust").count()
                        + text.matches("```toml").count()
                        + text.matches("```bash").count()
                        + text.matches("```sh").count()
                }
                Err(error) => {
                    blockers.push(format!(
                        "readme `{readme}` is unreadable for example scan: {error}"
                    ));
                    0
                }
            },
        )
        .unwrap_or(0);
    PackageExamples {
        example_files: example_files
            .into_iter()
            .map(|path| path.display().to_string())
            .collect(),
        readme_example_count,
        has_runnable_example,
        has_api_referencing_example,
        blockers,
    }
}

fn missing_required_release_surfaces(packages: &[PackageMetadata]) -> Vec<String> {
    required_release_surfaces()
        .into_iter()
        .filter_map(|required| {
            let present = packages.iter().any(|package| {
                package.name == required.name
                    && package.version.as_deref() == Some(required.expected_version)
                    && package.release_surface == required.release_surface
                    && package.readme.as_deref() == Some("README.md")
                    && required
                        .release_kind
                        .is_none_or(|release_kind| package.release_kind == release_kind)
            });
            (!present).then(|| {
                format!(
                    "{}@{}:{}",
                    required.name, required.expected_version, required.release_surface
                )
            })
        })
        .collect()
}

fn inherited_string(
    table: &toml::value::Table,
    workspace_package: Option<&toml::value::Table>,
    key: &str,
) -> Option<String> {
    match table.get(key) {
        Some(value) => value
            .as_str()
            .map(str::to_string)
            .or_else(|| workspace_string_if_requested(value, workspace_package, key)),
        None => None,
    }
}

fn workspace_string_if_requested(
    value: &toml::Value,
    workspace_package: Option<&toml::value::Table>,
    key: &str,
) -> Option<String> {
    if value.get("workspace").and_then(toml::Value::as_bool) != Some(true) {
        return None;
    }
    workspace_package?
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
}

fn release_kind(name: &str, publish: Option<bool>) -> &'static str {
    if let Some(required) = required_release_surface(name) {
        return required.release_kind.unwrap_or("publishable-crate");
    }
    if matches!(
        name,
        "xtask"
            | "vyre-bench"
            | "vyre-bench-competitors"
            | "vyre-conform"
            | "vyre-conform-runner"
            | "vyre-test-harness"
            | "vyre-foundation-fuzz"
    ) {
        "internal-tooling"
    } else if publish == Some(false) {
        "internal-tooling"
    } else {
        "publishable-crate"
    }
}

fn required_release_surface(name: &str) -> Option<RequiredReleaseSurface> {
    required_release_surfaces()
        .into_iter()
        .find(|surface| surface.name == name)
}

fn release_group(path: &Path, release_kind: &str) -> &'static str {
    if release_kind == "internal-tooling" {
        return "internal-tooling";
    }
    if path
        .components()
        .any(|component| component.as_os_str().to_string_lossy() == "weir")
    {
        "weir"
    } else {
        "vyre"
    }
}

fn release_surface(name: &str, release_group: &str, release_kind: &str) -> &'static str {
    match name {
        "vyre" => "vyre-engine",
        "vyre-driver-cuda" => "cuda-backend",
        "vyre-driver-wgpu" => "wgpu-backend",
        "weir" => "dataflow-analysis",
        "vyrec" => "parser-cli",
        "vyre-frontend-c" => "c-frontend",
        _ if release_kind == "internal-tooling" => "internal-tooling",
        _ if release_group == "weir" => "weir-crate",
        _ => "vyre-crate",
    }
}

fn expected_version(name: &str, release_group: &str, release_kind: &str) -> Option<&'static str> {
    if release_kind == "internal-tooling" {
        return None;
    }
    if let Some(required) = required_release_surface(name) {
        return Some(required.expected_version);
    }
    release_train::release_group_version(release_group)
}

fn read_text_bounded(path: &Path, max_bytes: u64) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(max_bytes.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {max_bytes} byte release metadata read cap",
                path.display()
            ),
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
                    "USAGE:\n  cargo_full run --bin xtask -- metadata-matrix [--output PATH]\n\n\
                     Writes Vyre/Weir crate metadata evidence."
                );
                std::process::exit(0);
            }
            other => return Err(format!("Fix: unknown metadata-matrix option `{other}`.")),
        }
    }
    Ok(output.unwrap_or_else(default_output))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/metadata/metadata-matrix.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/metadata/metadata-matrix.json"))
}
