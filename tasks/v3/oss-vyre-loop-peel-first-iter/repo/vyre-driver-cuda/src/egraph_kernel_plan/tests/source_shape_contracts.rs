use super::*;

#[test]
fn consuming_launch_artifact_matches_borrowed_artifact_without_plan_clone_contract() {
    let snapshot = GpuEGraphSnapshot::build([
        (0u32, "lit", &[][..]),
        (1u32, "lit", &[][..]),
        (2u32, "add", &[0u32, 1u32][..]),
        (3u32, "add", &[0u32, 1u32][..]),
        (4u32, "mul", &[0u32, 1u32][..]),
        (5u32, "mul", &[0u32, 1u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid egraph image must pack");
    let plan = plan_cuda_egraph_signature_buckets(
        &image,
        view_for_image(&image),
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 8,
            max_blocks_per_launch: 1,
        },
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - signature bucket plan must build");

    let borrowed = plan_cuda_egraph_structural_equivalence_launch_artifact(&plan)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - borrowed launch artifact must build");
    let consumed = plan_cuda_egraph_structural_equivalence_launch_artifact_from_plan(plan)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - consuming launch artifact must build");

    assert_eq!(consumed, borrowed);
}

#[test]
fn resident_snapshot_try_constructors_match_infallible_snapshots() {
    let snapshot = GpuEGraphSnapshot::build([
        (0u32, "lit", &[][..]),
        (1u32, "lit", &[][..]),
        (2u32, "add", &[0u32, 1u32][..]),
        (3u32, "mul", &[1u32, 2u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid egraph image must pack");

    let full = CudaEGraphResidentColumnSnapshot::try_from_device_image(&image)
        .expect("Fix: caller must pre-size buffers; use fallible reserve or return ResourceExhausted - fallible full snapshot should reserve");
    let infallible_full = CudaEGraphResidentColumnSnapshot::from_device_image(&image);
    assert_eq!(full, infallible_full);

    let signatures = CudaEGraphResidentSignatureSnapshot::try_from_device_image(&image)
        .expect("Fix: caller must pre-size buffers; use fallible reserve or return ResourceExhausted - fallible signature snapshot should reserve");
    let from_full = CudaEGraphResidentSignatureSnapshot::try_from_column_snapshot(&full)
        .expect("Fix: caller must pre-size buffers; use fallible reserve or return ResourceExhausted - fallible signature snapshot from full columns should reserve");
    assert_eq!(signatures, from_full);
    assert_eq!(
        signatures,
        CudaEGraphResidentSignatureSnapshot::from_device_image(&image)
    );
}

#[test]
fn resident_signature_bucket_planning_does_not_clone_full_signature_snapshot() {
    let source = planner_production_source();
    let forbidden_snapshot_clone = [
        "let signature_snapshot = CudaEGraphResidentSignatureSnapshot",
        "::from_column_snapshot(snapshot)",
    ]
    .concat();
    assert!(
            !source.contains(&forbidden_snapshot_clone),
            "Fix: resident CUDA e-graph bucket planning must borrow the resident signature column instead of cloning it into a temporary snapshot."
        );
    assert!(
            source.contains("plan_cuda_egraph_structural_equivalence_launch_artifact_from_plan"),
            "Fix: CUDA e-graph release execution must use the consuming launch-artifact path so bucket rows and pair waves move into the artifact."
        );
}

#[test]
fn union_compaction_uses_reserved_eclass_index_for_generated_large_components() {
    let edge_count = 1024_u32;
    let mut equivalences = Vec::new();
    equivalences.reserve((edge_count as usize) * 3);
    let mut expected_self_pairs = 0_u64;
    for edge in 0..edge_count {
        equivalences.push(Equivalence {
            left: edge + 1,
            right: edge,
        });
        equivalences.push(Equivalence {
            left: edge,
            right: edge + 1,
        });
        if edge % 7 == 0 {
            expected_self_pairs += 1;
            equivalences.push(Equivalence {
                left: edge,
                right: edge,
            });
        }
    }

    let plan = plan_cuda_egraph_union_compaction(
        &equivalences,
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 128,
            max_blocks_per_launch: 16,
        },
    )
    .expect("Fix: generated CUDA e-graph union compaction plan should fit");

    assert_eq!(plan.canonical_pairs.len(), edge_count as usize);
    assert_eq!(plan.duplicate_pair_count, edge_count as u64);
    assert_eq!(plan.ignored_self_pair_count, expected_self_pairs);
    assert_eq!(plan.affected_eclasses.len(), edge_count as usize + 1);
    assert_eq!(plan.canonical_rewrites.len(), edge_count as usize);
    assert!(plan
        .canonical_rewrites
        .iter()
        .all(|rewrite| rewrite.representative == 0 && rewrite.eclass_id != 0));

    let source = planner_production_source();
    let production = source
        .split("#[cfg(test)]")
        .next()
        .expect("Fix: production egraph planner source must precede tests");
    let old_left_lookup = ["affected_eclasses.", "binary_search(&pair.left)"].concat();
    let old_right_lookup = ["affected_eclasses.", "binary_search(&pair.right)"].concat();
    assert!(
            production.contains("FxHashMap::<u32, usize>")
                && production.contains("let mut eclass_indices")
                && production.contains(".get(&pair.left)")
                && production.contains(".get(&pair.right)")
                && !production.contains(&old_left_lookup)
                && !production.contains(&old_right_lookup),
            "Fix: CUDA e-graph union compaction must build one reserved e-class index table instead of doing binary-search lookup for every emitted merge edge."
        );
}

#[test]
fn egraph_planner_uses_shared_monotonic_sort_fast_path() {
    let source = planner_production_source();
    let production = source
        .split("#[cfg(test)]")
        .next()
        .expect("Fix: CUDA egraph kernel planner production source must precede tests");
    let readback_source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/egraph_readback.rs"
    ))
    .expect("Fix: CUDA egraph readback source must be readable");
    let readback_production = readback_source
        .split("#[cfg(test)]")
        .next()
        .expect("Fix: CUDA egraph readback production source must precede tests");

    assert!(
            production.contains(
                "use crate::backend::ordering::{sort_unstable_by_key_if_needed, sort_unstable_if_needed};",
            )
                && production.contains("sort_unstable_by_key_if_needed(&mut sorted_rows")
                && production.contains("sort_unstable_by_key_if_needed(&mut canonical_pairs")
                && readback_production.contains("sort_unstable_by_key_if_needed(&mut unique")
                && production.contains("sort_unstable_if_needed(&mut equivalence_keys)")
                && production.contains("sort_unstable_if_needed(&mut affected_eclasses)"),
            "Fix: CUDA e-graph planning/readback must reuse the shared monotonic sort fast path."
        );
    assert!(
            !production.contains(".sort_unstable_by_key("),
            "Fix: CUDA e-graph release paths must not unconditionally sort already monotonic rows or equivalence pairs."
        );
    assert!(
            !readback_production.contains(".sort_unstable_by_key("),
            "Fix: CUDA e-graph readback must not unconditionally sort already monotonic equivalence pairs."
        );
    assert!(
            !production.contains(".sort_unstable();"),
            "Fix: CUDA e-graph release paths must not unconditionally sort already monotonic primitive queues."
        );
}

#[test]
fn egraph_planner_uses_shared_cuda_numeric_policy_for_host_boundary_counts() {
    let source = planner_production_source();
    let production = source
        .split("#[cfg(test)]")
        .next()
        .expect("Fix: CUDA egraph kernel planner production source must precede tests");

    assert!(
            production.contains("use crate::numeric::CUDA_NUMERIC;")
                && production.contains(".usize_to_u64(value, field)"),
            "Fix: CUDA e-graph host/count boundary conversions must use the shared backend numeric policy."
        );
    assert!(
        !production.contains("u64::try_from(value)"),
        "Fix: CUDA e-graph planner must not reintroduce local usize-to-u64 conversion policy."
    );
}

#[test]
fn egraph_kernel_argument_tables_reuse_wave_staging() {
    let source = planner_production_source();
    let args_source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/egraph_kernel_plan/args.rs"
    ))
    .expect("Fix: CUDA egraph kernel argument source must be readable");

    assert!(
            args_source.contains("fn write_kernel_args_into(")
                && args_source.contains("fn reserve_egraph_kernel_args(")
                && source.matches("let mut kernel_args = SmallVec::<[*mut std::ffi::c_void; 8]>::new();").count() >= 3,
            "Fix: CUDA e-graph multi-wave kernels must reuse caller-owned argument tables across waves."
        );
    let forbidden_as_kernel_args = ["fn as_", "kernel_args("].concat();
    let forbidden_smallvec_macro = ["smallvec::", "smallvec!["].concat();
    assert!(
            !args_source.contains(&forbidden_as_kernel_args)
                && !args_source.contains(&forbidden_smallvec_macro),
            "Fix: CUDA e-graph wave launch code must not allocate a fresh SmallVec argument table per wave."
        );
}

#[test]
fn structural_equivalence_readback_skips_bucket_metadata() {
    let source = planner_production_source();
    let forbidden_full_scratch_download = ["self.download_", "resident(scratch.handle)"].concat();

    assert!(
            source.contains("download_structural_equivalence_output_ranges(self, &scratch)")
                && source.contains("download_resident_ranges_into(&ranges, &mut outputs)"),
            "Fix: CUDA e-graph structural-equivalence readback must use ranged fused D2H for output counter + output pairs only."
        );
    assert!(
            !source.contains(&forbidden_full_scratch_download),
            "Fix: CUDA e-graph structural-equivalence readback must not download bucket metadata after launch."
        );
}

#[test]
fn egraph_warm_helpers_reuse_resolved_cuda_function_for_launch() {
    let source = planner_production_source();
    let warm_lookup = concat!("module_for_ptx", "_with_key(&kernel.source, module_key)");
    let stale_inner_lookup = concat!("module_for_ptx", "_with_key(ptx_src, module_key)");
    let stale_inner_param = concat!(
        "ptx_src: &str,",
        "\n        module_key: crate::backend::ModuleCacheKey"
    );

    assert_eq!(
        source.matches(warm_lookup).count(),
        3,
        "Fix: each e-graph warm helper should resolve its CUDA function exactly once."
    );
    assert!(
        source.matches("Ok((kernel, function))").count() >= 3
            && source.matches("cudarc::driver::sys::CUfunction").count() >= 6,
        "Fix: e-graph warm helpers must return the resolved CUfunction to run-inner launch paths."
    );
    assert!(
        !source.contains(stale_inner_lookup) && !source.contains(&stale_inner_param),
        "Fix: e-graph run-inner paths must not repeat module-cache lookups after warm resolution."
    );
}

