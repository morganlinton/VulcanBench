//! Generated adversarial graph-fixpoint tests for CSR traversal and persistent BFS.

#![cfg(all(feature = "graph", feature = "bitset", feature = "cpu-parity"))]

use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::csr_forward_or_changed;
use vyre_primitives::graph::persistent_bfs;

fn next_u32(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

fn generated_graph(seed: u32) -> (u32, Vec<u32>, Vec<u32>, Vec<u32>) {
    let node_count = 1 + (seed % 97);
    let mut state = seed ^ 0xC0FF_EE11;
    let mut offsets = Vec::with_capacity(node_count as usize + 1);
    let mut targets = Vec::new();
    let mut masks = Vec::new();
    offsets.push(0);
    for src in 0..node_count {
        let degree = (next_u32(&mut state) % 4) as usize;
        for edge_index in 0..degree {
            let raw = next_u32(&mut state);
            let target = raw.wrapping_add(src.rotate_left((edge_index % 31) as u32)) % node_count;
            let kind = 1u32 << ((raw >> 11) % 4);
            targets.push(target);
            masks.push(kind);
        }
        offsets.push(targets.len() as u32);
    }
    (node_count, offsets, targets, masks)
}

fn generated_frontier(seed: u32, node_count: u32) -> Vec<u32> {
    let words = bitset_words(node_count) as usize;
    let mut state = seed ^ 0x5151_9EED;
    let mut frontier = vec![0u32; words.max(1)];
    for node in 0..node_count {
        if next_u32(&mut state) & 0b111 == 0 {
            frontier[(node / 32) as usize] |= 1u32 << (node % 32);
        }
    }
    if frontier.iter().all(|&word| word == 0) {
        let node = seed % node_count;
        frontier[(node / 32) as usize] |= 1u32 << (node % 32);
    }
    if node_count % 32 != 0 {
        let valid_bits = node_count % 32;
        let mask = (1u32 << valid_bits) - 1;
        let last = frontier.len() - 1;
        frontier[last] &= mask;
    }
    frontier
}

fn allow_mask(seed: u32) -> u32 {
    match seed % 7 {
        0 => 0,
        1 => 0b0001,
        2 => 0b0010,
        3 => 0b0100,
        4 => 0b1000,
        5 => 0b0101,
        _ => 0xFFFF_FFFF,
    }
}

#[test]
fn persistent_bfs_matches_csr_forward_closure_for_generated_graphs() {
    for seed in 0..8192u32 {
        let (node_count, offsets, targets, masks) = generated_graph(seed);
        let frontier = generated_frontier(seed, node_count);
        let allow = allow_mask(seed);
        let max_iters = node_count.saturating_add(2);

        let via_csr = csr_forward_or_changed::cpu_ref_closure(
            node_count, &offsets, &targets, &masks, &frontier, allow, max_iters,
        );
        let (via_bfs, _changed) = persistent_bfs::cpu_ref(
            node_count, &offsets, &targets, &masks, &frontier, allow, max_iters,
        );

        assert_eq!(via_bfs, via_csr, "seed {seed}");
    }
}

#[test]
fn persistent_bfs_generated_fixpoints_are_idempotent() {
    for seed in 8192..12_288u32 {
        let (node_count, offsets, targets, masks) = generated_graph(seed);
        let frontier = generated_frontier(seed, node_count);
        let allow = allow_mask(seed);
        let max_iters = node_count.saturating_add(2);

        let (closure, _first_changed) = persistent_bfs::cpu_ref(
            node_count, &offsets, &targets, &masks, &frontier, allow, max_iters,
        );
        let (closure_again, second_changed) = persistent_bfs::cpu_ref(
            node_count, &offsets, &targets, &masks, &closure, allow, max_iters,
        );

        assert_eq!(closure_again, closure, "idempotent closure seed {seed}");
        assert_eq!(
            second_changed, 0,
            "fixpoint must report no new bits at seed {seed}"
        );
    }
}

#[test]
fn generated_validation_rejects_corrupted_csr_shapes() {
    for seed in 12_288..14_336u32 {
        let (node_count, mut offsets, mut targets, mut masks) = generated_graph(seed);
        match seed % 4 {
            0 => {
                offsets.pop();
            }
            1 if offsets.len() > 2 => {
                offsets[1] = u32::MAX;
                offsets[2] = 0;
            }
            2 => {
                targets.push(node_count);
                if let Some(last) = offsets.last_mut() {
                    *last = targets.len() as u32;
                }
                masks.push(1);
            }
            _ => {
                masks.push(1);
            }
        }

        let err = persistent_bfs::validate_persistent_bfs_graph_layout(
            node_count, &offsets, &targets, &masks,
        )
        .expect_err("corrupted generated graph should fail validation at seed {seed}");
        assert!(
            err.contains("Fix:"),
            "validation error must be actionable at seed {seed}: {err}"
        );
    }
}
