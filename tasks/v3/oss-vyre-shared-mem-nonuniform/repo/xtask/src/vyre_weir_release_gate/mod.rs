//! Evidence gate for the Vyre release objective.
//!
//! This gate intentionally checks artifacts, not intent. The release is
//! blocked until every requirement in `release/vyre-release-evidence.toml` is
//! closed and backed by concrete files.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

mod checks;
mod paths;
mod semantic;
mod types;

use checks::check_markdown_evidence_path_ready;
use paths::{manifest_path_from_args, read_text_bounded, resolve_manifest_path};
use semantic::run_semantic_requirement_checks;
use types::EvidenceManifest;

pub(crate) fn run(args: &[String]) {
    let manifest_path = match manifest_path_from_args(args) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    let manifest_text = match read_text_bounded(&manifest_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "Fix: failed to read release evidence manifest `{}`: {error}",
                manifest_path.display()
            );
            std::process::exit(1);
        }
    };

    let manifest: EvidenceManifest = match toml::from_str(&manifest_text) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!(
                "Fix: release evidence manifest `{}` is invalid TOML: {error}",
                manifest_path.display()
            );
            std::process::exit(1);
        }
    };

    let base_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut failures = Vec::new();

    if manifest.schema_version != 1 {
        failures.push(format!(
            "schema_version must be 1, found {}",
            manifest.schema_version
        ));
    }
    if manifest.release.vyre.trim().is_empty() {
        failures.push("release.vyre is empty".to_string());
    }
    if manifest.release.weir.trim().is_empty() {
        failures.push("release.weir is empty".to_string());
    }

    let plan_path = resolve_manifest_path(&base_dir, &manifest.plan_path);
    if !plan_path.is_file() {
        failures.push(format!(
            "plan_path `{}` does not resolve to a file",
            plan_path.display()
        ));
    } else {
        match read_text_bounded(&plan_path) {
            Ok(plan) => {
                for phrase in [
                    "thousands of concrete",
                    "At least ten",
                    "CUDA-first",
                    "Linux subsystem corpus",
                    "Completion audit checklist",
                ] {
                    if !plan.contains(phrase) {
                        failures.push(format!(
                            "plan `{}` is missing required release phrase `{phrase}`",
                            plan_path.display()
                        ));
                    }
                }
            }
            Err(error) => failures.push(format!(
                "plan `{}` could not be read: {error}",
                plan_path.display()
            )),
        }
    }

    let mut ids = BTreeSet::new();
    for requirement in &manifest.requirements {
        if !ids.insert(requirement.id.as_str()) {
            failures.push(format!("duplicate requirement id `{}`", requirement.id));
        }
        if requirement.title.trim().is_empty() {
            failures.push(format!(
                "requirement `{}` has an empty title",
                requirement.id
            ));
        }
        if requirement.status != "closed" {
            failures.push(format!(
                "requirement `{}` is `{}`; release requires `closed`",
                requirement.id, requirement.status
            ));
        }
        if requirement.status == "closed" {
            if requirement.evidence.len() < requirement.minimum_evidence {
                failures.push(format!(
                    "requirement `{}` has {} evidence item(s), needs at least {}",
                    requirement.id,
                    requirement.evidence.len(),
                    requirement.minimum_evidence
                ));
            }
            for evidence in &requirement.evidence {
                if is_manifest_command_evidence(evidence) {
                    continue;
                }
                let evidence_path = resolve_manifest_path(&base_dir, evidence);
                match fs::metadata(&evidence_path) {
                    Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                        if evidence.ends_with(".md") {
                            check_markdown_evidence_path_ready(
                                requirement,
                                &evidence_path,
                                evidence,
                                &mut failures,
                            );
                        }
                    }
                    Ok(metadata) if metadata.is_file() => failures.push(format!(
                        "requirement `{}` evidence path `{}` is empty",
                        requirement.id,
                        evidence_path.display()
                    )),
                    Ok(_) => failures.push(format!(
                        "requirement `{}` evidence path `{}` exists but is not a file",
                        requirement.id,
                        evidence_path.display()
                    )),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        failures.push(format!(
                            "requirement `{}` evidence path `{}` does not exist",
                            requirement.id,
                            evidence_path.display()
                        ));
                    }
                    Err(error) => failures.push(format!(
                        "requirement `{}` evidence path `{}` is unreadable: {error}",
                        requirement.id,
                        evidence_path.display()
                    )),
                }
            }
            run_semantic_requirement_checks(requirement, &base_dir, &mut failures);
        }
    }

    const REQUIRED_IDS: &[&str] = &[
        "version-story",
        "cuda-first-path",
        "wgpu-fallback",
        "megakernel-default",
        "optimization-corpus-4096",
        "optimization-benchmark-proof",
        "weir-analysis-integration",
        "alias-aware-upgrades",
        "egraph-saturation",
        "proof-workloads-12",
        "cpu-only-100x-proof",
        "c-parser-linux-subsystem",
        "distributed-parser-coherence",
        "conformance-hard-gate",
        "modular-test-architecture",
        "exhaustive-verification",
        "docs-evidence-linked",
        "crate-metadata",
        "release-hygiene",
        "final-completion-audit",
    ];

    for required in REQUIRED_IDS {
        if !ids.contains(required) {
            failures.push(format!("manifest is missing required id `{required}`"));
        }
    }

    if failures.is_empty() {
        println!(
            "vyre-release-gate: {} requirement(s) closed for Vyre {} / Weir {}",
            manifest.requirements.len(),
            manifest.release.vyre,
            manifest.release.weir
        );
    } else {
        eprintln!("vyre-release-gate: {} release blocker(s):", failures.len());
        for failure in &failures {
            eprintln!("  - {failure}");
        }
        eprintln!("Fix: attach real evidence artifacts and close every manifest requirement.");
        std::process::exit(1);
    }
}

fn is_manifest_command_evidence(evidence: &str) -> bool {
    evidence.starts_with("cargo_full ")
}
