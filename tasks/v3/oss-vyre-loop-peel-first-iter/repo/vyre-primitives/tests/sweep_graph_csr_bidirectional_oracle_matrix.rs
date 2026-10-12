//! Handwritten oracle matrix for `graph::csr_bidirectional` one-step reach.
//!
//! Compares production bidirectional CSR step against an independent forward+
//! backward union oracle on 1024 generated CSR/frontier shapes.

#![forbid(unsafe_code)]
#![cfg(feature = "cpu-parity")]

use vyre_primitives::graph::csr_bidirectional;

#[test]
fn csr_bidirectional_matches_independent_union_oracle_matrix() {
    for case in 0..8192usize {
        let seed = case as u64 ^ 0xB1D1_0000_0000_0000;
        let (node_count, offsets, targets, masks, frontier, allow_mask) =
            generated_csr_frontier(seed);

        let expected = oracle_bidirectional_step(
            node_count, &offsets, &targets, &masks, &frontier, allow_mask,
        );
        let actual = csr_bidirectional::cpu_ref(
            node_count, &offsets, &targets, &masks, &frontier, allow_mask,
        );
        assert_eq!(
            actual, expected,
            "Fix: csr_bidirectional cpu_ref oracle case {case} node_count={node_count} allow_mask={allow_mask:#x} must match the independent union oracle."
        );

        let mut reused = vec![0xDEAD_BEEF; bitset_words(node_count) + 3];
        csr_bidirectional::cpu_ref_into(
            node_count,
            &offsets,
            &targets,
            &masks,
            &frontier,
            allow_mask,
            &mut reused,
        );
        assert_eq!(
            reused, expected,
            "Fix: csr_bidirectional cpu_ref_into oracle case {case} must clear stale frontier capacity before writing."
        );
    }
}

fn oracle_bidirectional_step(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
) -> Vec<u32> {
    let words = bitset_words(node_count);
    let node_words = node_count as usize;
    let mut out = vec![0u32; words];
    for src in 0..node_words {
        let src_word = src / 32;
        let src_bit = 1u32 << (src % 32);
        let src_in_frontier =
            src_word < frontier_in.len() && (frontier_in[src_word] & src_bit) != 0;
        let edge_start = edge_offsets[src] as usize;
        let edge_end = edge_offsets[src + 1] as usize;
        let mut backward_hit = false;
        for edge in edge_start..edge_end.min(edge_targets.len()).min(edge_kind_mask.len()) {
            if edge_kind_mask[edge] & allow_mask == 0 {
                continue;
            }
            let dst = edge_targets[edge] as usize;
            let dst_word = dst / 32;
            let dst_bit = 1u32 << (dst % 32);
            if src_in_frontier && dst < node_words {
                out[dst_word] |= dst_bit;
            }
            if dst_word < frontier_in.len() && (frontier_in[dst_word] & dst_bit) != 0 {
                backward_hit = true;
            }
        }
        if backward_hit && src_word < out.len() {
            out[src_word] |= src_bit;
        }
    }
    out
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
            let bit = 1u32 << (next_u32(&mut rng) % 5);
            let noise = if next_u32(&mut rng) & 7 == 0 {
                1u32 << (next_u32(&mut rng) % 5)
            } else {
                0
            };
            masks.push(bit | noise);
        }
        offsets.push(targets.len() as u32);
    }
    let mut frontier = vec![0u32; words];
    for node in 0..node_count {
        if next_u32(&mut rng) & 3 == 0 {
            frontier[(node / 32) as usize] |= 1u32 << (node % 32);
        }
    }
    if next_u32(&mut rng) & 1 == 0 {
        let word = (node_count - 1) / 32;
        let used = node_count % 32;
        if used != 0 {
            frontier[word as usize] |= !((1u32 << used) - 1);
        }
    }
    let allow_mask = match next_u32(&mut rng) % 6 {
        0 => 0,
        1 => 1,
        2 => 0b10,
        3 => 0b101,
        _ => 0xFFFF_FFFF,
    };
    (node_count, offsets, targets, masks, frontier, allow_mask)
}

fn bitset_words(node_count: u32) -> usize {
    node_count.div_ceil(32) as usize
}

fn next_u32(rng: &mut u64) -> u32 {
    *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*rng >> 16) as u32
}
