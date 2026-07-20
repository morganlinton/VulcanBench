//! Generate release version-story evidence for Vyre and Weir.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::release_train;

const MAX_VERSION_EVIDENCE_TEXT_BYTES: u64 = 8_388_608;

#[derive(Debug, Serialize)]
struct VersionMatrix {
    schema_version: u32,
    requested_vyre_release: &'static str,
    requested_weir_release: &'static str,
    tag_story: ReleaseTagStory,
    required_release_packages: Vec<String>,
    missing_required_release_packages: Vec<String>,
    crates: Vec<CrateVersion>,
    dependency_hints: Vec<DependencyVersionHint>,
    lockfile_packages: Vec<LockfilePackageVersion>,
    release_doc_tag_findings: Vec<ReleaseDocTagFinding>,
    release_note_token_findings: Vec<ReleaseNoteTokenFinding>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ReleaseTagStory {
    vyre_rc_tag: &'static str,
    weir_rc_tag: &'static str,
    combined_release_train_rc_tag: &'static str,
    vyre_tag: &'static str,
    weir_tag: &'static str,
    combined_release_train_tag: &'static str,
    policy: &'static str,
    required_in_release_notes: Vec<&'static str>,
    required_in_packaging: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct ReleaseTagPlan<'a> {
    schema_version: u32,
    vyre_rc_tag: &'a str,
    weir_rc_tag: &'a str,
    combined_release_train_rc_tag: &'a str,
    vyre_tag: &'a str,
    weir_tag: &'a str,
    combined_release_train_tag: &'a str,
    tag_creation_order: Vec<&'a str>,
    required_gate_before_rc_tag: &'a str,
    required_gate_before_tag: &'a str,
    version_matrix_blocker_count: usize,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CrateVersion {
    package: String,
    version: String,
    manifest: String,
    release_group: &'static str,
    publishable: bool,
}

#[derive(Debug, Serialize)]
struct DependencyVersionHint {
    manifest: String,
    dependency: String,
    version: String,
    expected: &'static str,
    release_group: &'static str,
}

#[derive(Debug, Serialize)]
struct LockfilePackageVersion {
    lockfile: String,
    package: String,
    version: String,
    expected: &'static str,
    release_group: &'static str,
}

#[derive(Debug, Serialize)]
struct ReleaseDocTagFinding {
    path: String,
    line: usize,
    text: String,
}

#[derive(Debug, Serialize)]
struct ReleaseNoteTokenFinding {
    path: String,
    missing: String,
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
    let mut crates = Vec::new();
    let mut collection_blockers = Vec::new();
    collect_workspace_versions(&vyre_root, "vyre", &mut crates, &mut collection_blockers);
    let santh_workspace_version = workspace_package_version(&santh_root, &mut collection_blockers);
    collect_one_version(
        &santh_root.join("libs/dataflow/weir/Cargo.toml"),
        "weir",
        santh_workspace_version.as_deref(),
        &mut crates,
        &mut collection_blockers,
    );
    collect_one_version(
        &santh_root.join("tools/vyrec/Cargo.toml"),
        "vyre",
        santh_workspace_version.as_deref(),
        &mut crates,
        &mut collection_blockers,
    );
    crates.sort_by(|left, right| left.package.cmp(&right.package));
    let missing_required_release_packages = missing_required_release_packages(&crates);
    let mut dependency_hints = Vec::new();
    collect_workspace_dependency_hints(&vyre_root, &mut dependency_hints, &mut collection_blockers);
    collect_one_dependency_hints(
        &santh_root.join("Cargo.toml"),
        &mut dependency_hints,
        &mut collection_blockers,
    );
    collect_one_dependency_hints(
        &santh_root.join("libs/dataflow/weir/Cargo.toml"),
        &mut dependency_hints,
        &mut collection_blockers,
    );
    collect_one_dependency_hints(
        &santh_root.join("tools/vyrec/Cargo.toml"),
        &mut dependency_hints,
        &mut collection_blockers,
    );
    dependency_hints.sort_by(|left, right| {
        left.manifest
            .cmp(&right.manifest)
            .then(left.dependency.cmp(&right.dependency))
    });
    let mut lockfile_packages = Vec::new();
    collect_lockfile_versions(
        &vyre_root.join("Cargo.lock"),
        &mut lockfile_packages,
        &mut collection_blockers,
    );
    collect_lockfile_versions(
        &santh_root.join("Cargo.lock"),
        &mut lockfile_packages,
        &mut collection_blockers,
    );
    lockfile_packages.sort_by(|left, right| {
        left.lockfile
            .cmp(&right.lockfile)
            .then(left.package.cmp(&right.package))
    });

    let mut blockers = Vec::new();
    blockers.extend(collection_blockers);
    for krate in &crates {
        if !krate.publishable && krate.package != "vyre-frontend-c" {
            continue;
        }
        match krate.release_group {
            "vyre" if krate.version != release_train::vyre_version() => blockers.push(format!(
                "{} is version {}, requested Vyre release is {}",
                krate.package,
                krate.version,
                release_train::vyre_version()
            )),
            "weir" if krate.version != release_train::weir_version() => blockers.push(format!(
                "{} is version {}, requested Weir release is {}",
                krate.package,
                krate.version,
                release_train::weir_version()
            )),
            _ => {}
        }
    }
    blockers.extend(
        missing_required_release_packages
            .iter()
            .map(|package| format!("missing required release package `{package}`")),
    );
    for hint in &dependency_hints {
        if hint.version != hint.expected {
            blockers.push(format!(
                "{} dependency `{}` is version {}, expected {} for {} release",
                hint.manifest, hint.dependency, hint.version, hint.expected, hint.release_group
            ));
        }
    }
    for package in &lockfile_packages {
        if package.version != package.expected {
            blockers.push(format!(
                "{} lock package `{}` is version {}, expected {} for {} release",
                package.lockfile,
                package.package,
                package.version,
                package.expected,
                package.release_group
            ));
        }
    }
    let (release_doc_tag_findings, doc_scan_blockers) =
        scan_bare_release_tags(&vyre_root, &santh_root);
    blockers.extend(doc_scan_blockers);
    for finding in &release_doc_tag_findings {
        blockers.push(format!(
            "{}:{} uses an ambiguous bare release tag command `{}`",
            finding.path, finding.line, finding.text
        ));
    }
    let release_note_token_findings = scan_release_note_tokens(&vyre_root, &santh_root);
    for finding in &release_note_token_findings {
        blockers.push(format!(
            "{} is missing release-note version token `{}`",
            finding.path, finding.missing
        ));
    }

    let matrix = VersionMatrix {
        schema_version: 1,
        requested_vyre_release: release_train::vyre_version(),
        requested_weir_release: release_train::weir_version(),
        tag_story: release_tag_story(),
        required_release_packages: release_train::required_release_packages()
            .into_iter()
            .map(|(package, version, _)| format!("{package}@{version}"))
            .collect(),
        missing_required_release_packages,
        crates,
        dependency_hints,
        lockfile_packages,
        release_doc_tag_findings,
        release_note_token_findings,
        blockers,
    };
    let json = match serde_json::to_string_pretty(&matrix) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize version matrix: {error}");
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
    write_release_tag_plan(&output, &matrix);
    println!("version-matrix: wrote {}", output.display());
    if !matrix.blockers.is_empty() {
        std::process::exit(1);
    }
}

fn missing_required_release_packages(crates: &[CrateVersion]) -> Vec<String> {
    release_train::required_release_packages()
        .into_iter()
        .filter_map(|(required_package, expected_version, expected_group)| {
            let present = crates.iter().any(|krate| {
                krate.package == required_package
                    && krate.version == expected_version
                    && krate.release_group == expected_group
            });
            (!present).then(|| format!("{required_package}@{expected_version}:{expected_group}"))
        })
        .collect()
}

fn write_release_tag_plan(output: &Path, matrix: &VersionMatrix) {
    let Some(parent) = output.parent() else {
        eprintln!(
            "Fix: version matrix output `{}` has no parent directory.",
            output.display()
        );
        std::process::exit(1);
    };
    let tag_story = &matrix.tag_story;
    let plan = ReleaseTagPlan {
        schema_version: 1,
        vyre_rc_tag: tag_story.vyre_rc_tag,
        weir_rc_tag: tag_story.weir_rc_tag,
        combined_release_train_rc_tag: tag_story.combined_release_train_rc_tag,
        vyre_tag: tag_story.vyre_tag,
        weir_tag: tag_story.weir_tag,
        combined_release_train_tag: tag_story.combined_release_train_tag,
        tag_creation_order: release_train::tag_creation_order().to_vec(),
        required_gate_before_rc_tag: "cargo_full run --bin xtask -- version-matrix --output release/evidence/version/version-matrix.json && cargo_full run --bin xtask -- release-completion-audit --output release/evidence/final/completion-audit.json && cargo_full run --bin xtask -- vyre-release-gate && scripts/apply-branch-protection.sh main",
        required_gate_before_tag: "cargo_full run --bin xtask -- version-matrix --output release/evidence/version/version-matrix.json && cargo_full run --bin xtask -- release-completion-audit --output release/evidence/final/completion-audit.json && cargo_full run --bin xtask -- vyre-release-gate && scripts/apply-branch-protection.sh main",
        version_matrix_blocker_count: matrix.blockers.len(),
        blockers: matrix.blockers.clone(),
    };
    let json = match serde_json::to_string_pretty(&plan) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize release tag plan: {error}");
            std::process::exit(1);
        }
    };
    let path = parent.join("release-tag-plan.json");
    if let Err(error) = fs::write(&path, format!("{json}\n")) {
        eprintln!("Fix: failed to write `{}`: {error}", path.display());
        std::process::exit(1);
    }
}

