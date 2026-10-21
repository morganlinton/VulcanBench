use std::sync::Arc;

use super::scallop_join_wide::{
    cpu_ref, cpu_ref_into, scallop_join_wide, scallop_join_wide_dispatch_grid,
};
use vyre_foundation::ir::Node;
use vyre_foundation::MemoryOrdering;

#[test]
fn cpu_ref_1x1_trivial() {
    let n = 1;
    let w = 1;
    let state = vec![0b01];
    let join_rules = vec![0b10];
    let (final_state, iters) = cpu_ref(&state, &join_rules, n, w, 10);

    assert_eq!(final_state, vec![0b11]);
    assert_eq!(iters, 1);
}

#[test]
fn cpu_ref_no_new_derivations() {
    let n = 2;
    let w = 2;
    let state = vec![0, 0, 0b01, 0, 0, 0, 0, 0];
    let join_rules = vec![0; 8];
    let (final_state, iters) = cpu_ref(&state, &join_rules, n, w, 10);

    assert_eq!(final_state, state);
    assert_eq!(iters, 0);
}

#[test]
#[should_panic(expected = "complete n*n*w state matrix")]
fn cpu_ref_short_inputs_fail_loudly() {
    let _ = cpu_ref(&[0b01], &[], 1, 2, 10);
}

#[test]
fn cpu_ref_transitive_3_nodes() {
    let n = 3;
    let w = 1;
    let mut state = vec![0; 9];
    state[1] = 0b001;
    let mut join_rules = vec![0; 9];
    join_rules[5] = 0b010;
    let (final_state, _) = cpu_ref(&state, &join_rules, n, w, 10);

    assert_eq!(final_state[2], 0b011);
}

#[test]
fn cpu_ref_wide_multi_word() {
    let n = 2;
    let w = 4;
    let mut state = vec![0; 16];
    state[6] = 0x1;
    let mut join_rules = vec![0; 16];
    join_rules[15] = 0x2;
    let (final_state, _) = cpu_ref(&state, &join_rules, n, w, 10);

    assert_eq!(final_state[6], 0x1);
    assert_eq!(final_state[7], 0x2);
}

#[test]
fn cpu_ref_into_reuses_wide_buffers_and_truncates_stale_tail() {
    let n = 2;
    let w = 2;
    let mut state = vec![0; 8];
    state[2] = 0b01;
    let mut join_rules = vec![0; 8];
    join_rules[7] = 0b10;
    let mut current = Vec::with_capacity(16);
    let mut next = Vec::with_capacity(16);
    current.extend_from_slice(&[99; 12]);
    next.extend_from_slice(&[77; 12]);
    let current_capacity = current.capacity();
    let next_capacity = next.capacity();

    let iters = cpu_ref_into(&state, &join_rules, n, w, 4, &mut current, &mut next);

    assert!(iters <= 4);
    assert_eq!(current, vec![0, 0, 0b01, 0b10, 0, 0, 0, 0]);
    assert_eq!(current.capacity(), current_capacity);
    assert_eq!(next.capacity(), next_capacity);

    let iters = cpu_ref_into(&[0b01], &[0b10], 1, 1, 10, &mut current, &mut next);
    assert_eq!(iters, 1);
    assert_eq!(current, vec![0b11]);
    assert_eq!(next, vec![0b11]);
    assert_eq!(current.capacity(), current_capacity);
    assert_eq!(next.capacity(), next_capacity);
}

#[test]
fn reference_parity_2x2_2w() {
    let n = 2;
    let w = 2;
    let mut state_init = vec![0; 8];
    state_init[2] = 0b01;
    let mut join_rules = vec![0; 8];
    join_rules[7] = 0b10;

    let p = scallop_join_wide("s", "nx", "j", "c", n, w, 4);
    let (expected_state, _) = cpu_ref(&state_init, &join_rules, n, w, 4);

    use vyre_reference::reference_eval;
    use vyre_reference::value::Value;

    let to_value = |data: &[u32]| {
        let bytes = crate::wire::pack_u32_slice(data);
        Value::Bytes(Arc::from(bytes))
    };

    let inputs = vec![
        to_value(&state_init),
        to_value(&[0_u32; 8]),
        to_value(&[0]),
        to_value(&join_rules),
    ];

    let results = reference_eval(&p, &inputs).expect("Fix: interpreter failed");
    let actual_bytes = results[0].to_bytes();
    let actual_state: Vec<u32> = actual_bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();

    assert_eq!(actual_state, expected_state);
}

#[test]
fn dispatch_grid_scales_large_wide_relations_into_blocks() {
    assert_eq!(scallop_join_wide_dispatch_grid(0, 2), [1, 1, 1]);
    assert_eq!(scallop_join_wide_dispatch_grid(1, 1), [1, 1, 1]);
    assert_eq!(scallop_join_wide_dispatch_grid(16, 1), [1, 1, 1]);
    assert_eq!(scallop_join_wide_dispatch_grid(17, 1), [2, 1, 1]);
    assert_eq!(scallop_join_wide_dispatch_grid(17, 2), [2, 1, 1]);
    assert_eq!(scallop_join_wide_dispatch_grid(33, 2), [5, 1, 1]);
}

#[test]
fn large_program_uses_split_visible_grid_sync() {
    let p = scallop_join_wide("s", "nx", "j", "c", 17, 2, 4);
    assert_eq!(count_grid_sync(p.entry()), 7);
}

fn count_grid_sync(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            Node::Barrier {
                ordering: MemoryOrdering::GridSync,
            } => 1,
            Node::If {
                then, otherwise, ..
            } => count_grid_sync(then) + count_grid_sync(otherwise),
            Node::Loop { body, .. } | Node::Block(body) => count_grid_sync(body),
            Node::Region { body, .. } => count_grid_sync(body),
            _ => 0,
        })
        .sum()
}
