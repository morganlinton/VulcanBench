//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "graph", feature = "cpu-parity"))]

use vyre_primitives::graph::csr_forward_traverse;

fn bitset_words(node_count: u32) -> usize {
    vyre_primitives::bitset::bitset_words(node_count) as usize
}

fn next_u32(rng: &mut u64) -> u32 {
    *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*rng >> 32) as u32
}

fn generated_csr_frontier(seed: u64) -> (u32, Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>, u32) {
    let mut rng = seed;
    let node_count = 1 + next_u32(&mut rng) % 96;
    let words = bitset_words(node_count);
    let mut offsets = Vec::with_capacity(node_count as usize + 1);
    let mut targets = Vec::new();
    let mut masks = Vec::new();
    offsets.push(0);
    for _ in 0..node_count {
        let degree = next_u32(&mut rng) % 6;
        for _ in 0..degree {
            targets.push(next_u32(&mut rng) % node_count);
            masks.push(1u32 << (next_u32(&mut rng) % 5));
        }
        offsets.push(targets.len() as u32);
    }
    let mut frontier = vec![0u32; words];
    let start = next_u32(&mut rng) % node_count;
    frontier[(start / 32) as usize] |= 1u32 << (start % 32);
    let allow_mask = 0xFFFF_FFFFu32;
    (node_count, offsets, targets, masks, frontier, allow_mask)
}

fn oracle_csr_forward_step(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
) -> Vec<u32> {
    let words = bitset_words(node_count);
    let mut out = vec![0u32; words];
    for src in 0..node_count {
        let word_idx = (src / 32) as usize;
        let bit_mask = 1u32 << (src % 32);
        if word_idx >= frontier_in.len() || (frontier_in[word_idx] & bit_mask) == 0 {
            continue;
        }
        let edge_start = edge_offsets[src as usize] as usize;
        let edge_end = edge_offsets[src as usize + 1] as usize;
        for e in edge_start..edge_end {
            if (edge_kind_mask[e] & allow_mask) == 0 {
                continue;
            }
            let dst = edge_targets[e];
            if dst < node_count {
                let dst_word = (dst / 32) as usize;
                let dst_bit = 1u32 << (dst % 32);
                out[dst_word] |= dst_bit;
            }
        }
    }
    out
}

const CASES: usize = 16384;

#[test]
fn sweep_graph_csr_forward_traverse_volume_oracle_matrix() {
    for case in 0..CASES {
        let seed = case as u64 ^ 0xF074D4D01;
        let (node_count, offsets, targets, masks, frontier, allow_mask) =
            generated_csr_frontier(seed);
        let expected = oracle_csr_forward_step(
            node_count, &offsets, &targets, &masks, &frontier, allow_mask,
        );
        let actual = csr_forward_traverse::cpu_ref(
            node_count, &offsets, &targets, &masks, &frontier, allow_mask,
        );
        assert_eq!(
            actual, expected,
            "Fix: csr_forward_traverse volume case {case} node_count={node_count}"
        );
    }
}
