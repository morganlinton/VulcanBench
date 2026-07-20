use super::*;

#[test]
fn scc_intersection_stamps_pivot() {
    let out = scc_cpu_ref(4, &[0b0011], &[0b0011], &[u32::MAX; 4], 0);
    assert_eq!(&out[0..2], &[0, 0]);
    assert_eq!(&out[2..4], &[u32::MAX, u32::MAX]);
}

#[test]
fn scc_disjoint_forward_backward_yields_no_change() {
    let comp_in = vec![u32::MAX; 4];
    let out = scc_cpu_ref(4, &[0b0001], &[0b1000], &comp_in, 0);
    assert_eq!(out, comp_in);
}

#[test]
fn scc_first_pivot_wins() {
    let comp_in = vec![u32::MAX; 4];
    let forward = vec![0b1111];
    let backward = vec![0b1111];
    let after_first = scc_cpu_ref(4, &forward, &backward, &comp_in, 5);
    assert_eq!(after_first, vec![5, 5, 5, 5]);
    let after_second = scc_cpu_ref(4, &forward, &backward, &after_first, 9);
    assert_eq!(
        after_second,
        vec![5, 5, 5, 5],
        "second pivot must not overwrite"
    );
}

#[test]
fn scc_unassigned_node_gets_second_pivot() {
    let comp_in = vec![u32::MAX; 4];
    let after_first = scc_cpu_ref(4, &[0b0001], &[0b0001], &comp_in, 5);
    assert_eq!(after_first[0], 5);
    assert_eq!(after_first[2], u32::MAX);
    let after_second = scc_cpu_ref(4, &[0b0100], &[0b0100], &after_first, 9);
    assert_eq!(after_second[0], 5);
    assert_eq!(after_second[2], 9);
}

#[test]
fn scc_empty_intersection_all_unassigned() {
    let comp_in = vec![u32::MAX; 4];
    let out = scc_cpu_ref(4, &[0b0001], &[0b0010], &comp_in, 0);
    assert_eq!(out, comp_in);
}

// ---------------------------------------------------------------------------
// Path reconstruction
// ---------------------------------------------------------------------------

#[test]
fn path_reconstruct_walks_to_root() {
    let mut scratch = Vec::with_capacity(8);
    let len = path_cpu_ref(&[0, 0, 1, 2], 3, 4, &mut scratch);
    assert_eq!(len, 4);
    assert_eq!(&scratch[..4], &[3, 2, 1, 0]);
}

#[test]
fn path_reconstruct_max_depth_caps() {
    let mut scratch = Vec::with_capacity(8);
    let len = path_cpu_ref(&[1, 0], 0, 8, &mut scratch);
    assert_eq!(len, 8);
    assert_eq!(&scratch[..], &[0, 1, 0, 1, 0, 1, 0, 1]);
}

#[test]
fn path_reconstruct_tail_zero_padded() {
    let mut scratch = Vec::with_capacity(8);
    let len = path_cpu_ref(&[0, 0, 1, 2], 3, 8, &mut scratch);
    assert_eq!(len, 4);
    assert_eq!(&scratch[..4], &[3, 2, 1, 0]);
    assert_eq!(&scratch[4..], &[0, 0, 0, 0]);
}

#[test]
fn path_reconstruct_single_node() {
    let mut scratch = Vec::with_capacity(4);
    let len = path_cpu_ref(&[0], 0, 4, &mut scratch);
    assert_eq!(len, 1);
    assert_eq!(scratch[0], 0);
    assert_eq!(&scratch[1..], &[0, 0, 0]);
}

#[test]
fn path_reconstruct_oob_parent_terminates_early() {
    let mut scratch = Vec::with_capacity(4);
    // parent[3] is OOB, so when current=3, next = unwrap_or(current) = 3,
    // which equals current → break.
    let len = path_cpu_ref(&[0, 0, 1], 3, 4, &mut scratch);
    assert_eq!(len, 1);
    assert_eq!(scratch[0], 3);
}

#[test]
fn path_reconstruct_zero_max_depth_emits_trap_program() {
    // Primitive builders are infallible: invalid shapes become IR traps,
    // not host panics. Verify max_depth == 0 produces a trap node.
    let p = vyre_primitives::graph::path_reconstruct::path_reconstruct(
        "parent", "target", "out", "len", 0,
    );
    let entry = p.entry();
    let has_trap = entry.iter().any(|n| {
        use vyre_foundation::ir::Node;
        if let Node::Region { body, .. } = n {
            body.iter().any(|inner| matches!(inner, Node::Trap { .. }))
        } else {
            matches!(n, Node::Trap { .. })
        }
    });
    assert!(
        has_trap,
        "max_depth == 0 must produce a trap program, not panic"
    );
}

// ---------------------------------------------------------------------------
// Tensor SCC (bounded bit-matrix fixpoint)
// ---------------------------------------------------------------------------

#[test]
fn tensor_scc_closes_cycle_inside_group() {
    let rows = [0b0010u32, 0b0100, 0b0001, 0b1000];
    assert_eq!(tensor_scc_cpu_ref(&rows, 0b0001, 0b0111, 8), 0b0111);
}

#[test]
fn tensor_scc_masks_edges_outside_group() {
    let rows = [0b1010u32, 0b0100, 0b0000, 0b0001];
    assert_eq!(tensor_scc_cpu_ref(&rows, 0b0001, 0b0011, 8), 0b0011);
}

#[test]
fn tensor_scc_no_edges_isolated() {
    let rows = [0u32; 4];
    assert_eq!(tensor_scc_cpu_ref(&rows, 0b0001, 0b1111, 8), 0b0001);
}

#[test]
fn tensor_scc_converges_before_limit() {
    let rows = [0b0010u32, 0b0100, 0b0001];
    // Cycle 0->1->2->0; starting from 0b0001 with group=0b0111
    // Iter 0: active=0b0001, next adds row0=0b0010 -> 0b0011
    // Iter 1: active=0b0011, next adds row0+row1 -> 0b0011 | 0b0110 = 0b0111
    // Iter 2: active=0b0111, next adds row0+row1+row2 -> already stable
    // Should converge in 2 iters, but we give 100.
    assert_eq!(tensor_scc_cpu_ref(&rows, 0b0001, 0b0111, 100), 0b0111);
}

#[test]
fn tensor_scc_group_mask_zero_annihilates() {
    let rows = [0b1111u32; 4];
    assert_eq!(tensor_scc_cpu_ref(&rows, 0b1111, 0b0000, 8), 0b0000);
}

#[test]
fn tensor_scc_seed_outside_group_is_masked() {
    let rows = [0b1111u32; 4];
    assert_eq!(tensor_scc_cpu_ref(&rows, 0b1000, 0b0111, 8), 0b0000);
}

#[test]
fn tensor_scc_program_buffer_counts() {
    let p = tensor_scc_fixpoint("rows", "seed", "group", "out", 4, 8);
    assert_eq!(p.workgroup_size(), [1, 1, 1]);
    assert_eq!(p.buffers()[0].count(), 4);
    assert_eq!(p.buffers()[3].count(), 1);
}

// ---------------------------------------------------------------------------
// CSR forward-or-changed (in-place expansion with sticky flag)
// ---------------------------------------------------------------------------