fn scan_bare_release_tags(
    vyre_root: &Path,
    santh_root: &Path,
) -> (Vec<ReleaseDocTagFinding>, Vec<String>) {
    let mut findings = Vec::new();
    let mut blockers = Vec::new();
    for path in release_doc_paths(vyre_root, santh_root) {
        let text = match read_text_bounded(&path) {
            Ok(text) => text,
            Err(error) => {
                blockers.push(format!(
                    "failed to read release doc `{}` for tag scan: {error}",
                    path.display()
                ));
                continue;
            }
        };
        for (line_index, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("git tag v0.4.2")
                || trimmed.starts_with("git push origin v0.4.2")
                || trimmed.starts_with("gh release create v0.4.2")
                || trimmed.starts_with("git tag v0.4.2-rc.1")
                || trimmed.starts_with("git push origin v0.4.2-rc.1")
                || trimmed.starts_with("gh release create v0.4.2-rc.1")
            {
                findings.push(ReleaseDocTagFinding {
                    path: path.display().to_string(),
                    line: line_index + 1,
                    text: trimmed.to_string(),
                });
            }
        }
    }
    (findings, blockers)
}

fn scan_release_note_tokens(vyre_root: &Path, santh_root: &Path) -> Vec<ReleaseNoteTokenFinding> {
    let mut findings = Vec::new();
    for path in [
        vyre_root.join("release/evidence/docs/release-notes.md"),
        vyre_root.join("release/evidence/docs/release-notes-version-story.md"),
        santh_root.join("docs/vyre-weir-release-plan.md"),
    ] {
        let text = match read_text_bounded(&path) {
            Ok(text) => text,
            Err(error) => {
                findings.push(ReleaseNoteTokenFinding {
                    path: path.display().to_string(),
                    missing: format!("required release-note document unreadable: {error}"),
                });
                continue;
            }
        };
        for required in release_tag_story().required_in_release_notes {
            if !text.contains(required) {
                findings.push(ReleaseNoteTokenFinding {
                    path: path.display().to_string(),
                    missing: required.to_string(),
                });
            }
        }
    }
    findings
}

