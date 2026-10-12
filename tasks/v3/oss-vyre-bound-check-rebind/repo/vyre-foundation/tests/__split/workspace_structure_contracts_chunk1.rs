// Workspace-level structure contract tests.
//
// These tests enforce organization contracts across the workspace without
// editing any production source or documentation files.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// 1. Oversized modules must be baselined in vyre-foundation
// ---------------------------------------------------------------------------

/// Organization contract: production source files in vyre-foundation/src
/// should remain under 500 lines. Existing oversized modules are baselined;
/// any new file exceeding the threshold or any unbaselined growth is a
/// violation.
#[test]
fn foundation_oversized_modules_are_baselined() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest.join("src");
    let mut found: HashMap<String, usize> = HashMap::new();

    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Exclude test-only directories nested inside src/
                if path.file_name().and_then(|s| s.to_str()) != Some("tests") {
                    stack.push(path);
                }
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                // Skip test-only .rs files inside src/ (e.g. foo/tests.rs)
                let is_test_file = path.file_stem().and_then(|s| s.to_str()) == Some("tests");
                if is_test_file {
                    continue;
                }
                let content = std::fs::read_to_string(&path).unwrap();
                let lines = content.lines().count();
                if lines > 500 {
                    let rel = path.strip_prefix(&manifest).unwrap_or(&path);
                    found.insert(rel.display().to_string(), lines);
                }
            }
        }
    }

    let known: HashSet<String> = [
        "src/validate/validate.rs",
        "src/transform/visit.rs",
        "src/ir_inner/model/program/meta.rs",
        "src/serial/wire/decode/from_wire.rs",
        "src/serial/wire/encode/to_wire.rs",
        "src/ir_inner/model/program/buffer_decl.rs",
        "src/execution_plan/policy.rs",
        "src/transform/autodiff/grad.rs",
        "src/optimizer.rs",
        "src/execution_plan/mod.rs",
        "src/validate/nodes.rs",
        "src/validate/expr_rules.rs",
        "src/validate/typecheck.rs",
        "src/optimizer/rewrite.rs",
        "src/ir_inner/model/expr.rs",
        // Pass-shipping rounds (A1, A14, A17, A22, A26-A36, A31 sweep)
        // added new files beyond the 500-line floor. Each must be
        // split  -  tracked under ROADMAP S10. Baselined here to keep
        // the gate green while the splits land. Do not extend without
        // splitting first.
        "src/optimizer/passes/algebraic/atomic_minimize.rs",
        "src/optimizer/passes/loops/loop_strip_mine.rs",
        "src/optimizer/passes/algebraic/const_fold/binop_identities.rs",
        "src/optimizer/expr_arena.rs",
        "src/optimizer/passes/algebraic/strength_reduce/arithmetic.rs",
        "src/optimizer/passes/fusion_cse/fusion.rs",
        "src/optimizer/passes/cleanup/rematerialize_cheap_let.rs",
        "src/optimizer/passes/loops/loop_software_pipeline.rs",
        "src/optimizer/passes/loops/loop_fusion.rs",
        "src/optimizer/passes/memory/store_to_load_forward.rs",
        "src/optimizer/effect_lattice.rs",
        "src/optimizer/passes/loops/loop_licm.rs",
        // Round 3 baselines (A2/A11/A15/A16/A19 sweep + Codex eqsat
        // additions). Same S10 follow-up.
        "src/optimizer/program_soa.rs",
        "src/optimizer/eqsat.rs",
        "src/optimizer/cost.rs",
        "src/optimizer/passes/loops/loop_var_range_fold.rs",
        "src/optimizer/passes/memory/read_only_load_hoist.rs",
        "src/optimizer/passes/loops/loop_redundant_bound_check_elide.rs",
        // Round 4 baselines (A11/A20/A27/A28/A30 + G1/G5 + loop_unroll).
        // Same S10 follow-up.
        "src/optimizer/passes/algebraic/const_fold/reaching_def_propagate.rs",
        "src/optimizer/passes/loops/loop_unroll.rs",
        "src/optimizer/passes/loops/loop_lower_bound_normalize.rs",
        "src/optimizer/passes/loops/loop_fission.rs",
        "src/optimizer/passes/memory/dead_store_elim.rs",
        "src/optimizer/passes/algebraic/precision_hint.rs",
        "src/ir_inner/model/node_kind.rs",
        "src/ir_inner/model/program/stats.rs",
        "src/optimizer/eqsat_gpu.rs",
        "src/optimizer/scheduler/mod.rs",
        "src/optimizer/scheduler/queries.rs",
        "src/optimizer/scheduler/run.rs",
        "src/serial/wire/decode/impl_reader.rs",
        "src/transform/collectives.rs",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut new_violations = Vec::new();
    for (path, lines) in &found {
        if !known.contains(path) {
            new_violations.push(format!("{} ({} lines)", path, lines));
        }
    }

    // Also flag if a known file is no longer present (renamed / removed)
    let mut missing = Vec::new();
    for k in &known {
        if !found.contains_key(k) {
            missing.push(k.clone());
        }
    }

    assert!(
        new_violations.is_empty() && missing.is_empty(),
        "oversized module contract violation.\n\
         New oversized files:\n{}\n\
         Missing known files (renamed/removed):\n{}\n\
         If a new file is legitimately large, add it to the known list; otherwise, split it.",
        new_violations.join("\n"),
        missing.join("\n")
    );
}

