use super::*;

#[test]
fn egraph_union_compaction_plan_canonicalizes_duplicates_reversals_and_chains() {
    let plan = plan_cuda_egraph_union_compaction(
        &[
            Equivalence { left: 5, right: 3 },
            Equivalence { left: 3, right: 5 },
            Equivalence { left: 8, right: 5 },
            Equivalence { left: 9, right: 9 },
            Equivalence {
                left: 11,
                right: 10,
            },
            Equivalence {
                left: 12,
                right: 11,
            },
        ],
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 2,
            max_blocks_per_launch: 1,
        },
    )
    .expect(
        "Fix: CUDA e-graph union compaction planning must accept hostile duplicate merge batches.",
    );

    assert_eq!(plan.ignored_self_pair_count, 1);
    assert_eq!(plan.duplicate_pair_count, 1);
    assert_eq!(
        plan.canonical_pairs,
        vec![
            Equivalence { left: 3, right: 5 },
            Equivalence { left: 5, right: 8 },
            Equivalence {
                left: 10,
                right: 11,
            },
            Equivalence {
                left: 11,
                right: 12,
            },
        ]
    );
    assert_eq!(plan.affected_eclasses, vec![3, 5, 8, 10, 11, 12]);
    assert_eq!(
        plan.canonical_rewrites,
        vec![
            CudaEGraphCanonicalRewrite {
                eclass_id: 5,
                representative: 3,
            },
            CudaEGraphCanonicalRewrite {
                eclass_id: 8,
                representative: 3,
            },
            CudaEGraphCanonicalRewrite {
                eclass_id: 11,
                representative: 10,
            },
            CudaEGraphCanonicalRewrite {
                eclass_id: 12,
                representative: 10,
            },
        ]
    );
    assert_eq!(plan.total_items, 8);
    assert_eq!(plan.total_blocks, 4);
    assert_eq!(plan.waves.len(), 4);
    assert_eq!(
        plan.waves[0].pass,
        CudaEGraphUnionCompactionPass::UnionPairs
    );
    assert_eq!(plan.waves[0].first_item, 0);
    assert_eq!(plan.waves[0].item_count, 2);
    assert_eq!(
        plan.waves[1].pass,
        CudaEGraphUnionCompactionPass::UnionPairs
    );
    assert_eq!(plan.waves[1].first_item, 2);
    assert_eq!(
        plan.waves[2].pass,
        CudaEGraphUnionCompactionPass::CanonicalRewrites
    );
    assert_eq!(plan.waves[2].first_item, 0);
    assert_eq!(
        plan.waves[3].pass,
        CudaEGraphUnionCompactionPass::CanonicalRewrites
    );
    assert_eq!(plan.waves[3].first_item, 2);
}

#[test]
fn egraph_union_compaction_plan_splits_oversized_merge_batches() {
    let pairs = (0..17)
        .map(|index| Equivalence {
            left: index,
            right: index + 1,
        })
        .collect::<Vec<_>>();

    let plan = plan_cuda_egraph_union_compaction(
        &pairs,
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 4,
            max_blocks_per_launch: 2,
        },
    )
    .expect("Fix: CUDA e-graph union compaction planning must split oversized batches.");

    assert_eq!(plan.canonical_pairs.len(), 17);
    assert_eq!(plan.canonical_rewrites.len(), 17);
    assert_eq!(plan.total_items, 34);
    assert_eq!(plan.waves.len(), 6);
    assert_eq!(
        plan.waves[0].pass,
        CudaEGraphUnionCompactionPass::UnionPairs
    );
    assert_eq!(plan.waves[0].item_count, 8);
    assert_eq!(plan.waves[1].item_count, 8);
    assert_eq!(plan.waves[2].item_count, 1);
    assert_eq!(
        plan.waves[3].pass,
        CudaEGraphUnionCompactionPass::CanonicalRewrites
    );
    assert_eq!(plan.waves[3].item_count, 8);
    assert_eq!(plan.waves[4].item_count, 8);
    assert_eq!(plan.waves[5].item_count, 1);
}

