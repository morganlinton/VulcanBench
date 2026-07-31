//! Contracts for graph-domain single-sourcing between `vyre-primitives` and
//! `vyre-self-substrate`.
//!
//! The primitive crate owns graph algorithms and validation. Self-substrate
//! may add dispatch scratch, batching, plan-cache, and backend wiring, but it
//! must not re-fork primitive graph logic at the crate root or in parallel
//! implementations.

use std::fs;
use std::path::{Path, PathBuf};

struct WrapperContract {
    file: &'static str,
    primitive_module: &'static str,
    required_tokens: &'static [&'static str],
    max_wrapper_lines: usize,
}

const GRAPH_WRAPPERS: &[WrapperContract] = &[
    WrapperContract {
        file: "adaptive_traverse.rs",
        primitive_module: "adaptive_traverse",
        required_tokens: &[
            "primitive_adaptive_sparse_dense_step",
            "primitive_csr_queue_forward_traverse",
            "validate_adaptive_frontier",
        ],
        max_wrapper_lines: 775,
    },
    WrapperContract {
        file: "alias_registry.rs",
        primitive_module: "alias_registry",
        required_tokens: &[
            "primitive_default_alias_registry",
            "primitive_alias_union_registered",
            "ALIAS_UNION_OP_ID",
        ],
        max_wrapper_lines: 102,
    },
    WrapperContract {
        file: "csr_bidirectional.rs",
        primitive_module: "csr_bidirectional",
        required_tokens: &[
            "plan_csr_bidirectional_step",
            "merge_frontier_or_changed",
            "reference_csr_bidir",
        ],
        max_wrapper_lines: 360,
    },
    WrapperContract {
        file: "csr_forward_or_changed.rs",
        primitive_module: "csr_forward_or_changed",
        required_tokens: &[
            "plan_csr_forward_or_changed_dispatch",
            "plan.uses_changed_history",
            "csr_foc_cpu",
        ],
        max_wrapper_lines: 570,
    },
    WrapperContract {
        file: "dominator_frontier.rs",
        primitive_module: "dominator_frontier",
        required_tokens: &[
            "plan_dominator_frontier_dispatch",
            "primitive_frontier_size",
            "reference_dominator_frontier",
        ],
        max_wrapper_lines: 520,
    },
    WrapperContract {
        file: "exploded.rs",
        primitive_module: "exploded",
        required_tokens: &[
            "plan_ifds_csr_dispatch",
            "canonicalize_csr_within_rows_in_place",
            "build_cpu_reference",
        ],
        max_wrapper_lines: 420,
    },
    WrapperContract {
        file: "motif.rs",
        primitive_module: "motif",
        required_tokens: &[
            "plan_motif_dispatch",
            "count_witness_participants",
            "reference_motif",
        ],
        max_wrapper_lines: 609,
    },
    WrapperContract {
        file: "path_reconstruct.rs",
        primitive_module: "path_reconstruct",
        required_tokens: &[
            "plan_path_reconstruct_dispatch",
            "plan_batched_path_reconstruct_dispatch",
            "validate_path_reconstruct_readback",
            "validate_batched_path_reconstruct_readback",
            "path_reconstruct_cpu",
        ],
        max_wrapper_lines: 260,
    },
    WrapperContract {
        file: "persistent_bfs.rs",
        primitive_module: "persistent_bfs",
        required_tokens: &[
            "plan_persistent_bfs_dispatch",
            "plan_persistent_bfs_resident_batch_dispatch",
            "primitive_persistent_bfs_layout_hash",
        ],
        max_wrapper_lines: 1234,
    },
    WrapperContract {
        file: "toposort.rs",
        primitive_module: "toposort",
        required_tokens: &[
            "plan_toposort_csr_dispatch",
            "validate_toposort_csr_order",
            "toposort_csr_into",
        ],
        max_wrapper_lines: 260,
    },
    WrapperContract {
        file: "vast_tree_walk.rs",
        primitive_module: "vast_tree_walk",
        required_tokens: &[
            "try_ast_walk_preorder",
            "try_ast_walk_postorder",
            "ast_walk_preorder",
            "ast_walk_postorder",
        ],
        max_wrapper_lines: 178,
    },
];

