use super::*;

#[test]
fn cuda_adaptive_dense_step_chain() {
    let backend = live_dispatcher();
    // 4 nodes; reverse adjacency (row d = predecessors of d):
    // node 0 ← {3}, node 1 ← {0}, node 2 ← {1}, node 3 ← {2}.
    let node_count = 4u32;
    let words = bitset_words(node_count) as usize;
    let mut adj = vec![0u32; node_count as usize * words];
    let mut set_pred = |dst: u32, src: u32| {
        adj[(dst as usize) * words + (src as usize / 32)] |= 1u32 << (src & 31);
    };
    set_pred(0, 3);
    set_pred(1, 0);
    set_pred(2, 1);
    set_pred(3, 2);
    let frontier_in = vec![0b0001u32]; // {0}
    let cpu = cpu_dense_step(&frontier_in, &adj, node_count);
    let gpu = run_dense_step(&backend, &frontier_in, &adj, node_count);
    assert_eq!(gpu, cpu);
    // {0} reaches {1} via reverse-adj rows.
    assert_eq!(gpu, vec![0b0010u32]);
}

#[test]
fn cuda_adaptive_dense_step_empty_frontier() {
    let backend = live_dispatcher();
    let node_count = 4u32;
    let words = bitset_words(node_count) as usize;
    let mut adj = vec![0u32; node_count as usize * words];
    adj[0] = 0b0010; // node 0 ← {1}
    let frontier_in = vec![0u32];
    let cpu = cpu_dense_step(&frontier_in, &adj, node_count);
    let gpu = run_dense_step(&backend, &frontier_in, &adj, node_count);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0u32]);
}

#[test]
fn cuda_adaptive_dense_step_full_frontier_reaches_all_with_any_pred() {
    let backend = live_dispatcher();
    let node_count = 4u32;
    let words = bitset_words(node_count) as usize;
    let mut adj = vec![0u32; node_count as usize * words];
    // Nodes 0 and 1 have predecessor 0; nodes 2 and 3 have no preds.
    adj[0] = 0b0001;
    adj[words] = 0b0001;
    let frontier_in = vec![0b1111u32];
    let cpu = cpu_dense_step(&frontier_in, &adj, node_count);
    let gpu = run_dense_step(&backend, &frontier_in, &adj, node_count);
    assert_eq!(gpu, cpu);
    // Only nodes 0 and 1 see a hit because nodes 2,3 have no preds.
    assert_eq!(gpu, vec![0b0011u32]);
}

#[test]
fn cuda_adaptive_dense_step_covers_node_past_first_workgroup() {
    let backend = live_dispatcher();
    let node_count = 513u32;
    let adj = build_dense_adj(&[(300, 512), (301, 400)], node_count);
    let frontier_in = pack_nodes(&[300], node_count);

    let cpu = cpu_dense_step(&frontier_in, &adj, node_count);
    let gpu = run_dense_step(&backend, &frontier_in, &adj, node_count);

    assert_eq!(gpu, cpu);
    assert_eq!(gpu, pack_nodes(&[512], node_count));
}

#[test]
fn cuda_adaptive_sparse_dense_sparse_branch_uses_csr_from_gpu_popcount() {
    let backend = live_dispatcher();
    let node_count = 8u32;
    let frontier_in = pack_nodes(&[0], node_count);
    let count_bytes = run_reduce_count(&backend, &frontier_in);
    let selector_count = bytes_u32(&count_bytes)[0];
    let edge_offsets = vec![0, 1, 1, 1, 1, 1, 1, 1, 1];
    let edge_targets = vec![1];
    let edge_kind_mask = vec![1];
    let adj = build_dense_adj(&[(0, 2)], node_count);

    let cpu = cpu_sparse_dense_step(
        &frontier_in,
        selector_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
        node_count,
        1,
        50,
    );
    let gpu = run_sparse_dense_step(
        &backend,
        &frontier_in,
        count_bytes,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
        node_count,
        50,
    );
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, pack_nodes(&[1], node_count));
}

#[test]
fn cuda_adaptive_sparse_dense_dense_branch_uses_rows_from_gpu_popcount() {
    let backend = live_dispatcher();
    let node_count = 8u32;
    let frontier_in = pack_nodes(&[0, 1, 2, 3], node_count);
    let count_bytes = run_reduce_count(&backend, &frontier_in);
    let selector_count = bytes_u32(&count_bytes)[0];
    let edge_offsets = vec![0, 1, 1, 1, 1, 1, 1, 1, 1];
    let edge_targets = vec![1];
    let edge_kind_mask = vec![1];
    let adj = build_dense_adj(&[(0, 5)], node_count);

    let cpu = cpu_sparse_dense_step(
        &frontier_in,
        selector_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
        node_count,
        1,
        50,
    );
    let gpu = run_sparse_dense_step(
        &backend,
        &frontier_in,
        count_bytes,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
        node_count,
        50,
    );
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, pack_nodes(&[5], node_count));
}

#[test]
fn cuda_adaptive_sparse_dense_sparse_branch_covers_source_past_first_workgroup() {
    let backend = live_dispatcher();
    let node_count = 513u32;
    let frontier_in = pack_nodes(&[300], node_count);
    let count_bytes = run_reduce_count(&backend, &frontier_in);
    let selector_count = bytes_u32(&count_bytes)[0];
    let mut edge_offsets = vec![0u32; node_count as usize + 1];
    for src in 301..=node_count {
        edge_offsets[src as usize] = 1;
    }
    let edge_targets = vec![512];
    let edge_kind_mask = vec![1];
    let adj = build_dense_adj(&[], node_count);

    let cpu = cpu_sparse_dense_step(
        &frontier_in,
        selector_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
        node_count,
        1,
        100,
    );
    let gpu = run_sparse_dense_step(
        &backend,
        &frontier_in,
        count_bytes,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
        node_count,
        100,
    );

    assert_eq!(gpu, cpu);
    assert_eq!(gpu, pack_nodes(&[512], node_count));
}

#[test]
fn cuda_adaptive_sparse_dense_dense_branch_covers_node_past_first_workgroup() {
    let backend = live_dispatcher();
    let node_count = 513u32;
    let frontier_in = pack_nodes(&[300], node_count);
    let count_bytes = run_reduce_count(&backend, &frontier_in);
    let selector_count = bytes_u32(&count_bytes)[0];
    let edge_offsets = vec![0u32; node_count as usize + 1];
    let edge_targets = vec![0];
    let edge_kind_mask = vec![0];
    let adj = build_dense_adj(&[(300, 512), (301, 400)], node_count);

    let cpu = cpu_sparse_dense_step(
        &frontier_in,
        selector_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
        node_count,
        1,
        0,
    );
    let gpu = run_sparse_dense_step(
        &backend,
        &frontier_in,
        count_bytes,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
        node_count,
        0,
    );

    assert_eq!(gpu, cpu);
    assert_eq!(gpu, pack_nodes(&[512], node_count));
}

