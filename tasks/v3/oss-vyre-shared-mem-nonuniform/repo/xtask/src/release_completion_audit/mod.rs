//! Prompt-to-artifact completion audit for the Vyre/Weir release.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct EvidenceManifest {
    plan_path: String,
    requirements: Vec<Requirement>,
}

#[derive(Debug, Deserialize)]
struct Requirement {
    id: String,
    title: String,
    status: String,
    evidence: Vec<String>,
    minimum_evidence: usize,
}

#[derive(Debug, Serialize)]
struct CompletionAudit {
    schema_version: u32,
    objective: &'static str,
    success_criteria: Vec<&'static str>,
    prompt_to_artifact_checklist: Vec<ChecklistItem>,
    plan_path: String,
    total_requirements: usize,
    closed_requirements: usize,
    blocked_or_open_requirements: usize,
    requirements: Vec<RequirementAudit>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChecklistItem {
    requirement_id: &'static str,
    explicit_requirement: &'static str,
    required_artifacts_or_commands: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct RequirementAudit {
    id: String,
    title: String,
    status: String,
    minimum_evidence: usize,
    evidence_count: usize,
    existing_evidence_count: usize,
    missing_evidence: Vec<String>,
    semantic_blockers: Vec<String>,
    complete: bool,
}

pub(crate) const MAX_RELEASE_AUDIT_TEXT_BYTES: u64 = 16_777_216;

mod config;
mod paths;
mod semantics;

use config::{parse_args, release_checklist};
use paths::{
    is_checklist_artifact, is_manifest_command_evidence, paths_equal, read_text_bounded,
    resolve_checklist_artifact_path, resolve_manifest_path,
};
use semantics::inspect_evidence_semantics;

pub(crate) fn run(args: &[String]) {
    let config = match parse_args(args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let manifest_text = match read_text_bounded(&config.manifest) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "Fix: failed to read release manifest `{}`: {error}",
                config.manifest.display()
            );
            std::process::exit(1);
        }
    };
    let manifest = match toml::from_str::<EvidenceManifest>(&manifest_text) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!(
                "Fix: release manifest `{}` is invalid TOML: {error}",
                config.manifest.display()
            );
            std::process::exit(1);
        }
    };
    let base_dir = config
        .manifest
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut blockers = Vec::new();
    let mut audits = Vec::new();
    for requirement in manifest.requirements {
        let mut missing = Vec::new();
        let mut semantic_blockers = Vec::new();
        let mut existing = 0usize;
        for evidence in &requirement.evidence {
            if is_manifest_command_evidence(evidence) {
                existing += 1;
                continue;
            }
            let path = resolve_manifest_path(&base_dir, evidence);
            let is_self_output = paths_equal(&path, &config.output);
            if is_self_output {
                existing += 1;
                continue;
            }
            match fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                    existing += 1;
                    inspect_evidence_semantics(evidence, &path, &mut semantic_blockers);
                }
                Ok(metadata) if metadata.is_file() => {
                    existing += 1;
                    semantic_blockers.push(format!("{} is empty", path.display()));
                }
                Ok(_) => {
                    semantic_blockers.push(format!("{} exists but is not a file", path.display()));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    missing.push(evidence.clone());
                }
                Err(error) => {
                    semantic_blockers.push(format!("failed to stat {}: {error}", path.display()));
                }
            }
        }
        let complete = requirement.status == "closed"
            && requirement.evidence.len() >= requirement.minimum_evidence
            && missing.is_empty()
            && semantic_blockers.is_empty();
        if !complete {
            blockers.push(format!(
                "{} is status `{}` with {}/{} evidence files present and {} semantic blocker(s)",
                requirement.id,
                requirement.status,
                existing,
                requirement.evidence.len(),
                semantic_blockers.len()
            ));
        }
        audits.push(RequirementAudit {
            id: requirement.id,
            title: requirement.title,
            status: requirement.status,
            minimum_evidence: requirement.minimum_evidence,
            evidence_count: requirement.evidence.len(),
            existing_evidence_count: existing,
            missing_evidence: missing,
            semantic_blockers,
            complete,
        });
    }
    let checklist = release_checklist();
    let manifest_ids = audits
        .iter()
        .map(|audit| audit.id.as_str())
        .collect::<BTreeSet<_>>();
    let checklist_ids = checklist
        .iter()
        .map(|item| item.requirement_id)
        .collect::<BTreeSet<_>>();
    for missing in manifest_ids.difference(&checklist_ids) {
        blockers.push(format!(
            "final audit checklist is missing manifest requirement `{missing}`"
        ));
    }
    for extra in checklist_ids.difference(&manifest_ids) {
        blockers.push(format!(
            "final audit checklist references non-manifest requirement `{extra}`"
        ));
    }
    for item in &checklist {
        for artifact in item
            .required_artifacts_or_commands
            .iter()
            .copied()
            .filter(|entry| is_checklist_artifact(entry))
        {
            let path = resolve_checklist_artifact_path(&base_dir, artifact);
            let is_self_output = paths_equal(&path, &config.output);
            if is_self_output {
                continue;
            }
            match fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                    inspect_evidence_semantics(artifact, &path, &mut blockers);
                }
                Ok(metadata) if metadata.is_file() => {
                    blockers.push(format!(
                        "final audit checklist artifact `{artifact}` for `{}` is empty at {}",
                        item.requirement_id,
                        path.display()
                    ));
                }
                Ok(_) => {
                    blockers.push(format!(
                        "final audit checklist artifact `{artifact}` for `{}` exists but is not a file at {}",
                        item.requirement_id,
                        path.display()
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    blockers.push(format!(
                        "final audit checklist artifact `{artifact}` for `{}` is missing at {}",
                        item.requirement_id,
                        path.display()
                    ));
                }
                Err(error) => {
                    blockers.push(format!(
                        "final audit checklist artifact `{artifact}` for `{}` is unreadable at {}: {error}",
                        item.requirement_id,
                        path.display()
                    ));
                }
            }
        }
    }
    let closed_requirements = audits.iter().filter(|audit| audit.complete).count();
    let audit = CompletionAudit {
        schema_version: 1,
        objective: "Make Vyre 0.6.3 and Weir 0.1.0 release-ready end to end",
        success_criteria: vec![
            "1. CUDA-first execution path is documented, benchmarked, and selected as the fast release substrate.",
            "2. WGPU fallback is functional, tested, benchmarked, and never hides a CUDA or CPU downgrade.",
            "3. Megakernel is the default high-throughput runtime path where legal, including paired speculation evidence.",
            "4. Non-megakernel dispatch is retained only with measured or architectural justification.",
            "5. Optimization infrastructure proves at least 4096 concrete rewrite/pass opportunities across families.",
            "6. Optimization passes have correctness tests and before/after benchmark evidence.",
            "7. Weir alias, reaching-def, points-to, callgraph, slicing, summary, loop, and fixpoint facts integrate into Vyre optimization.",
            "8. The distributed C parser parses a full selected Linux subsystem corpus with AST/semantic contract evidence.",
            "9. At least 12 proof workload families compare CUDA against serious CPU baselines with reproducible artifacts; the current matrix carries 13 required rows.",
            "10. At least ten formerly CPU-only workload families prove 100x+ wins or block release.",
            "11. Conformance gates block release for every claimed op/backend path with zero blocked_release OP_MATRIX rows.",
            "12. Test organization is modular, including distributed parser CLI evidence for tools/vyrec.",
            "13. Documentation is coherent and every release claim links to concrete evidence.",
            "14. Crate metadata, feature surfaces, readmes, examples, license, and version policy are consistent for Vyre 0.6.3 / Weir 0.1.0.",
            "15. Final hygiene review finds no unbounded caches, hidden fallbacks, placeholders, library panics, unactionable errors, or undocumented public API.",
        ],
        prompt_to_artifact_checklist: checklist,
        plan_path: manifest.plan_path,
        total_requirements: audits.len(),
        closed_requirements,
        blocked_or_open_requirements: audits.len().saturating_sub(closed_requirements),
        requirements: audits,
        blockers,
    };
    let json = match serde_json::to_string_pretty(&audit) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize completion audit: {error}");
            std::process::exit(1);
        }
    };
    if let Some(parent) = config.output.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("Fix: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    if let Err(error) = fs::write(&config.output, format!("{json}\n")) {
        eprintln!(
            "Fix: failed to write `{}`: {error}",
            config.output.display()
        );
        std::process::exit(1);
    }
    println!(
        "release-completion-audit: wrote {}",
        config.output.display()
    );
    if !audit.blockers.is_empty() {
        std::process::exit(1);
    }
}
