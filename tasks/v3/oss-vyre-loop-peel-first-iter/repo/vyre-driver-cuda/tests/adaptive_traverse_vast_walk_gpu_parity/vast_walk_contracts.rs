use super::*;

#[test]
fn cuda_vast_walk_preorder_balanced_tree() {
    let backend = live_dispatcher();
    // Tree:
    //       0
    //      / \
    //     1   2
    //    / \
    //   3   4
    let nodes = vec![
        make_node(SENTINEL, 1, SENTINEL), // 0: root
        make_node(0, 3, 2),               // 1: child of 0, sibling 2
        make_node(0, SENTINEL, SENTINEL), // 2: child of 0
        make_node(1, SENTINEL, 4),        // 3: child of 1, sibling 4
        make_node(1, SENTINEL, SENTINEL), // 4: child of 1
    ];
    let node_count = nodes.len() as u32;
    let (node_bytes, node_words) = pack_vast(&nodes);
    let cpu = walk_preorder_indices(&node_bytes, node_count, 64).expect("walk ok");
    // Read only the first cpu.len() entries of the GPU output (out_cap is the buffer size).
    let gpu_full = run_preorder(&backend, &node_words, node_count, node_count);
    let gpu_emitted: Vec<u32> = gpu_full.iter().take(cpu.len()).copied().collect();
    assert_eq!(gpu_emitted, cpu);
    assert_eq!(gpu_emitted, vec![0, 1, 3, 4, 2]);
}

#[test]
fn cuda_vast_walk_preorder_single_node() {
    let backend = live_dispatcher();
    let nodes = vec![make_node(SENTINEL, SENTINEL, SENTINEL)];
    let (node_bytes, node_words) = pack_vast(&nodes);
    let cpu = walk_preorder_indices(&node_bytes, 1, 64).expect("walk ok");
    let gpu_full = run_preorder(&backend, &node_words, 1, 1);
    let gpu_emitted: Vec<u32> = gpu_full.iter().take(cpu.len()).copied().collect();
    assert_eq!(gpu_emitted, cpu);
    assert_eq!(gpu_emitted, vec![0]);
}

#[test]
fn cuda_vast_walk_preorder_linear_chain() {
    let backend = live_dispatcher();
    // 0 -> 1 -> 2 -> 3 (each first_child links to the next).
    let nodes = vec![
        make_node(SENTINEL, 1, SENTINEL),
        make_node(0, 2, SENTINEL),
        make_node(1, 3, SENTINEL),
        make_node(2, SENTINEL, SENTINEL),
    ];
    let (node_bytes, node_words) = pack_vast(&nodes);
    let cpu = walk_preorder_indices(&node_bytes, 4, 64).expect("walk ok");
    let gpu_full = run_preorder(&backend, &node_words, 4, 4);
    let gpu_emitted: Vec<u32> = gpu_full.iter().take(cpu.len()).copied().collect();
    assert_eq!(gpu_emitted, cpu);
    assert_eq!(gpu_emitted, vec![0, 1, 2, 3]);
}