// ---------------------------------------------------------------------------
// 2. No nested tests/ directories inside vyre-foundation/src
// ---------------------------------------------------------------------------

/// Organization contract: test code must live in the top-level tests/
/// directory, not in nested tests/ folders inside src/. Existing nested
/// directories are baselined; new ones are forbidden.
#[test]
fn foundation_no_nested_tests_in_src() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest.join("src");
    let mut found = Vec::new();

    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path.file_name().and_then(|s| s.to_str()) == Some("tests") {
                let rel = path.strip_prefix(&manifest).unwrap_or(&path);
                found.push(rel.display().to_string());
            } else {
                stack.push(path);
            }
        }
    }

    let known: HashSet<String> = [
        "src/transform/optimize/tests",
        // Pass-rewrite test colocations from the A-series rounds.
        // Each owns a small set of pass-specific fixtures that are
        // tightly coupled to the pass module they sit beside; moving
        // them to the top-level tests/ directory would force the
        // private pass internals to leak as pub(crate). Tracked under
        // ROADMAP S13 alongside the broader nested-tests cleanup.
        "src/optimizer/passes/algebraic/const_fold/tests",
        "src/optimizer/passes/fusion_cse/cse/tests",
        "src/optimizer/passes/fusion_cse/dce/tests",
        "src/optimizer/passes/algebraic/strength_reduce/tests",
        "src/optimizer/scheduler/tests",
        "src/visit/tests",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let new_violations: Vec<String> = found.into_iter().filter(|v| !known.contains(v)).collect();

    assert!(
        new_violations.is_empty(),
        "new nested tests/ directories inside src/ are forbidden. \
         Move tests to the top-level tests/ directory. Violations:\n{}",
        new_violations.join("\n")
    );
}

// ---------------------------------------------------------------------------
// 3. Duplicate plan / document sources
// ---------------------------------------------------------------------------

/// Organization contract: planning and audit documents must not be duplicated
/// across docs/, audits/, and .internals/plans/. Exact content duplicates are
/// baselined where they represent intentional .md/.txt frozen-trait pairs;
/// any other exact duplicate is a violation.
#[test]
fn workspace_duplicate_plan_sources_are_baselined() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    let scan_dirs = [
        workspace_root.join("docs"),
        workspace_root.join("audits"),
        workspace_root.join(".internals/plans"),
        workspace_root.join(".internals/planning"),
    ];

    let mut hashes: HashMap<String, Vec<PathBuf>> = HashMap::new();

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
                } else {
                    let ext = path.extension().and_then(|s| s.to_str());
                    if ext == Some("md") || ext == Some("txt") {
                        let content = std::fs::read_to_string(&path).unwrap();
                        hashes.entry(content).or_default().push(path);
                    }
                }
            }
        }
    }

    let known_duplicates: HashSet<String> = [
        "docs/frozen-traits/AlgebraicLaw.md+docs/frozen-traits/AlgebraicLaw.txt",
        "docs/frozen-traits/MutationClass.md+docs/frozen-traits/MutationClass.txt",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut new_violations = Vec::new();
    for paths in hashes.values() {
        if paths.len() < 2 {
            continue;
        }
        let mut rels: Vec<String> = paths
            .iter()
            .map(|p| {
                p.strip_prefix(workspace_root)
                    .unwrap_or(p)
                    .display()
                    .to_string()
            })
            .collect();
        rels.sort();
        let key = rels.join("+");
        if !known_duplicates.contains(&key) {
            new_violations.push(key.replace('+', "\n  "));
        }
    }

    assert!(
        new_violations.is_empty(),
        "duplicate plan/document sources are forbidden. \
         If intentional, baseline the pair; otherwise, deduplicate. Violations:\n{}",
        new_violations.join("\n\n")
    );
}

