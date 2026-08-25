use super::*;

#[test]
fn egraph_structural_equivalence_kernel_ptx_loads_on_live_cuda_driver() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let before = backend
        .cached_module_count()
        .expect("Fix: CUDA module cache count must be readable before e-graph kernel warm-load.");

    let kernel = backend
        .warm_egraph_structural_equivalence_kernel()
        .expect("Fix: CUDA driver rejected the generated e-graph structural-equivalence PTX.");
    let after_first = backend
        .cached_module_count()
        .expect("Fix: CUDA module cache count must be readable after e-graph kernel warm-load.");
    let second = backend
        .warm_egraph_structural_equivalence_kernel()
        .expect("Fix: CUDA e-graph structural-equivalence PTX must remain cache-loadable.");
    let after_second = backend
        .cached_module_count()
        .expect("Fix: CUDA module cache count must be readable after cached warm-load.");

    assert_eq!(
        kernel.entry_name,
        CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_ENTRY
    );
    assert_eq!(second.source, kernel.source);
    assert!(
        after_first >= before,
        "Fix: CUDA e-graph PTX warm-load must not shrink the module cache."
    );
    assert_eq!(
        after_second, after_first,
        "Fix: repeated e-graph PTX warm-load should hit the module cache instead of inserting duplicate modules."
    );
}

