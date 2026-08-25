use super::*;

#[test]
fn reusable_layout_validation_rejects_bad_csr_and_frontier() {
    let err = validate_persistent_bfs_graph_layout(2, &[0, 2, 1], &[1], &[1]).unwrap_err();
    assert!(err.contains("final CSR offset") || err.contains("non-monotonic"));

    let err = validate_persistent_bfs_graph_layout(2, &[0, 1, 1], &[2], &[1]).unwrap_err();
    assert!(err.contains("outside node_count"));

    let err = validate_persistent_bfs_inputs(33, &[0; 34], &[], &[], &[0]).unwrap_err();
    assert!(err.contains("frontier length 2 words"));
}

#[test]
fn reusable_graph_layout_returns_dispatch_shape() {
    assert_eq!(
        validate_persistent_bfs_graph_layout(33, &[0; 34], &[], &[]).unwrap(),
        PersistentBfsLayout {
            node_count: 33,
            edge_count: 0,
            words: 2,
            words_u32: 2,
            node_words: 33,
            edge_storage_words: 1,
        }
    );
    assert_eq!(
        validate_persistent_bfs_inputs(4, &[0, 1, 2, 3, 3], &[1, 2, 3], &[1, 1, 1], &[0]).unwrap(),
        PersistentBfsLayout {
            node_count: 4,
            edge_count: 3,
            words: 1,
            words_u32: 1,
            node_words: 4,
            edge_storage_words: 3,
        }
    );
}

#[test]
fn dispatch_plans_pin_grid_cache_shape_and_program_builders() {
    let edge_offsets = [0, 1, 2, 3, 3];
    let edge_targets = [1, 2, 3];
    let edge_kind_mask = [1, 1, 1];
    let plan = plan_persistent_bfs_dispatch(
        4,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &[0b0001],
        0xFFFF_FFFF,
        8,
    )
    .expect("Fix: canonical persistent-BFS dispatch plan should validate");

    assert_eq!(plan.layout().node_count, 4);
    assert_eq!(plan.layout().edge_count, 3);
    assert_eq!(plan.frontier_words(), 1);
    assert_eq!(plan.node_words(), 4);
    assert_eq!(plan.edge_storage_words(), 3);
    assert_eq!(plan.dispatch_grid(), persistent_bfs_single_dispatch_grid(4));
    assert_eq!(
        plan.layout_hash(),
        persistent_bfs_layout_hash(4, &edge_offsets, &edge_targets, &edge_kind_mask)
    );
    assert_eq!(
        plan.cache_key(0xCAFE),
        PersistentBfsPlanCacheKey {
            layout_hash: plan.layout_hash(),
            node_count: 4,
            edge_count: 3,
            words_per_query: 1,
            query_count: 1,
            allow_mask: 0xFFFF_FFFF,
            max_iters: 8,
            device_features: 0xCAFE,
            kind: PersistentBfsPlanCacheKind::Single,
        }
    );
    assert_eq!(
        plan.program_cache_key(0xCAFE),
        PersistentBfsPlanCacheKey {
            layout_hash: persistent_bfs_program_layout_hash(
                4,
                3,
                1,
                1,
                PersistentBfsPlanCacheKind::Single,
            ),
            node_count: 4,
            edge_count: 3,
            words_per_query: 1,
            query_count: 1,
            allow_mask: 0xFFFF_FFFF,
            max_iters: 8,
            device_features: 0xCAFE,
            kind: PersistentBfsPlanCacheKind::Single,
        }
    );
    assert_eq!(
        plan.program("frontier_in", "frontier_out").workgroup_size,
        PERSISTENT_BFS_WORKGROUP_SIZE
    );

    let empty_edge_plan =
        plan_persistent_bfs_dispatch(2, &[0, 0, 0], &[], &[], &[0], 0xFFFF_FFFF, 1)
            .expect("Fix: zero-edge persistent-BFS graph is a valid dispatch shape");
    assert_eq!(empty_edge_plan.layout().edge_count, 0);
    assert_eq!(empty_edge_plan.edge_storage_words(), 1);
    assert_eq!(
        empty_edge_plan
            .program("frontier_in", "frontier_out")
            .workgroup_size,
        PERSISTENT_BFS_WORKGROUP_SIZE
    );

    let resident = plan_persistent_bfs_resident_dispatch(4, 3, 1, &[0b0001], 0xFF, 4)
        .expect("Fix: resident single-frontier plan should validate");
    assert_eq!(resident.frontier_words(), 1);
    assert_eq!(resident.words_u32(), 1);
    assert_eq!(
        resident.dispatch_grid(),
        persistent_bfs_single_dispatch_grid(4)
    );
    assert_eq!(
        resident.cache_key(0xABCD, 0x10),
        PersistentBfsPlanCacheKey {
            layout_hash: 0xABCD,
            node_count: 4,
            edge_count: 3,
            words_per_query: 1,
            query_count: 1,
            allow_mask: 0xFF,
            max_iters: 4,
            device_features: 0x10,
            kind: PersistentBfsPlanCacheKind::Single,
        }
    );
    assert_eq!(
        resident.program_cache_key(0x10),
        PersistentBfsPlanCacheKey {
            layout_hash: persistent_bfs_program_layout_hash(
                4,
                3,
                1,
                1,
                PersistentBfsPlanCacheKind::Single,
            ),
            node_count: 4,
            edge_count: 3,
            words_per_query: 1,
            query_count: 1,
            allow_mask: 0xFF,
            max_iters: 4,
            device_features: 0x10,
            kind: PersistentBfsPlanCacheKind::Single,
        }
    );

    let batch = plan_persistent_bfs_resident_batch_dispatch(4, 3, 1, &[1, 2], 2, 0xFF, 4)
        .expect("Fix: resident batch plan should validate");
    assert_eq!(batch.query_count(), 2);
    assert_eq!(batch.query_count_u32(), 2);
    assert_eq!(batch.total_words(), 2);
    assert_eq!(batch.words_per_query(), 1);
    assert_eq!(batch.dispatch_grid(), [1, 2, 1]);
    assert_eq!(
        batch
            .program("frontier_in", "frontier_out", "changed")
            .workgroup_size,
        PERSISTENT_BFS_WORKGROUP_SIZE
    );
    assert_eq!(
        batch.cache_key(0xABCD, 0x20),
        PersistentBfsPlanCacheKey {
            layout_hash: 0xABCD,
            node_count: 4,
            edge_count: 3,
            words_per_query: 1,
            query_count: 2,
            allow_mask: 0xFF,
            max_iters: 4,
            device_features: 0x20,
            kind: PersistentBfsPlanCacheKind::Batch,
        }
    );
    assert_eq!(
        batch.program_cache_key(0x20),
        PersistentBfsPlanCacheKey {
            layout_hash: persistent_bfs_program_layout_hash(
                4,
                3,
                1,
                2,
                PersistentBfsPlanCacheKind::Batch,
            ),
            node_count: 4,
            edge_count: 3,
            words_per_query: 1,
            query_count: 2,
            allow_mask: 0xFF,
            max_iters: 4,
            device_features: 0x20,
            kind: PersistentBfsPlanCacheKind::Batch,
        }
    );
}

