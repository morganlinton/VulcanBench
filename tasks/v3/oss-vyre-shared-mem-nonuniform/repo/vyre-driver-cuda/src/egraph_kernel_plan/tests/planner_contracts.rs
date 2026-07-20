use super::*;

#[test]
fn planner_emits_passes_in_row_child_group_order() {
    let view = synthetic_view(3, 2, 2);
    let plan = plan_cuda_egraph_kernel_work(
        view,
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 4,
            max_blocks_per_launch: 2,
        },
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid egraph kernel plan");

    assert_eq!(plan.waves.len(), 3);
    assert_eq!(plan.waves[0].pass, CudaEGraphKernelPass::RowScan);
    assert_eq!(plan.waves[0].item_count, 3);
    assert_eq!(plan.waves[0].blocks, 1);
    assert_eq!(plan.waves[1].pass, CudaEGraphKernelPass::ChildEdgeScan);
    assert_eq!(plan.waves[1].item_count, 2);
    assert_eq!(plan.waves[2].pass, CudaEGraphKernelPass::EclassGroupScan);
    assert_eq!(plan.waves[2].item_count, 2);
    assert_eq!(plan.total_items, 7);
    assert_eq!(plan.total_blocks, 3);
}

#[test]
fn planner_splits_large_passes_into_bounded_waves() {
    let view = synthetic_view(19, 0, 0);
    let plan = plan_cuda_egraph_kernel_work(
        view,
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 4,
            max_blocks_per_launch: 2,
        },
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid split egraph kernel plan");

    let items = plan
        .waves
        .iter()
        .map(|wave| (wave.first_item, wave.item_count, wave.blocks))
        .collect::<Vec<_>>();
    assert_eq!(
        items,
        vec![
            (0, 8, 2),
            (8, 8, 2),
            (16, 3, 1),
            (0, 8, 2),
            (8, 8, 2),
            (16, 3, 1),
        ]
    );
    assert_eq!(plan.total_items, 38);
    assert_eq!(plan.total_blocks, 10);
}

#[test]
fn planner_rejects_zero_launch_dimensions() {
    let view = synthetic_view(1, 0, 0);
    assert_eq!(
        plan_cuda_egraph_kernel_work(
            view,
            CudaEGraphKernelLaunchConfig {
                threads_per_block: 0,
                max_blocks_per_launch: 1,
            },
        )
        .expect_err("zero threads must be rejected"),
        CudaEGraphKernelPlanError::ZeroThreadsPerBlock
    );
    assert_eq!(
        plan_cuda_egraph_kernel_work(
            view,
            CudaEGraphKernelLaunchConfig {
                threads_per_block: 1,
                max_blocks_per_launch: 0,
            },
        )
        .expect_err("zero max blocks must be rejected"),
        CudaEGraphKernelPlanError::ZeroMaxBlocksPerLaunch
    );
}

#[test]
fn signature_bucket_planner_groups_only_candidate_duplicate_rows() {
    let snapshot = GpuEGraphSnapshot::build([
        (0u32, "lit", &[][..]),
        (1u32, "lit", &[][..]),
        (2u32, "add", &[0u32, 1u32][..]),
        (3u32, "add", &[0u32, 1u32][..]),
        (4u32, "add", &[1u32, 0u32][..]),
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

    let grouped_rows = plan
        .buckets
        .iter()
        .map(|bucket| {
            let start = bucket.first_bucket_row as usize;
            let end = start + bucket.row_count as usize;
            plan.bucket_rows[start..end].to_vec()
        })
        .collect::<Vec<_>>();

    assert_eq!(grouped_rows.len(), 2);
    assert!(grouped_rows.contains(&vec![0, 1]));
    assert!(grouped_rows.contains(&vec![2, 3]));
    assert_eq!(plan.candidate_pair_count, 2);
    assert_eq!(plan.pair_waves.len(), 2);
    assert!(plan
        .pair_waves
        .iter()
        .all(|wave| wave.pair_count == 1 && wave.blocks == 1));
}

#[test]
fn structural_equivalence_planner_rejects_divergent_language_op_ids() {
    let snapshot = GpuEGraphSnapshot::build([(10u32, "lit", &[][..]), (20u32, "opaque", &[][..])]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: valid divergent-op egraph image must pack");

    assert_ne!(
        image.row_language_op_ids()[0],
        image.row_language_op_ids()[1]
    );

    let plan = plan_cuda_egraph_structural_equivalences(
        &image,
        view_for_image(&image),
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 8,
            max_blocks_per_launch: 1,
        },
    )
    .expect("Fix: divergent-op egraph image must remain plannable");

    assert!(plan.signature_plan.buckets.is_empty());
    assert_eq!(plan.signature_plan.candidate_pair_count, 0);
    assert!(plan.equivalences.is_empty());
    assert_eq!(plan.exact_pair_count, 0);
    assert_eq!(plan.rejected_candidate_pair_count, 0);
}