#[test]
fn egraph_union_compaction_plan_rejects_zero_launch_dimensions() {
    let pair = [Equivalence { left: 1, right: 2 }];
    use vyre_driver_cuda::egraph_kernel_plan::CudaEGraphKernelPlanError;
    assert_eq!(
        plan_cuda_egraph_union_compaction(
            &pair,
            CudaEGraphKernelLaunchConfig {
                threads_per_block: 0,
                max_blocks_per_launch: 1,
            },
        )
        .unwrap_err(),
        CudaEGraphKernelPlanError::ZeroThreadsPerBlock
    );
    assert_eq!(
        plan_cuda_egraph_union_compaction(
            &pair,
            CudaEGraphKernelLaunchConfig {
                threads_per_block: 1,
                max_blocks_per_launch: 0,
            },
        )
        .unwrap_err(),
        CudaEGraphKernelPlanError::ZeroMaxBlocksPerLaunch
    );
}

#[test]
fn egraph_union_compaction_plan_fast_paths_empty_batches() {
    let plan = plan_cuda_egraph_union_compaction(
        &[],
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 4,
            max_blocks_per_launch: 2,
        },
    )
    .expect("Fix: CUDA e-graph union compaction must accept empty convergence batches.");

    assert!(plan.canonical_pairs.is_empty());
    assert!(plan.affected_eclasses.is_empty());
    assert!(plan.canonical_rewrites.is_empty());
    assert!(plan.waves.is_empty());
    assert_eq!(plan.ignored_self_pair_count, 0);
    assert_eq!(plan.duplicate_pair_count, 0);
    assert_eq!(plan.total_items, 0);
    assert_eq!(plan.total_blocks, 0);

    let self_pair_only = plan_cuda_egraph_union_compaction(
        &[Equivalence { left: 7, right: 7 }],
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 4,
            max_blocks_per_launch: 2,
        },
    )
    .expect("Fix: CUDA e-graph union compaction must accept self-pair-only convergence batches.");

    assert!(self_pair_only.canonical_pairs.is_empty());
    assert!(self_pair_only.affected_eclasses.is_empty());
    assert!(self_pair_only.canonical_rewrites.is_empty());
    assert!(self_pair_only.waves.is_empty());
    assert_eq!(self_pair_only.ignored_self_pair_count, 1);
    assert_eq!(self_pair_only.duplicate_pair_count, 0);
    assert_eq!(self_pair_only.total_items, 0);
    assert_eq!(self_pair_only.total_blocks, 0);
}

#[test]
fn egraph_union_compaction_plan_handles_generated_adversarial_batches() {
    let config = CudaEGraphKernelLaunchConfig {
        threads_per_block: 7,
        max_blocks_per_launch: 3,
    };
    let max_items_per_wave = u64::from(config.threads_per_block * config.max_blocks_per_launch);
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;

    for case_index in 0..4096_u32 {
        let class_count = 4 + (next_u32(&mut seed) % 37);
        let base = case_index
            .checked_mul(1000)
            .expect("Fix: generated e-graph case base must fit u32.");
        let edge_count = 2 * class_count + (next_u32(&mut seed) % class_count);
        let mut pairs = Vec::new();
        for edge_index in 0..edge_count {
            let left = base + (next_u32(&mut seed) % class_count);
            let right = base + (next_u32(&mut seed) % class_count);
            pairs.push(Equivalence { left, right });
            if edge_index % 3 == 0 {
                pairs.push(Equivalence {
                    left: right,
                    right: left,
                });
            }
            if edge_index % 5 == 0 {
                pairs.push(Equivalence { left, right: left });
            }
        }

        let plan = plan_cuda_egraph_union_compaction(&pairs, config)
            .expect("Fix: generated hostile e-graph union batches must remain plannable.");

        assert_sorted_unique_pairs(&plan);
        assert_rewrites_are_final_representatives(&plan);
        assert_wave_coverage(&plan, max_items_per_wave);
    }
}