#[test]
fn large_dispatch_plans_cover_every_node_with_parallel_grid() {
    let node_count = 513u32;
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::with_capacity(node_count as usize - 1);
    let mut edge_kind_mask = Vec::with_capacity(node_count as usize - 1);
    edge_offsets.push(0);
    for src in 0..node_count {
        if src + 1 < node_count {
            edge_targets.push(src + 1);
            edge_kind_mask.push(1);
        }
        edge_offsets.push(edge_targets.len() as u32);
    }
    let seed = vec![1u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]; // 513 bits.
    let plan = plan_persistent_bfs_dispatch(
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &seed,
        0xFFFF_FFFF,
        node_count,
    )
    .expect("Fix: large persistent-BFS chain should plan");

    assert_eq!(plan.dispatch_grid(), [3, 1, 1]);
    assert_eq!(
        plan.program("frontier_in", "frontier_out").workgroup_size,
        PERSISTENT_BFS_WORKGROUP_SIZE
    );

    let resident = plan_persistent_bfs_resident_dispatch(
        node_count,
        edge_targets.len() as u32,
        seed.len(),
        &seed,
        0xFFFF_FFFF,
        node_count,
    )
    .expect("Fix: large resident persistent-BFS chain should plan");
    assert_eq!(resident.dispatch_grid(), [3, 1, 1]);

    let batch_seed = vec![0u32; seed.len() * 3];
    let resident_batch = plan_persistent_bfs_resident_batch_dispatch(
        node_count,
        edge_targets.len() as u32,
        seed.len(),
        &batch_seed,
        3,
        0xFFFF_FFFF,
        node_count,
    )
    .expect("Fix: large resident persistent-BFS batch should plan");
    assert_eq!(resident_batch.dispatch_grid(), [3, 3, 1]);
}
