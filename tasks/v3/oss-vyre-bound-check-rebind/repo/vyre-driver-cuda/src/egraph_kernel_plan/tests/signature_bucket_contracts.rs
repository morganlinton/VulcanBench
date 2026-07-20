use super::*;

#[test]
fn signature_bucket_planner_splits_large_candidate_bucket() {
    let snapshot = GpuEGraphSnapshot::build([
        (0u32, "lit", &[][..]),
        (1u32, "lit", &[][..]),
        (2u32, "lit", &[][..]),
        (3u32, "lit", &[][..]),
        (4u32, "lit", &[][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid egraph image must pack");
    let plan = plan_cuda_egraph_signature_buckets(
        &image,
        view_for_image(&image),
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 2,
            max_blocks_per_launch: 2,
        },
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - signature bucket plan must build");

    assert_eq!(plan.buckets.len(), 1);
    assert_eq!(plan.buckets[0].row_count, 5);
    assert_eq!(plan.candidate_pair_count, 10);
    assert_eq!(plan.bucket_rows, vec![0, 1, 2, 3, 4]);
    assert_eq!(
        plan.pair_waves,
        vec![
            CudaEGraphSignaturePairWave {
                bucket_index: 0,
                first_pair: 0,
                pair_count: 4,
                blocks: 2,
                threads_per_block: 2,
            },
            CudaEGraphSignaturePairWave {
                bucket_index: 0,
                first_pair: 4,
                pair_count: 4,
                blocks: 2,
                threads_per_block: 2,
            },
            CudaEGraphSignaturePairWave {
                bucket_index: 0,
                first_pair: 8,
                pair_count: 2,
                blocks: 1,
                threads_per_block: 2,
            },
        ]
    );
    assert_eq!(plan.total_blocks, 5);
}

#[test]
fn signature_pair_ordinals_decode_to_row_pairs_without_materialized_pairs() {
    let snapshot = GpuEGraphSnapshot::build([
        (0u32, "lit", &[][..]),
        (1u32, "lit", &[][..]),
        (2u32, "lit", &[][..]),
        (3u32, "lit", &[][..]),
        (4u32, "lit", &[][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid egraph image must pack");
    let plan = plan_cuda_egraph_signature_buckets(
        &image,
        view_for_image(&image),
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 4,
            max_blocks_per_launch: 1,
        },
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - signature bucket plan must build");

    let decoded = (0..plan.candidate_pair_count)
        .map(|ordinal| cuda_egraph_signature_pair_rows(&plan, 0, ordinal).unwrap())
        .collect::<Vec<_>>();

    assert_eq!(
        decoded,
        vec![
            (0, 1),
            (0, 2),
            (0, 3),
            (0, 4),
            (1, 2),
            (1, 3),
            (1, 4),
            (2, 3),
            (2, 4),
            (3, 4),
        ]
    );
}

#[test]
fn signature_pair_decoder_rejects_out_of_bounds_ordinals() {
    let snapshot = GpuEGraphSnapshot::build([(0u32, "lit", &[][..]), (1u32, "lit", &[][..])]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid egraph image must pack");
    let plan = plan_cuda_egraph_signature_buckets(
        &image,
        view_for_image(&image),
        CudaEGraphKernelLaunchConfig::default(),
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - signature bucket plan must build");

    assert_eq!(
        cuda_egraph_signature_pair_rows(&plan, 0, 1)
            .expect_err("one two-row bucket has exactly one pair"),
        CudaEGraphKernelPlanError::SignaturePairOrdinalOutOfBounds {
            bucket_index: 0,
            pair_ordinal: 1,
            candidate_pair_count: 1,
        }
    );
    assert_eq!(
        cuda_egraph_signature_pair_rows(&plan, 7, 0).expect_err("missing bucket must be rejected"),
        CudaEGraphKernelPlanError::SignaturePairOrdinalOutOfBounds {
            bucket_index: 7,
            pair_ordinal: 0,
            candidate_pair_count: 0,
        }
    );
}

#[test]
fn signature_pair_decoder_rejects_malformed_bucket_row_ranges() {
    let snapshot = GpuEGraphSnapshot::build([(0u32, "lit", &[][..]), (1u32, "lit", &[][..])]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid egraph image must pack");
    let plan = CudaEGraphSignatureBucketPlan {
        view: view_for_image(&image),
        buckets: vec![CudaEGraphSignatureBucket {
            signature: image.row_signatures()[0],
            first_bucket_row: 1,
            row_count: 2,
            candidate_pair_count: 1,
        }],
        bucket_rows: vec![0, 1],
        pair_waves: Vec::new(),
        candidate_pair_count: 1,
        total_blocks: 0,
    };

    assert_eq!(
        cuda_egraph_signature_pair_rows(&plan, 0, 0)
            .expect_err("malformed bucket row range must be rejected"),
        CudaEGraphKernelPlanError::SignatureBucketRowsOutOfBounds {
            bucket_index: 0,
            first_bucket_row: 1,
            row_count: 2,
            bucket_rows_len: 2,
        }
    );
}

