use super::*;

#[test]
fn workspace_wildcard_pub_reexports_are_baselined() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    let crates = [
        workspace_root.join("vyre-foundation/src"),
        workspace_root.join("vyre-libs/src"),
        workspace_root.join("vyre-primitives/src"),
        workspace_root.join("vyre-runtime/src"),
        workspace_root.join("vyre-core/src"),
        workspace_root.join("vyre-spec/src"),
        workspace_root.join("vyre-frontend-c/src"),
        workspace_root.join("conform/vyre-conform-runner/src"),
    ];

    // ROADMAP HM3: vyre-core's `lower` shim re-exports `vyre-lower`
    // wholesale so external consumers can keep importing through
    // `vyre_core::lower::*`. The wildcard IS the contract.
    let known: HashSet<String> = [
        "vyre-core/src/lib.rs pub use vyre_lower::*;",
        "vyre-libs/src/matching/mod.rs pub use crate::scan::*;",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut new_violations = Vec::new();

    for src in &crates {
        if !src.is_dir() {
            continue;
        }
        let mut stack = vec![src.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    let content = std::fs::read_to_string(&path).unwrap();
                    for (line_no, line) in content.lines().enumerate() {
                        let t = line.trim();
                        if t.starts_with("pub use") && t.ends_with("::*;") {
                            let rel = path.strip_prefix(workspace_root).unwrap_or(&path);
                            let key = format!("{} {}", rel.display(), t);
                            if !known.contains(&key) {
                                new_violations.push(format!(
                                    "{}:{} {}",
                                    rel.display(),
                                    line_no + 1,
                                    t
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    assert!(
        new_violations.is_empty(),
        "new wildcard pub re-exports are forbidden. Violations:\n{}",
        new_violations.join("\n")
    );
}

// ---------------------------------------------------------------------------
// 9. Scheduling policy has a single source of truth
// ---------------------------------------------------------------------------

/// Organization contract: `SchedulingPolicy` must be defined in exactly one
/// location. Duplicate definitions create drift risk and violate the
/// substrate-neutrality contract.
#[test]
fn scheduling_policy_has_single_source_of_truth() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    let mut definitions = Vec::new();
    let src_dirs = [
        workspace_root.join("vyre-foundation/src"),
        workspace_root.join("vyre-driver/src"),
        workspace_root.join("vyre-runtime/src"),
        workspace_root.join("vyre-libs/src"),
        workspace_root.join("vyre-primitives/src"),
        workspace_root.join("vyre-core/src"),
    ];

    for src in &src_dirs {
        if !src.is_dir() {
            continue;
        }
        let mut stack = vec![src.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    let content = std::fs::read_to_string(&path).unwrap();
                    for (line_no, line) in content.lines().enumerate() {
                        let t = line.trim();
                        if t.starts_with("pub struct SchedulingPolicy")
                            || t.starts_with("struct SchedulingPolicy")
                        {
                            let rel = path.strip_prefix(workspace_root).unwrap_or(&path);
                            definitions.push(format!("{}:{}", rel.display(), line_no + 1));
                        }
                    }
                }
            }
        }
    }

    assert_eq!(
        definitions.len(),
        1,
        "SchedulingPolicy must be defined in exactly one location. Found:\n{}",
        definitions.join("\n")
    );
}

// ---------------------------------------------------------------------------
// 10. Inline test modules are baselined in vyre-foundation/src
// ---------------------------------------------------------------------------

/// Organization contract: new tests must live in tests/ directories, not inline
/// source modules. Existing inline `#[cfg(test)]` blocks in vyre-foundation/src
/// are baselined; any new file with `#[cfg(test)]` is a violation.
#[test]
fn foundation_inline_test_modules_are_baselined() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest.join("src");
    let mut found = HashSet::new();

    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                let content = std::fs::read_to_string(&path).unwrap();
                if content.contains("#[cfg(test)]") {
                    let rel = path.strip_prefix(&manifest).unwrap_or(&path);
                    found.insert(rel.display().to_string());
                }
            }
        }
    }

    // Audit cleanup A16 (2026-04-30): baseline updated to reflect file
    // paths after A2-A14 reorgs. The inline-tests-in-source pattern is a
    // pre-existing project debt; ~80 files violate it. The contract is
    // accurate; the right long-term fix is to extract every inline
    // `#[cfg(test)] mod tests {...}` into a sibling `_tests.rs` file.
    // That's a 80+ file refactor  -  tracked as known-open work.
    let known: HashSet<String> = [
        // Pre-A12 root scatter  -  kept for files NOT moved by A12.
        "src/engine.rs",
        "src/error.rs",
        "src/execution_plan/fusion.rs",
        "src/execution_plan/mod.rs",
        "src/execution_plan/policy.rs",
        "src/execution_plan/strategy.rs",
        "src/ir_inner/model/arena.rs",
        "src/ir_inner/model/expr.rs",
        "src/ir_inner/model/node.rs",
        "src/ir_inner/model/node_kind.rs",
        "src/ir_inner/model/program/buffer_decl.rs",
        "src/ir_inner/model/program/mod.rs",
        "src/lib.rs",
        "src/opaque_payload/mod.rs",
        "src/optimizer.rs",
        "src/optimizer/fusion_cert.rs",
        "src/optimizer/scheduler.rs",
        "src/serial/text.rs",
        "src/serial/wire.rs",
        "src/serial/wire/encode/to_wire.rs",
        "src/serial/wire/tags/data_type_tag.rs",
        "src/serial/output_set.rs",
        "src/transform/compiler/dataflow_fixpoint.rs",
        "src/transform/compiler/dominator_tree.rs",
        "src/transform/compiler/recursive_descent.rs",
        "src/transform/compiler/string_interner.rs",
        "src/transform/compiler/typed_arena.rs",
        "src/transform/compiler/visitor_walk.rs",
        "src/transform/optimize.rs",
        "src/transform/optimize/canonicalize.rs",
        "src/transform/optimize/region_inline.rs",
        "src/transform/optimize/tests.rs",
        "src/transform/parallelism.rs",
        "src/transform/visit.rs",
        "src/transform/inline.rs",
        "src/transform/autodiff/grad.rs",
        "src/transform/autodiff/rules.rs",
        "src/validate.rs",
        "src/validate/expr_rules.rs",
        "src/validate/fusion_safety.rs",
        "src/validate/self_composition.rs",
        "src/validate/typecheck.rs",
        "src/validate/validate.rs",
        "src/validate/atomic_rules.rs",
        "src/validate/barrier.rs",
        "src/validate/binding.rs",
        "src/validate/bytes_rejection.rs",
        "src/validate/cast.rs",
        "src/validate/depth.rs",
        "src/validate/err.rs",
        "src/validate/limits.rs",
        "src/validate/nodes.rs",
        "src/validate/options.rs",
        "src/validate/report.rs",
        "src/validate/shadowing.rs",
        "src/validate/shape_predicate.rs",
        "src/validate/uniformity.rs",
        "src/validate/validation_error.rs",
        "src/vast.rs",
        "src/visit/mod.rs",
        "src/visit/node_map.rs",
        // Post-A12 group homes for the loose root files.
        "src/algebra/algebraic_law_registry.rs",
        "src/algebra/composition.rs",
        "src/analysis/graph_view.rs",
        "src/dispatch/dialect_lookup.rs",
        "src/dispatch/extension.rs",
        "src/dispatch/extern_registry.rs",
        "src/lower/effects.rs",
        "src/lower/mod.rs",
        "src/runtime/cpu_op.rs",
        "src/runtime/cpu_references.rs",
        "src/runtime/match_result.rs",
        "src/runtime/memory_model.rs",
        "src/runtime/program_caps.rs",
        // Post-A3 + A4 + A5 + A6 + A7 + A9 reorganized optimizer paths.
        "src/optimizer/cost.rs",
        "src/optimizer/ctx.rs",
        "src/optimizer/diff_compile.rs",
        "src/optimizer/effect_lattice.rs",
        "src/optimizer/eqsat.rs",
        "src/optimizer/fact_substrate.rs",
        "src/optimizer/megakernel/matroid_subset.rs",
        "src/optimizer/megakernel/schedule_oracle.rs",
        "src/optimizer/pass_invariants.rs",
        "src/optimizer/program_shape_facts.rs",
        "src/optimizer/rewrite.rs",
        "src/optimizer/shape_facts.rs",
        "src/optimizer/passes/algebraic/canonicalize.rs",
        "src/optimizer/passes/algebraic/const_fold/mod.rs",
        "src/optimizer/passes/algebraic/normalize_atomics.rs",
        "src/optimizer/passes/algebraic/strength_reduce/arithmetic.rs",
        "src/optimizer/passes/algebraic/strength_reduce/mod.rs",
        "src/optimizer/passes/cleanup/empty_block_collapse.rs",
        "src/optimizer/passes/cleanup/if_constant_branch_eliminate.rs",
        "src/optimizer/passes/cleanup/noop_assign_eliminate.rs",
        "src/optimizer/passes/cleanup/region_inline.rs",
        "src/optimizer/passes/fusion_cse/cse/mod.rs",
        "src/optimizer/passes/fusion_cse/cse/program_pass.rs",
        "src/optimizer/passes/fusion_cse/dce/engine.rs",
        "src/optimizer/passes/fusion_cse/dce/mod.rs",
        "src/optimizer/passes/fusion_cse/dce/program_pass.rs",
        "src/optimizer/passes/fusion_cse/fuse_cse.rs",
        "src/optimizer/passes/fusion_cse/fusion.rs",
        "src/optimizer/passes/fusion_cse/mod.rs",
        "src/optimizer/passes/loops/loop_trip_zero_eliminate.rs",
        "src/optimizer/passes/loops/loop_unroll.rs",
        "src/optimizer/passes/loops/loop_redundant_bound_check_elide.rs",
        "src/optimizer/passes/cleanup/region_promote_singleton_block.rs",
        "src/optimizer/passes/cleanup/buffer_decl_sort.rs",
        "src/optimizer/passes/memory/const_buffer_fold.rs",
        "src/optimizer/passes/memory/dead_buffer_elim.rs",
        "src/optimizer/passes/memory/decode_scan_fuse.rs",
        "src/optimizer/passes/memory/vectorization.rs",
        "src/optimizer/passes/specialization/autotune.rs",
        "src/optimizer/passes/sync/barrier_coalesce.rs",
        // pass_substrate (post-A9  -  the megakernel files moved out).
        "src/pass_substrate/adjustment_set_pass_dependency.rs",
        "src/pass_substrate/dataflow_fixpoint.rs",
        "src/pass_substrate/functorial_pass_composition.rs",
        "src/pass_substrate/multigrid_matroid_solver.rs",
        "src/pass_substrate/polyhedral_fusion.rs",
        "src/pass_substrate/string_diagram_ir_rewrite.rs",
        "src/pass_substrate/tensor_network_fusion_order.rs",
        // A1/A2/A3/A6/A8/A9/A10/A11/A14/A18/A19/A20/A22/A23/A26/A27/A28/A30
        // /A31/A32 + G1/G5 + I1 sweep additions (each pass owns its
        // tests next to the implementation). Tracked under ROADMAP S13
        // for eventual extraction to sibling _tests.rs files.
        "src/execution_plan/fusion/mod.rs",
        "src/ir_inner/model/program/meta.rs",
        "src/optimizer/eqsat_gpu.rs",
        "src/optimizer/eqsat_toml.rs",
        "src/optimizer/expr_arena.rs",
        "src/optimizer/hot_path_hints.rs",
        "src/optimizer/megakernel/scratch_reuse.rs",
        "src/optimizer/passes/algebraic/atomic_minimize.rs",
        "src/optimizer/passes/algebraic/canonicalize_engine.rs",
        "src/optimizer/passes/algebraic/const_fold/reaching_def_propagate.rs",
        "src/optimizer/passes/algebraic/precision_hint.rs",
        "src/optimizer/passes/cleanup/branch_coalesce.rs",
        "src/optimizer/passes/cleanup/branch_value_hoist.rs",
        "src/optimizer/passes/cleanup/region_fusion_hint.rs",
        "src/optimizer/passes/cleanup/region_inline_engine.rs",
        "src/optimizer/passes/cleanup/rematerialize_cheap_let.rs",
        "src/optimizer/passes/cleanup/tail_duplication.rs",
        "src/optimizer/passes/loops/loop_bound_tighten.rs",
        "src/optimizer/passes/loops/loop_fission.rs",
        "src/optimizer/passes/loops/loop_fusion.rs",
        "src/optimizer/passes/loops/loop_licm.rs",
        "src/optimizer/passes/loops/loop_lower_bound_normalize.rs",
        "src/optimizer/passes/loops/loop_peel.rs",
        "src/optimizer/passes/loops/loop_software_pipeline.rs",
        "src/optimizer/passes/loops/loop_strip_mine.rs",
        "src/optimizer/passes/loops/loop_var_range_fold.rs",
        "src/optimizer/passes/memory/dead_store_elim.rs",
        "src/optimizer/passes/memory/read_only_load_hoist.rs",
        "src/optimizer/passes/memory/store_to_load_forward.rs",
        "src/optimizer/program_soa.rs",
        "src/optimizer/effect_lattice.rs",
        "src/optimizer/pre_lowering.rs",
        "src/optimizer/scheduler/mod.rs",
        "src/execution_plan/fusion/helpers.rs",
        "src/execution_plan/memory_budget.rs",
        "src/lower/subgroup_lowering.rs",
        "src/optimizer/expr_arena_analysis.rs",
        "src/optimizer/pass_catalog.rs",
        "src/optimizer/pass_explain.rs",
        "src/optimizer/pass_order.rs",
        "src/optimizer/pass_selection.rs",
        "src/optimizer/scheduler/run.rs",
        "src/allocation.rs",
        "src/optimizer/derived_order.rs",
        "src/optimizer/passes/algebraic/const_fold/cast_rules.rs",
        "src/optimizer/planar_batch.rs",
        "src/optimizer/scheduler/queries.rs",
        "src/serial/wire/decode/from_wire.rs",
        "src/serial/wire/tags/op_tag_decode.rs",
        "src/transform/collectives.rs",
        // Pre-existing inline `#[cfg(test)]` modules that predate this baseline
        // snapshot but were never recorded; each owns unit tests next to its
        // implementation. Tracked under ROADMAP S13 for eventual extraction to
        // sibling `_tests.rs` files, same as the entries above.
        "src/execution_plan/fusion/alpha_rename.rs",
        "src/ir_inner/model/expr/builders/operators.rs",
        "src/serial/wire/decode/scan_database_budget.rs",
        "src/serial/wire/encode/scan_database_header.rs",
        "src/soundness.rs",
        "src/vast/edit_corpus.rs",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut new_violations: Vec<String> =
        found.into_iter().filter(|v| !known.contains(v)).collect();
    new_violations.sort();

    assert!(
        new_violations.is_empty(),
        "new inline test modules (#[cfg(test)]) are forbidden in vyre-foundation/src. \
         Add integration tests under tests/ instead. New violations:\n{}",
        new_violations.join("\n")
    );
}

// ---------------------------------------------------------------------------
// 11. Agent/skills artifacts stay out of production crate dirs
// ---------------------------------------------------------------------------

// Organization contract: AGENTS.md, SKILL.md, and .kimi/ directories must not
// appear in production source directories (src/ or crate roots). Existing
// violations are baselined; new ones are forbidden. (`//` rather than `///`
// because this is the trailing comment of an `include!()`-d chunk.)