#[test]
fn egraph_structural_equivalence_kernel_emits_live_cuda_pairs() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let snapshot = GpuEGraphSnapshot::build([
        (10u32, "lit", &[][..]),
        (20u32, "lit", &[][..]),
        (30u32, "add", &[10u32, 20u32][..]),
        (40u32, "add", &[10u32, 20u32][..]),
        (50u32, "add", &[20u32, 10u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: valid foundation e-graph image must pack.");
    let upload_plan = plan_cuda_egraph_device_upload_from_image(image.clone())
        .expect("Fix: packed e-graph image must produce a CUDA upload plan.");
    let resident = backend
        .upload_egraph_device_image_plan(upload_plan)
        .expect("Fix: CUDA e-graph resident image upload failed.");
    let view = backend
        .egraph_device_kernel_view(resident)
        .expect("Fix: CUDA e-graph resident image must expose kernel pointers.");
    let signature_plan = plan_cuda_egraph_signature_buckets(
        &image,
        view,
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 8,
            max_blocks_per_launch: 1,
        },
    )
    .expect("Fix: CUDA e-graph signature bucket planning must succeed.");
    let artifact = plan_cuda_egraph_structural_equivalence_launch_artifact(&signature_plan)
        .expect("Fix: CUDA e-graph structural-equivalence artifact must build.");

    let result = backend
        .run_egraph_structural_equivalence_kernel(resident, &artifact)
        .expect("Fix: live CUDA e-graph structural-equivalence kernel launch failed.");

    assert_eq!(result.device_reported_count, 2);
    assert!(!result.overflowed_output_capacity);
    assert_eq!(
        result.unique,
        vec![
            Equivalence {
                left: 10,
                right: 20,
            },
            Equivalence {
                left: 30,
                right: 40,
            },
        ]
    );

    backend
        .free_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image free failed.");
}

#[test]
fn egraph_structural_equivalence_discovery_api_runs_end_to_end() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let snapshot = GpuEGraphSnapshot::build([
        (10u32, "lit", &[][..]),
        (20u32, "lit", &[][..]),
        (30u32, "add", &[10u32, 20u32][..]),
        (40u32, "add", &[10u32, 20u32][..]),
        (50u32, "add", &[20u32, 10u32][..]),
        (60u32, "mul", &[30u32, 40u32][..]),
        (70u32, "mul", &[30u32, 40u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: valid foundation e-graph image must pack.");

    let result = backend
        .discover_egraph_structural_equivalences(
            image,
            CudaEGraphKernelLaunchConfig {
                threads_per_block: 4,
                max_blocks_per_launch: 1,
            },
        )
        .expect(
            "Fix: live CUDA e-graph discovery API must own upload, launch, readback, and cleanup.",
        );

    assert_eq!(result.device_reported_count, 3);
    assert!(!result.overflowed_output_capacity);
    assert_eq!(
        result.unique,
        vec![
            Equivalence {
                left: 10,
                right: 20,
            },
            Equivalence {
                left: 30,
                right: 40,
            },
            Equivalence {
                left: 60,
                right: 70,
            },
        ]
    );
}

#[test]

fn egraph_structural_equivalence_kernel_rejects_forced_ordering_bucket() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let snapshot = GpuEGraphSnapshot::build([
        (10u32, "lit", &[][..]),
        (20u32, "lit", &[][..]),
        (30u32, "add", &[10u32, 20u32][..]),
        (40u32, "add", &[20u32, 10u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: valid foundation e-graph image must pack.");
    let upload_plan = plan_cuda_egraph_device_upload_from_image(image.clone())
        .expect("Fix: packed e-graph image must produce a CUDA upload plan.");
    let resident = backend
        .upload_egraph_device_image_plan(upload_plan)
        .expect("Fix: CUDA e-graph resident image upload failed.");
    let artifact = CudaEGraphStructuralEquivalenceLaunchArtifact {
        bucket_image: CudaEGraphSignatureBucketDeviceImage {
            bucket_words: vec![image.row_signatures()[2], 0, 2, 1, 0],
            bucket_rows: vec![2, 3],
            bucket_count: 1,
            bucket_record_words: CUDA_EGRAPH_SIGNATURE_BUCKET_RECORD_WORDS,
            candidate_pair_count: 1,
        },
        output: CudaEGraphStructuralEquivalenceOutputPlan {
            max_equivalences: 1,
            output_pair_words: 2,
            output_pair_bytes: 8,
            output_counter_words: 2,
            output_counter_bytes: 8,
        },
        pair_waves: vec![CudaEGraphSignaturePairWave {
            bucket_index: 0,
            first_pair: 0,
            pair_count: 1,
            blocks: 1,
            threads_per_block: 1,
        }],
    };

    let result = backend.run_egraph_structural_equivalence_kernel(resident, &artifact);
    backend
        .free_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image free failed.");
    let result = result.expect(
        "Fix: live CUDA e-graph kernel must reject forced non-equivalent ordering without failing launch.",
    );

    assert_eq!(result.device_reported_count, 0);
    assert_eq!(result.emitted_pair_count, 0);
    assert!(!result.overflowed_output_capacity);
    assert!(result.unique.is_empty());
}

/// Verify that the structural-equivalence kernel fails closed (returns `Err`)
/// when the device reports more pairs than the planned `max_equivalences`
/// buffer can hold, instead of silently truncating the equivalence set and
/// feeding an incomplete union plan to `plan_cuda_egraph_union_compaction`.
///
/// Regression guard for the silent-truncation defect: before the fix,
/// `run_egraph_structural_equivalence_kernel_inner` returned `Ok` with
/// `overflowed_output_capacity: true` and a partial `unique` set, and the
/// only production caller (`backend_canonicalization.rs`) never checked the
/// flag — corrupting the canonicalization round silently.
///
/// This test uses the same 4-row + forced-ordering artifact shape as
/// `egraph_structural_equivalence_kernel_rejects_forced_ordering_bucket`
/// but points at two structurally equal rows (`add(10, 20)` vs `add(10, 20)`)
/// and constrains `max_equivalences` to 0 so any positive device count exceeds
/// capacity.  The kernel must return `Err`, not `Ok` with a truncated set.
#[test]
fn egraph_structural_equivalence_kernel_fails_closed_on_output_overflow() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    // Two rows that are structurally identical: `add(10, 20)` and `add(10, 20)`.
    // The kernel will emit exactly 1 equivalence pair (30, 40).
    // We cap `max_equivalences` to 0 so device_reported_count (1) > planned_capacity (0),
    // triggering the fail-closed path.
    let snapshot = GpuEGraphSnapshot::build([
        (10u32, "lit", &[][..]),
        (20u32, "lit", &[][..]),
        (30u32, "add", &[10u32, 20u32][..]),
        (40u32, "add", &[10u32, 20u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: valid foundation e-graph image must pack.");
    let upload_plan = plan_cuda_egraph_device_upload_from_image(image.clone())
        .expect("Fix: packed e-graph image must produce a CUDA upload plan.");
    let resident = backend
        .upload_egraph_device_image_plan(upload_plan)
        .expect("Fix: CUDA e-graph resident image upload failed.");
    let view = backend
        .egraph_device_kernel_view(resident)
        .expect("Fix: CUDA e-graph resident image must expose kernel pointers.");
    let signature_plan = plan_cuda_egraph_signature_buckets(
        &image,
        view,
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 8,
            max_blocks_per_launch: 1,
        },
    )
    .expect("Fix: CUDA e-graph signature bucket planning must succeed.");
    // Use the fully-computed artifact (correct bucket_words, waves, etc.) but
    // override the output plan to cap at 0 equivalences.  This forces
    // device_reported_count (1) to exceed planned_capacity (0).
    let correct_artifact = plan_cuda_egraph_structural_equivalence_launch_artifact(&signature_plan)
        .expect("Fix: structural equivalence launch artifact must build from valid plan.");
    let overflow_artifact = CudaEGraphStructuralEquivalenceLaunchArtifact {
        output: CudaEGraphStructuralEquivalenceOutputPlan {
            // 0 capacity: any equivalence pair makes device_reported_count > 0 = planned_capacity.
            max_equivalences: 0,
            // Keep the output buffers large enough that the kernel can write at least one
            // pair without a bounds error — we want to test the counter check, not I/O size.
            output_pair_words: correct_artifact.output.output_pair_words,
            output_pair_bytes: correct_artifact.output.output_pair_bytes,
            output_counter_words: correct_artifact.output.output_counter_words,
            output_counter_bytes: correct_artifact.output.output_counter_bytes,
        },
        ..correct_artifact
    };

    let result = backend.run_egraph_structural_equivalence_kernel(resident, &overflow_artifact);
    backend
        .free_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image free failed.");

    let err = result.expect_err(
        "Fix: run_egraph_structural_equivalence_kernel must return Err when \
         device_reported_count > max_equivalences; before the fix this returned \
         Ok(overflowed_output_capacity: true) which silently fed a truncated set \
         to plan_cuda_egraph_union_compaction and corrupted canonicalization.",
    );
    let msg = err.to_string();
    assert!(
        msg.contains("max_equivalences") || msg.contains("capacity") || msg.contains("equivalence"),
        "Fix: overflow error must identify the capacity limit and instruct the caller to \
         re-plan with a larger max_equivalences; got: {msg}"
    );
}