fn release_doc_paths(vyre_root: &Path, santh_root: &Path) -> Vec<PathBuf> {
    vec![
        vyre_root.join("docs/RELEASE.md"),
        vyre_root.join("docs/RELEASE_ENGINEERING.md"),
        vyre_root.join("docs/RELEASE_CHECKLIST.md"),
        vyre_root.join("docs/release/v0.4.2.md"),
        vyre_root.join("README.md"),
        vyre_root.join("vyre-frontend-c/README.md"),
        vyre_root.join("release/evidence/docs/release-notes.md"),
        vyre_root.join("release/evidence/docs/release-notes-version-story.md"),
        santh_root.join("docs/vyre-weir-release-plan.md"),
        santh_root.join("libs/dataflow/weir/README.md"),
        santh_root.join("tools/vyrec/README.md"),
    ]
}

fn release_tag_story() -> ReleaseTagStory {
    ReleaseTagStory {
        vyre_rc_tag: release_train::vyre_rc_tag(),
        weir_rc_tag: release_train::weir_rc_tag(),
        combined_release_train_rc_tag: release_train::combined_release_train_rc_tag(),
        vyre_tag: release_train::vyre_tag(),
        weir_tag: release_train::weir_tag(),
        combined_release_train_tag: release_train::combined_release_train_tag(),
        policy: release_train::tag_policy(),
        required_in_release_notes: release_train::required_release_note_tokens(),
        required_in_packaging: release_train::required_packaging_steps(),
    }
}

