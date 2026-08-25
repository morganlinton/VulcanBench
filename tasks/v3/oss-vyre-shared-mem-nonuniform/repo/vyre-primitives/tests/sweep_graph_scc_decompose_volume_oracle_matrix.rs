//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "graph", feature = "cpu-parity"))]

use vyre_primitives::graph::scc_decompose;

fn bitset_words(node_count: u32) -> usize {
    vyre_primitives::bitset::bitset_words(node_count) as usize
}

fn next_u32(rng: &mut u64) -> u32 {
    *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*rng >> 32) as u32
}

fn generated_scc_case(seed: u64) -> (u32, Vec<u32>, Vec<u32>, Vec<u32>, u32) {
    let mut rng = seed;
    const BOUNDARY_SHAPES: [u32; 27] = [
        0, 1, 2, 31, 32, 33, 63, 64, 65, 95, 96, 127, 128, 129, 255, 256, 257, 300, 511, 512, 513,
        1023, 1024, 1025, 1535, 1536, 1537,
    ];
    let node_count = if next_u32(&mut rng) % 4 == 0 {
        BOUNDARY_SHAPES[(next_u32(&mut rng) as usize) % BOUNDARY_SHAPES.len()]
    } else {
        next_u32(&mut rng) % 2048
    };
    let words = bitset_words(node_count);
    let mut forward = Vec::with_capacity(words);
    let mut backward = Vec::with_capacity(words);
    for _ in 0..words {
        forward.push(next_u32(&mut rng));
        backward.push(next_u32(&mut rng));
    }

    let tail_bits = node_count % 32;
    if tail_bits != 0 && words != 0 {
        let tail_mask = (1u32 << tail_bits) - 1;
        forward[words - 1] &= tail_mask;
        backward[words - 1] &= tail_mask;
    }

    let mut component_in = Vec::with_capacity(node_count as usize);
    for node in 0..node_count {
        let assigned = next_u32(&mut rng) % 7 == 0;
        component_in.push(if assigned {
            next_u32(&mut rng).wrapping_add(node) & 0x7FFF_FFFF
        } else {
            u32::MAX
        });
    }
    let pivot = next_u32(&mut rng);
    (node_count, forward, backward, component_in, pivot)
}

fn oracle_scc(
    node_count: u32,
    forward: &[u32],
    backward: &[u32],
    component_in: &[u32],
    pivot: u32,
) -> Vec<u32> {
    let mut out = component_in.to_vec();
    for v in 0..node_count {
        let word = (v / 32) as usize;
        let bit = 1u32 << (v % 32);
        let fwd = word < forward.len() && forward[word] & bit != 0;
        let bwd = word < backward.len() && backward[word] & bit != 0;
        if fwd && bwd && out[v as usize] == u32::MAX {
            out[v as usize] = pivot;
        }
    }
    out
}

const CASES: usize = 32768;

#[test]
fn sweep_graph_scc_decompose_volume_oracle_matrix() {
    for case in 0..CASES {
        let seed = case as u64 ^ 0x5CCDEC0D;
        let (node_count, forward, backward, component_in, pivot) = generated_scc_case(seed);
        let expected = oracle_scc(node_count, &forward, &backward, &component_in, pivot);
        let actual = scc_decompose::cpu_ref(node_count, &forward, &backward, &component_in, pivot);
        assert_eq!(actual, expected, "Fix: scc_decompose volume case {case}");

        let grid = scc_decompose::scc_decompose_dispatch_grid(node_count);
        assert_eq!(
            grid[1], 1,
            "Fix: SCC grid y dimension drifted at case {case}"
        );
        assert_eq!(
            grid[2], 1,
            "Fix: SCC grid z dimension drifted at case {case}"
        );
        assert!(
            grid[0] >= 1,
            "Fix: SCC dispatch grid must keep an empty graph launchable at case {case}"
        );
        assert!(
            grid[0] * scc_decompose::SCC_DECOMPOSE_WORKGROUP_SIZE[0] >= node_count.max(1),
            "Fix: SCC dispatch grid under-covers node_count={node_count} at case {case}"
        );
    }
}
