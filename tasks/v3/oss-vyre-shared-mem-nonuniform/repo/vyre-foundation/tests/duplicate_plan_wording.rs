//! Organization contract: plan-of-record wording must not be duplicated across
//! living documents. Duplicate text creates drift risk and violates lego-block
//! discipline: each document should have a single, clear responsibility.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Scans docs/, audits/, and .internals/ for sequences of 6 consecutive
/// non-empty, non-trivial lines that appear in two or more distinct files.
/// Generated mirrors and archives are excluded.
#[test]
fn no_duplicate_plan_of_record_wording() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    let dirs = [
        workspace_root.join("docs"),
        workspace_root.join("audits"),
        workspace_root.join(".internals"),
    ];

    let exclude_prefixes: Vec<PathBuf> = [
        ".internals/catalogs/from-docs-catalogs",
        ".internals/audits/from-docs-audits",
        "docs/generated",
        ".internals/archive",
    ]
    .iter()
    .map(|s| workspace_root.join(s))
    .collect();

    // Map from sequence -> list of file paths where it appears.
    let mut sequences: HashMap<Vec<String>, Vec<PathBuf>> = HashMap::new();

    for base in &dirs {
        if !base.is_dir() {
            continue;
        }
        let mut stack = vec![base.clone()];
        while let Some(dir) = stack.pop() {
            if exclude_prefixes.iter().any(|ep| dir.starts_with(ep)) {
                continue;
            }
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
                    if is_git_ignored(workspace_root, &path) {
                        continue;
                    }
                    let content = std::fs::read_to_string(&path).unwrap();
                    let lines: Vec<String> =
                        content.lines().map(|l| l.trim_end().to_string()).collect();
                    for window in lines.windows(6) {
                        if window.iter().any(|l| l.trim().is_empty()) {
                            continue;
                        }
                        if window.iter().all(|l| l.starts_with('#')) {
                            continue; // all headers
                        }
                        if window
                            .iter()
                            .all(|l| l.trim() == "---" || l.trim().is_empty())
                        {
                            continue; // structural
                        }
                        sequences
                            .entry(window.to_vec())
                            .or_default()
                            .push(path.clone());
                    }
                }
            }
        }
    }

    // Aggregate duplicated sequences by unique file pairs.
    let mut pair_counts: HashMap<(String, String), usize> = HashMap::new();

    for paths in sequences.values() {
        let mut uniq: Vec<String> = paths
            .iter()
            .map(|p| {
                p.strip_prefix(workspace_root)
                    .unwrap_or(p)
                    .display()
                    .to_string()
            })
            .collect();
        uniq.sort();
        uniq.dedup();
        if uniq.len() >= 2 {
            for i in 0..uniq.len() {
                for j in (i + 1)..uniq.len() {
                    *pair_counts
                        .entry((uniq[i].clone(), uniq[j].clone()))
                        .or_insert(0) += 1;
                }
            }
        }
    }

    if pair_counts.is_empty() {
        return;
    }

    let mut report: Vec<String> = pair_counts
        .into_iter()
        .map(|((a, b), count)| format!("{} duplicated sequences between:\n  {}\n  {}", count, a, b))
        .collect();
    report.sort();

    panic!(
        "plan-of-record wording is duplicated across {} file pair(s). \
         Each document must have a single source of truth.\
         Fix: extract shared content into a single canonical document and link to it.\n\n{}",
        report.len(),
        report.join("\n\n")
    );
}

fn is_git_ignored(workspace_root: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(workspace_root) else {
        return false;
    };
    Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .arg("check-ignore")
        .arg("-q")
        .arg(rel)
        .status()
        .is_ok_and(|status| status.success())
}