#[test]
fn duplicated_graph_files_are_not_flat_root_modules() {
    let manifest = manifest_dir();
    for contract in GRAPH_WRAPPERS {
        let old_root = manifest.join("src").join(contract.file);
        let graph_wrapper = graph_wrapper_path(&manifest, contract.file);
        assert!(
            !old_root.exists(),
            "{} must not exist; graph substrate wrappers belong under src/graph/",
            old_root.display()
        );
        assert!(
            graph_wrapper.is_file(),
            "{} must exist as the graph-domain wrapper",
            graph_wrapper.display()
        );
    }
}

#[test]
fn graph_wrappers_import_their_primitive_authority() {
    let manifest = manifest_dir();
    let mut failures = Vec::new();

    for contract in GRAPH_WRAPPERS {
        let path = graph_wrapper_path(&manifest, contract.file);
        let source = read_wrapper_with_child_tests(&path);
        let primitive_path = format!("vyre_primitives::graph::{}", contract.primitive_module);
        if !source.contains(&primitive_path) {
            failures.push(format!(
                "{} does not import primitive authority {primitive_path}",
                contract.file
            ));
        }
        for token in contract.required_tokens {
            if !source.contains(token) {
                failures.push(format!(
                    "{} is missing primitive delegation token `{token}`",
                    contract.file
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "graph self-substrate wrappers must delegate algorithms and validation to vyre-primitives:\n{}",
        failures.join("\n")
    );
}

#[test]
fn graph_wrappers_keep_closure_bar_tests_near_dispatch_wiring() {
    let manifest = manifest_dir();
    let mut failures = Vec::new();

    for contract in GRAPH_WRAPPERS {
        let path = graph_wrapper_path(&manifest, contract.file);
        let source = read_wrapper_with_child_tests(&path);
        if !source.contains("matches_primitive_directly")
            && !source.contains("equals primitive")
            && !source.contains("primitive output")
        {
            failures.push(format!(
                "{} lacks an explicit primitive-equivalence closure-bar test",
                contract.file
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "every fork-risk graph wrapper needs an in-module primitive equivalence test:\n{}",
        failures.join("\n")
    );
}

#[test]
fn graph_mod_declares_every_single_sourced_wrapper_once() {
    let manifest = manifest_dir();
    let source = read(&manifest.join("src").join("graph").join("mod.rs"));
    let mut failures = Vec::new();

    for contract in GRAPH_WRAPPERS {
        let module = contract.file.trim_end_matches(".rs");
        let declaration = format!("pub mod {module};");
        if source.matches(&declaration).count() != 1 {
            failures.push(format!(
                "src/graph/mod.rs must declare `{declaration}` exactly once"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "graph module declarations drifted:\n{}",
        failures.join("\n")
    );
}

#[test]
fn graph_wrappers_do_not_grow_past_single_source_ratchet() {
    let manifest = manifest_dir();
    let mut failures = Vec::new();

    for contract in GRAPH_WRAPPERS {
        let path = graph_wrapper_path(&manifest, contract.file);
        let source = read(&path);
        let observed = source.lines().count();
        if observed > contract.max_wrapper_lines {
            failures.push(format!(
                "{} has {observed} lines, above single-source ratchet {}. Move reusable graph logic into vyre-primitives instead of expanding the wrapper.",
                contract.file, contract.max_wrapper_lines
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "graph wrappers must get thinner over time, never grow forked logic:\n{}",
        failures.join("\n")
    );
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn graph_wrapper_path(manifest: &Path, file: &str) -> PathBuf {
    let direct = manifest.join("src").join("graph").join(file);
    if direct.exists() {
        return direct;
    }
    let stem = file
        .strip_suffix(".rs")
        .expect("graph wrapper contract files must use .rs");
    manifest.join("src").join("graph").join(stem).join("mod.rs")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn read_wrapper_with_child_tests(path: &Path) -> String {
    let mut source = read(path);
    if let (Some(parent), Some(stem)) = (path.parent(), path.file_stem()) {
        let child_dir = if path.file_name().is_some_and(|name| name == "mod.rs") {
            parent.to_path_buf()
        } else {
            parent.join(stem)
        };
        if child_dir.is_dir() {
            let mut children = fs::read_dir(&child_dir)
                .unwrap_or_else(|err| panic!("{} must be readable: {err}", child_dir.display()))
                .map(|entry| {
                    entry
                        .unwrap_or_else(|err| {
                            panic!("{} entry must be readable: {err}", child_dir.display())
                        })
                        .path()
                })
                .filter(|child| child.extension().is_some_and(|ext| ext == "rs"))
                .collect::<Vec<_>>();
            children.sort();
            for child in children {
                source.push('\n');
                source.push_str(&read(&child));
            }
        }
    }
    source
}