fn collect_lockfile_versions(
    path: &Path,
    packages: &mut Vec<LockfilePackageVersion>,
    blockers: &mut Vec<String>,
) {
    let text = match read_text_bounded(path) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "failed to read lockfile `{}`: {error}",
                path.display()
            ));
            return;
        }
    };
    let value = match toml::from_str::<toml::Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            blockers.push(format!(
                "failed to parse lockfile `{}`: {error}",
                path.display()
            ));
            return;
        }
    };
    let Some(entries) = value.get("package").and_then(toml::Value::as_array) else {
        return;
    };
    for entry in entries {
        let Some(table) = entry.as_table() else {
            continue;
        };
        let Some(name) = table.get("name").and_then(toml::Value::as_str) else {
            continue;
        };
        let Some((expected, release_group)) = expected_dependency_version(name) else {
            continue;
        };
        let Some(version) = table.get("version").and_then(toml::Value::as_str) else {
            continue;
        };
        packages.push(LockfilePackageVersion {
            lockfile: path.display().to_string(),
            package: name.to_string(),
            version: version.to_string(),
            expected,
            release_group,
        });
    }
}

fn collect_workspace_dependency_hints(
    root: &Path,
    hints: &mut Vec<DependencyVersionHint>,
    blockers: &mut Vec<String>,
) {
    let root_manifest = root.join("Cargo.toml");
    collect_one_dependency_hints(&root_manifest, hints, blockers);
    let text = match read_text_bounded(&root_manifest) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "failed to read workspace manifest `{}` for dependency hints: {error}",
                root_manifest.display()
            ));
            return;
        }
    };
    let value = match toml::from_str::<toml::Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            blockers.push(format!(
                "failed to parse workspace manifest `{}` for dependency hints: {error}",
                root_manifest.display()
            ));
            return;
        }
    };
    let Some(members) = value
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
    else {
        return;
    };
    for member in members {
        let Some(member) = member.as_str() else {
            continue;
        };
        if member.contains('*') {
            continue;
        }
        collect_one_dependency_hints(&root.join(member).join("Cargo.toml"), hints, blockers);
    }
}

fn collect_one_dependency_hints(
    path: &Path,
    hints: &mut Vec<DependencyVersionHint>,
    blockers: &mut Vec<String>,
) {
    let text = match read_text_bounded(path) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "failed to read dependency manifest `{}`: {error}",
                path.display()
            ));
            return;
        }
    };
    let value = match toml::from_str::<toml::Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            blockers.push(format!(
                "failed to parse dependency manifest `{}`: {error}",
                path.display()
            ));
            return;
        }
    };
    collect_dependency_table(path, "dependencies", &value, hints);
    collect_dependency_table(path, "dev-dependencies", &value, hints);
    collect_dependency_table(path, "build-dependencies", &value, hints);
    if let Some(workspace) = value.get("workspace") {
        collect_dependency_table(path, "dependencies", workspace, hints);
    }
}

fn collect_dependency_table(
    manifest: &Path,
    table_name: &str,
    value: &toml::Value,
    hints: &mut Vec<DependencyVersionHint>,
) {
    let Some(table) = value.get(table_name).and_then(toml::Value::as_table) else {
        return;
    };
    for (dependency, spec) in table {
        let Some((expected, release_group)) = expected_dependency_version(dependency) else {
            continue;
        };
        let version = match spec {
            toml::Value::String(version) => Some(version.as_str()),
            toml::Value::Table(table) => table.get("version").and_then(toml::Value::as_str),
            _ => None,
        };
        let Some(version) = version else {
            continue;
        };
        hints.push(DependencyVersionHint {
            manifest: manifest.display().to_string(),
            dependency: dependency.clone(),
            version: version.to_string(),
            expected,
            release_group,
        });
    }
}

