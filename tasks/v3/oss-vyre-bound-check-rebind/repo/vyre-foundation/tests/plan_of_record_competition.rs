//! Competing plan-of-record document contract.
//!
//! Only documents that explicitly disclaim authority may use POR language
//! outside the single active planning source. New undisputed POR claims are
//! forbidden.

use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn no_undisputed_plan_of_record_documents() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    let scan_dirs = [
        workspace_root.join("docs"),
        workspace_root.join("audits"),
        workspace_root.join(".internals/planning"),
        workspace_root.join(".internals/plans"),
        workspace_root.join(".internals/archive"),
    ];

    let known_with_disclaimer: HashSet<String> = [
        "docs/OP_MASTER_PLAN_BUILDING_BLOCKS_AND_QA.md",
        ".internals/audits/from-docs-audits/MASTER_PLAN_RELEASE.md",
        ".internals/audits/from-docs-audits/MASTER_PLAN.md",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut violations = Vec::new();

    for dir in &scan_dirs {
        if !dir.is_dir() {
            continue;
        }
        let mut stack = vec![dir.clone()];
        while let Some(current) = stack.pop() {
            for entry in std::fs::read_dir(&current).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
                    let content = std::fs::read_to_string(&path).unwrap();
                    let lower = content.to_lowercase();
                    if lower.contains("plan of record") || lower.contains("plan-of-record") {
                        let rel = path
                            .strip_prefix(workspace_root)
                            .unwrap()
                            .display()
                            .to_string();
                        let has_disclaimer = lower.contains("not the current plan of record")
                            || lower.contains("not the release plan of record");
                        if !has_disclaimer && !known_with_disclaimer.contains(&rel) {
                            violations.push(rel);
                        }
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "documents claiming plan-of-record authority must disclaim it or be baselined. Violations:\n{}",
        violations.join("\n")
    );
}