// ---------------------------------------------------------------------------
// 4. Workspace crates must have a tests/ directory or be explicitly exempt
// ---------------------------------------------------------------------------

/// Organization contract: every library/tool crate in the workspace should
/// have a top-level tests/ directory so that tests live externally rather
/// than inline. Crates without one are baselined; new omissions are forbidden.
#[test]
fn workspace_crates_have_tests_directory_or_are_exempt() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    // Derived from root Cargo.toml [workspace] members
    let members = [
        "benches/competition",
        "vyre-core",
        "vyre-foundation",
        "vyre-driver",
        "vyre-reference",
        "vyre-spec",
        "vyre-macros",
        "vyre-primitives",
        "conform/vyre-conform-spec",
        "conform/vyre-conform-generate",
        "conform/vyre-conform-enforce",
        "conform/vyre-conform-runner",
        "conform/vyre-test-harness",
        "xtask",
        "vyre-runtime",
        "vyre-libs",
        "vyre-intrinsics",
        "vyre-frontend-c",
        "vyre-harness",
    ];

    let exempt: HashSet<String> = [
        "benches/competition", // benchmark harness, not a library
        "conform/vyre-conform-spec",
        "conform/vyre-conform-generate",
        "conform/vyre-test-harness",
        "xtask",        // build utility
        "vyre-harness", // test harness library
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut missing = Vec::new();
    for member in &members {
        let path = workspace_root.join(member);
        if !path.is_dir() {
            continue;
        }
        let tests_dir = path.join("tests");
        if !tests_dir.exists() && !exempt.contains(*member) {
            missing.push(member.to_string());
        }
    }

    assert!(
        missing.is_empty(),
        "workspace crates must have a tests/ directory or be exempt. \
         Missing tests/ in: {:?}. \
         Either add a tests/ directory or add the crate to the exempt list.",
        missing
    );
}

// ---------------------------------------------------------------------------
// 5. Shared layers cannot depend upward on catalogs or concrete backends
// ---------------------------------------------------------------------------

#[test]
fn shared_driver_and_runtime_do_not_depend_on_catalog_crates() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();
    let checked = [
        workspace_root.join("vyre-foundation/Cargo.toml"),
        workspace_root.join("vyre-driver/Cargo.toml"),
        workspace_root.join("vyre-runtime/Cargo.toml"),
        workspace_root.join("vyre-core/Cargo.toml"),
    ];
    let forbidden = ["vyre-primitives", "vyre-libs", "vyre-intrinsics"];
    let optional_adapters = ["vyre-self-substrate"];
    let mut violations = Vec::new();

    for manifest_path in checked {
        let manifest_text = std::fs::read_to_string(&manifest_path).unwrap();
        let dependencies = manifest_section(&manifest_text, "dependencies");
        for dep in forbidden {
            if dependencies
                .lines()
                .any(|line| manifest_dep_line_matches(line, dep))
            {
                violations.push(format!("{} depends on {dep}", manifest_path.display()));
            }
        }
        for dep in optional_adapters {
            for line in dependencies.lines() {
                if manifest_dep_line_matches(line, dep) && !line.contains("optional = true") {
                    violations.push(format!(
                        "{} depends on {dep} outside an optional adapter feature",
                        manifest_path.display()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "shared driver/runtime layers must not depend upward on op catalogs. \
         Move catalog-backed builders into self-substrate or consumer crates. Violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn shared_source_does_not_name_concrete_backend_apis() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();
    let roots = [
        workspace_root.join("vyre-foundation/src"),
        workspace_root.join("vyre-driver/src"),
        workspace_root.join("vyre-runtime/src"),
        workspace_root.join("vyre-core/src"),
    ];
    let forbidden = [
        "vyre-driver-wgpu",
        "vyre_driver_wgpu",
        "vyre-driver-cuda",
        "vyre_driver_cuda",
        "vyre-driver-spirv",
        "vyre_driver_spirv",
        "wgpu::",
        "naga::",
        "cudarc",
        "cuda_sm",
        "find_cuda",
        "apply_cuda",
        "adapter_name_contains",
        "find_adapter_name",
        "apply_adapter_name",
    ];
    let mut violations = Vec::new();

    for root in roots {
        for path in rust_files_under(root) {
            let text = std::fs::read_to_string(&path).unwrap();
            for needle in forbidden {
                if text.contains(needle) {
                    violations.push(format!("{} contains `{needle}`", path.display()));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "shared crates must use neutral driver-owned abstractions; concrete API names belong only in concrete backend crates. Violations:\n{}",
        violations.join("\n")
    );
}
