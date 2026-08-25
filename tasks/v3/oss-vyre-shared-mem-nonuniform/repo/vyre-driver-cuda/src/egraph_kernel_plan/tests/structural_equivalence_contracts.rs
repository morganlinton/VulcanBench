use super::*;

#[test]
fn structural_equivalence_plan_emits_unique_exact_eclass_merges() {
    let snapshot = GpuEGraphSnapshot::build([
        (10u32, "lit", &[][..]),
        (20u32, "lit", &[][..]),
        (30u32, "add", &[10u32, 20u32][..]),
        (40u32, "add", &[10u32, 20u32][..]),
        (50u32, "add", &[20u32, 10u32][..]),
        (30u32, "add", &[10u32, 20u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid egraph image must pack");

    let plan = plan_cuda_egraph_structural_equivalences(
        &image,
        view_for_image(&image),
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 8,
            max_blocks_per_launch: 1,
        },
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - structural equivalence plan must build");

    assert_eq!(
        plan.equivalences,
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
    assert_eq!(plan.exact_pair_count, 4);
    assert_eq!(plan.redundant_pair_count, 1);
    assert_eq!(plan.rejected_candidate_pair_count, 0);
    assert_eq!(plan.equivalence_output_words, 4);
}

#[test]
fn structural_equivalence_collection_filters_signature_collision_bucket() {
    let snapshot = GpuEGraphSnapshot::build([(0u32, "lit", &[][..]), (1u32, "add", &[0u32][..])]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid egraph image must pack");
    let signature_plan = CudaEGraphSignatureBucketPlan {
        view: view_for_image(&image),
        buckets: vec![CudaEGraphSignatureBucket {
            signature: image.row_signatures()[0],
            first_bucket_row: 0,
            row_count: 2,
            candidate_pair_count: 1,
        }],
        bucket_rows: vec![0, 1],
        pair_waves: vec![CudaEGraphSignaturePairWave {
            bucket_index: 0,
            first_pair: 0,
            pair_count: 1,
            blocks: 1,
            threads_per_block: 1,
        }],
        candidate_pair_count: 1,
        total_blocks: 1,
    };

    let plan = collect_cuda_egraph_structural_equivalences(&image, signature_plan)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - collision-safe structural collection must complete");

    assert!(plan.equivalences.is_empty());
    assert_eq!(plan.exact_pair_count, 0);
    assert_eq!(plan.redundant_pair_count, 0);
    assert_eq!(plan.rejected_candidate_pair_count, 1);
    assert_eq!(plan.equivalence_output_words, 0);
}

#[test]
fn signature_bucket_device_image_packs_fixed_width_records() {
    let snapshot = GpuEGraphSnapshot::build([
        (0u32, "lit", &[][..]),
        (1u32, "lit", &[][..]),
        (2u32, "lit", &[][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid egraph image must pack");
    let signature_plan = plan_cuda_egraph_signature_buckets(
        &image,
        view_for_image(&image),
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 2,
            max_blocks_per_launch: 1,
        },
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - signature bucket plan must build");

    let device_image = pack_cuda_egraph_signature_bucket_device_image(&signature_plan)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - signature bucket device image must pack");

    assert_eq!(device_image.bucket_count, 1);
    assert_eq!(device_image.bucket_record_words, 5);
    assert_eq!(device_image.bucket_rows, vec![0, 1, 2]);
    assert_eq!(
        device_image.bucket_words,
        vec![image.row_signatures()[0], 0, 3, 3, 0,]
    );
    assert_eq!(device_image.candidate_pair_count, 3);
}

#[test]
fn structural_equivalence_launch_artifact_sizes_worst_case_output() {
    let snapshot = GpuEGraphSnapshot::build([
        (0u32, "lit", &[][..]),
        (1u32, "lit", &[][..]),
        (2u32, "lit", &[][..]),
        (3u32, "lit", &[][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid egraph image must pack");
    let signature_plan = plan_cuda_egraph_signature_buckets(
        &image,
        view_for_image(&image),
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 4,
            max_blocks_per_launch: 1,
        },
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - signature bucket plan must build");

    let artifact = plan_cuda_egraph_structural_equivalence_launch_artifact(&signature_plan)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - structural equivalence launch artifact must build");

    assert_eq!(artifact.bucket_image.bucket_count, 1);
    assert_eq!(artifact.output.max_equivalences, 6);
    assert_eq!(artifact.output.output_pair_words, 12);
    assert_eq!(artifact.output.output_pair_bytes, 48);
    assert_eq!(artifact.output.output_counter_words, 2);
    assert_eq!(artifact.output.output_counter_bytes, 8);
    assert_eq!(artifact.pair_waves.len(), 2);
}

