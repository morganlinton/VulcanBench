//! Test: rewrite layer contract.
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("vyre-lower must live under workspace root")
        .to_path_buf()
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    fn visit(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    let mut out = Vec::new();
    visit(root, &mut out);
    out
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| {
        panic!("failed to read {}: {error}", path.display());
    })
}

#[derive(Debug, Deserialize)]
struct KernelFamilySurfaceManifest {
    schema_version: u32,
    contract: String,
    family: Vec<KernelFamilySurface>,
}

#[derive(Debug, Deserialize)]
struct KernelFamilySurface {
    family_id: String,
    owner_lane: String,
    root: String,
    public_reexport: String,
    schedule_config: String,
    evidence_writer: String,
    forbid_section_dividers: bool,
    forbidden_private_import_prefixes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct KernelFamilySchedule {
    schema_version: u32,
    contract: String,
    family_id: String,
    stage_order: Vec<String>,
    evidence_policy: String,
}

#[test]
fn foundation_program_optimizer_does_not_depend_on_lowered_descriptors() {
    let root = workspace_root().join("vyre-foundation/src/optimizer");
    let forbidden = [
        "KernelDescriptor",
        "KernelOp",
        "KernelBody",
        "vyre_lower::",
        "descriptor_const_fold",
        "descriptor_cse",
        "descriptor_dce",
    ];

    let mut offenders = Vec::new();
    for file in rust_files(&root) {
        let text = read(&file);
        for needle in forbidden {
            if text.contains(needle) {
                offenders.push(format!("{} contains {needle}", file.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "Program-IR optimizer must stay semantic and must not reach into lowered descriptor cleanup:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn descriptor_rewrite_cleanup_names_are_layer_prefixed() {
    let mod_rs = workspace_root().join("vyre-lower/src/rewrites/mod.rs");
    let text = read(&mod_rs);

    for name in ["const_fold", "cse", "dce"] {
        assert!(
            !text.contains(&format!("pub mod {name};")),
            "lowered descriptor cleanup must not expose an unprefixed `{name}` module"
        );
        assert!(
            !text.contains(&format!("pub use {name}::{name};")),
            "lowered descriptor cleanup must not re-export an unprefixed `{name}` function"
        );
    }

    for name in ["descriptor_const_fold", "descriptor_cse", "descriptor_dce"] {
        assert!(
            text.contains(&format!("pub mod {name};")),
            "missing descriptor-prefixed rewrite module `{name}`"
        );
        assert!(
            text.contains(&format!("pub use {name}::{name};")),
            "missing descriptor-prefixed rewrite re-export `{name}`"
        );
    }
}

#[test]
fn emit_and_driver_crates_do_not_host_program_optimizer_passes() {
    let root = workspace_root();
    let checked_roots = [
        root.join("vyre-emit-naga/src"),
        root.join("vyre-emit-ptx/src"),
        root.join("vyre-driver-wgpu/src"),
        root.join("vyre-driver-cuda/src"),
    ];
    let forbidden = [
        "ProgramPass",
        "PassScheduler",
        "pre_lowering::optimize",
        "optimizer::passes::const_fold",
        "optimizer::passes::fusion_cse",
        "fn fold_expr",
        "fold_binary_literal",
        "fold_unary_literal",
        "fold_cast_literal",
    ];

    let mut offenders = Vec::new();
    for checked_root in checked_roots {
        for file in rust_files(&checked_root) {
            let text = read(&file);
            for needle in forbidden {
                if text.contains(needle) {
                    offenders.push(format!("{} contains {needle}", file.display()));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "Emit/backend crates must not host Program-IR optimizer passes or duplicate Layer-1 constant folding:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn kernel_family_surfaces_have_single_reexport_schedule_and_evidence_contracts() {
    let root = workspace_root();
    let manifest_path = root.join("vyre-lower/rules/kernel_family_surfaces.toml");
    let manifest: KernelFamilySurfaceManifest = toml::from_str(&read(&manifest_path))
        .expect("Fix: kernel family surface manifest must be valid TOML.");

    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.contract, "vyre-kernel-family-surfaces:v1");
    assert!(
        !manifest.family.is_empty(),
        "Fix: kernel family surface manifest must declare at least one family."
    );

    let mut public_reexports = std::collections::BTreeSet::new();
    let mut failures = Vec::new();
    for family in &manifest.family {
        if family.family_id.trim().is_empty() {
            failures.push("family_id is blank".to_string());
        }
        if family.owner_lane.trim().is_empty() {
            failures.push(format!("{} owner_lane is blank", family.family_id));
        }
        for (label, rel_path) in [
            ("root", &family.root),
            ("public_reexport", &family.public_reexport),
            ("schedule_config", &family.schedule_config),
            ("evidence_writer", &family.evidence_writer),
        ] {
            if !root.join(rel_path).exists() {
                failures.push(format!(
                    "{} {label} `{rel_path}` does not exist",
                    family.family_id
                ));
            }
        }
        if !public_reexports.insert(family.public_reexport.as_str()) {
            failures.push(format!(
                "duplicate public re-export point `{}`",
                family.public_reexport
            ));
        }
        if let Ok(schedule_text) = fs::read_to_string(root.join(&family.schedule_config)) {
            let schedule: KernelFamilySchedule = toml::from_str(&schedule_text).unwrap_or_else(
                |error| {
                    panic!(
                        "Fix: schedule config `{}` must be valid TOML: {error}",
                        family.schedule_config
                    )
                },
            );
            if schedule.schema_version != 1 {
                failures.push(format!(
                    "{} schedule schema_version must be 1",
                    family.family_id
                ));
            }
            if schedule.contract != "vyre-kernel-family-schedule:v1" {
                failures.push(format!(
                    "{} schedule contract must be vyre-kernel-family-schedule:v1",
                    family.family_id
                ));
            }
            if schedule.family_id != family.family_id {
                failures.push(format!(
                    "{} schedule family_id `{}` does not match manifest",
                    family.family_id, schedule.family_id
                ));
            }
            if schedule.stage_order.len() < 3 {
                failures.push(format!(
                    "{} schedule must declare at least three stages",
                    family.family_id
                ));
            }
            if schedule.evidence_policy.trim().is_empty() {
                failures.push(format!(
                    "{} schedule evidence_policy is blank",
                    family.family_id
                ));
            }
        }
        for file in rust_files(&root.join(&family.root)) {
            let text = read(&file);
            if family.forbid_section_dividers && has_section_divider(&text) {
                failures.push(format!(
                    "{} contains section-divider monolith marker in {}",
                    family.family_id,
                    file.display()
                ));
            }
            for prefix in &family.forbidden_private_import_prefixes {
                if !prefix.trim().is_empty() && text.contains(prefix) {
                    failures.push(format!(
                        "{} imports forbidden private family prefix `{}` in {}",
                        family.family_id,
                        prefix,
                        file.display()
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "Kernel family organization contract failed:\n{}",
        failures.join("\n")
    );
}

fn has_section_divider(text: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("// ===")
            || trimmed.starts_with("// ---")
            || trimmed.starts_with("// SECTION")
            || trimmed.starts_with("// region:")
    })
}
