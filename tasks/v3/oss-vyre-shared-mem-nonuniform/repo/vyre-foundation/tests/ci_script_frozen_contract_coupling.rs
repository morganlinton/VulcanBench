//! CI / script / frozen-contract coupling contract.
//!
//! Workflows must reference scripts that exist. Frozen-trait snapshot scripts
//! must reference source files and snapshots that exist. Stale couplings must
//! be baselined or removed.

use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn ci_workflows_reference_existing_scripts() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();
    let workflows_dir = workspace_root.join(".github/workflows");
    if !workflows_dir.is_dir() {
        return;
    }

    let scripts_dir = workspace_root.join("scripts");

    // Known script references that use wildcards or are not literal filenames.
    let known_wildcards: HashSet<String> =
        ["scripts/check_*.sh".to_string()].iter().cloned().collect();

    let mut violations = Vec::new();

    for entry in std::fs::read_dir(&workflows_dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("yml") {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap();
        for (line_no, line) in content.lines().enumerate() {
            if line.contains("scripts/") && line.contains(".sh") {
                let trimmed = line.trim();
                // Extract the script name after scripts/
                if let Some(idx) = trimmed.find("scripts/") {
                    let rest = &trimmed[idx + 8..];
                    let script_name = rest.split_whitespace().next().unwrap_or(rest);
                    let script_name = script_name.trim_end_matches('"').trim_end_matches('\'');
                    if script_name.contains('*') {
                        if !known_wildcards.contains(&format!("scripts/{}", script_name)) {
                            violations.push(format!(
                                "{}:{} unknown wildcard script reference: scripts/{}",
                                path.file_name().unwrap().to_string_lossy(),
                                line_no + 1,
                                script_name
                            ));
                        }
                        continue;
                    }
                    let script_path = scripts_dir.join(script_name);
                    if !script_path.exists() {
                        violations.push(format!(
                            "{}:{} missing script: scripts/{}",
                            path.file_name().unwrap().to_string_lossy(),
                            line_no + 1,
                            script_name
                        ));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "CI workflows must reference existing scripts. Violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn frozen_trait_contract_files_exist() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    let contracts = [
        ("VyreBackend", "vyre-driver/src/backend/vyre_backend.rs"),
        ("ExprVisitor", "vyre-foundation/src/visit/expr.rs"),
        ("Lowerable", "vyre-driver/src/backend/lowering.rs"),
        ("AlgebraicLaw", "vyre-spec/src/algebraic_law.rs"),
        ("EnforceGate", "vyre-driver/src/registry/enforce.rs"),
        ("MutationClass", "vyre-driver/src/registry/mutation.rs"),
    ];

    let mut violations = Vec::new();
    for (name, file) in &contracts {
        let path = workspace_root.join(file);
        if !path.exists() {
            violations.push(format!(
                "frozen contract source missing: {} ({})",
                name, file
            ));
            continue;
        }
        let snapshot = workspace_root.join(format!("docs/frozen-traits/{}.txt", name));
        if !snapshot.exists() {
            violations.push(format!(
                "frozen contract snapshot missing: {} ({}). Fix: run scripts/check_trait_freeze.sh --refresh-snapshots",
                name, snapshot.display()
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "frozen trait contracts must have source files and snapshots. Violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn frozen_trait_script_is_executable() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();
    let script = workspace_root.join("scripts/check_trait_freeze.sh");
    assert!(
        script.exists(),
        "scripts/check_trait_freeze.sh must exist to enforce frozen contracts"
    );
}