fn expected_dependency_version(dependency: &str) -> Option<(&'static str, &'static str)> {
    if dependency == "weir" {
        return Some((release_train::weir_version(), "weir"));
    }
    if matches!(
        dependency,
        "vyre-conform"
            | "vyre-conform-runner"
            | "vyre-test-harness"
            | "vyre-bench"
            | "vyre-bench-competitors"
            | "vyre-foundation-fuzz"
    ) {
        return None;
    }
    if dependency == "vyre" || dependency.starts_with("vyre-") {
        return Some((release_train::vyre_version(), "vyre"));
    }
    None
}

fn collect_workspace_versions(
    root: &Path,
    release_group: &'static str,
    versions: &mut Vec<CrateVersion>,
    blockers: &mut Vec<String>,
) {
    let workspace_version = workspace_package_version(root, blockers);
    collect_one_version(
        &root.join("Cargo.toml"),
        release_group,
        workspace_version.as_deref(),
        versions,
        blockers,
    );
    let root_manifest = root.join("Cargo.toml");
    let text = match read_text_bounded(&root_manifest) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "failed to read workspace manifest `{}` for version collection: {error}",
                root_manifest.display()
            ));
            return;
        }
    };
    let value = match toml::from_str::<toml::Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            blockers.push(format!(
                "failed to parse workspace manifest `{}` for version collection: {error}",
                root_manifest.display()
            ));
            return;
        }
    };
    let Some(members) = value
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
    else {
        return;
    };
    for member in members {
        let Some(member) = member.as_str() else {
            continue;
        };
        if member.contains('*') {
            continue;
        }
        collect_one_version(
            &root.join(member).join("Cargo.toml"),
            release_group,
            workspace_version.as_deref(),
            versions,
            blockers,
        );
    }
}

fn workspace_package_version(root: &Path, blockers: &mut Vec<String>) -> Option<String> {
    let manifest = root.join("Cargo.toml");
    let text = match read_text_bounded(&manifest) {
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
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_string)
}

fn collect_one_version(
    path: &Path,
    release_group: &'static str,
    workspace_version: Option<&str>,
    versions: &mut Vec<CrateVersion>,
    blockers: &mut Vec<String>,
) {
    let text = match read_text_bounded(path) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "failed to read package manifest `{}`: {error}",
                path.display()
            ));
            return;
        }
    };
    let value = match toml::from_str::<toml::Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            blockers.push(format!(
                "failed to parse package manifest `{}`: {error}",
                path.display()
            ));
            return;
        }
    };
    let Some(package) = value.get("package").and_then(toml::Value::as_table) else {
        return;
    };
    let Some(name) = package.get("name").and_then(toml::Value::as_str) else {
        return;
    };
    let version = package
        .get("version")
        .and_then(toml::Value::as_str)
        .or_else(|| {
            package
                .get("version")
                .and_then(|value| value.get("workspace"))
                .and_then(toml::Value::as_bool)
                .filter(|workspace| *workspace)
                .and_then(|_| workspace_version)
        });
    let Some(version) = version else {
        return;
    };
    let publishable = !matches!(package.get("publish"), Some(toml::Value::Boolean(false)))
        && !matches!(
            name,
            "vyre-conform" | "vyre-conform-runner" | "vyre-test-harness"
        );
    versions.push(CrateVersion {
        package: name.to_string(),
        version: version.to_string(),
        manifest: path.display().to_string(),
        release_group,
        publishable,
    });
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
                    "USAGE:\n  cargo_full run --bin xtask -- version-matrix [--output PATH]\n\n\
                     Writes release version evidence for the Vyre/Weir release story."
                );
                std::process::exit(0);
            }
            other => return Err(format!("Fix: unknown version-matrix option `{other}`.")),
        }
    }
    Ok(output.unwrap_or_else(default_output))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/version/version-matrix.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/version/version-matrix.json"))
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_VERSION_EVIDENCE_TEXT_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_VERSION_EVIDENCE_TEXT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_VERSION_EVIDENCE_TEXT_BYTES} byte version evidence read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}
