//! Oversized test module contract.
//!
//! Test files across the workspace should remain under 800 lines. Existing
//! oversized tests are baselined; new ones must be split or explicitly exempt.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[test]
fn workspace_oversized_test_modules_are_baselined() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    let members_with_tests = [
        "vyre-core",
        "vyre-foundation",
        "vyre-driver",
        "vyre-reference",
        "vyre-spec",
        "vyre-macros",
        "vyre-primitives",
        "vyre-runtime",
        "vyre-libs",
        "vyre-intrinsics",
        "vyre-frontend-c",
        "conform/vyre-conform-spec",
        "conform/vyre-conform-generate",
        "conform/vyre-conform-enforce",
        "conform/vyre-conform-runner",
        "conform/vyre-test-harness",
    ];

    // Baseline of grandfathered oversized (>800-line) test modules. Every prior
    // entry (r2_corpus_measurement, gpu_prep_kernel_libc_shape, ast_oracle,
    // release_gate_contracts, cert_artifact, adversarial_graph_csr_validation_
    // contracts) has since been split below the threshold, so the baseline is
    // now EMPTY: any tests/ module that grows past 800 lines must be split or
    // explicitly re-added here with justification.
    let known: HashMap<String, usize> = HashMap::new();

    let known_set: HashSet<String> = known.keys().cloned().collect();

    let mut found: HashMap<String, usize> = HashMap::new();
    for member in &members_with_tests {
        let tests_dir = workspace_root.join(member).join("tests");
        if !tests_dir.is_dir() {
            continue;
        }
        let mut stack = vec![tests_dir];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    let content = std::fs::read_to_string(&path).unwrap();
                    let lines = content.lines().count();
                    if lines > 800 {
                        let rel = path
                            .strip_prefix(workspace_root)
                            .unwrap()
                            .display()
                            .to_string();
                        found.insert(rel, lines);
                    }
                }
            }
        }
    }

    let mut new_violations = Vec::new();
    for (path, lines) in &found {
        if !known_set.contains(path) {
            new_violations.push(format!("{} ({} lines)", path, lines));
        }
    }

    let mut missing = Vec::new();
    for k in &known_set {
        if !found.contains_key(k) {
            missing.push(k.clone());
        }
    }

    assert!(
        new_violations.is_empty() && missing.is_empty(),
        "oversized test module contract violation.\n\
         New oversized files:\n{}\n\
         Missing known files (renamed/removed):\n{}\n\
         If a new file is legitimately large, add it to the known list; otherwise, split it.",
        new_violations.join("\n"),
        missing.join("\n")
    );
}
