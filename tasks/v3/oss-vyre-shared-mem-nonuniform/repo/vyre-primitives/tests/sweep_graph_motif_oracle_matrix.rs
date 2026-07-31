//! Handwritten oracle matrix for `graph::motif` CSR reference.
//!
//! Compares production motif witness/participation helpers against an
//! independent edge-intersection oracle on 1024 generated CSR shapes.

#![forbid(unsafe_code)]
#![cfg(feature = "cpu-parity")]

use vyre_primitives::graph::motif::{self, count_witness_participants, MotifEdge};

#[test]
fn motif_csr_matches_independent_witness_oracle_matrix() {
    for case in 0..8192usize {
        let seed = case as u64 ^ 0xA07F_CAFE_0000_0000;
        let (node_count, offsets, targets, masks) = generated_csr(seed);
        let motif_edges = generated_motif_edges(seed.rotate_left(13), node_count);

        let expected = oracle_motif_witness(node_count, &offsets, &targets, &masks, &motif_edges);
        let actual = motif::cpu_ref(node_count, &offsets, &targets, &masks, &motif_edges);
        assert_eq!(
            actual, expected,
            "Fix: motif cpu_ref oracle case {case} node_count={node_count} must match the independent witness oracle."
        );

        let expected_matches =
            oracle_motif_all_edges_present(&offsets, &targets, &masks, &motif_edges);
        assert_eq!(
            motif::cpu_ref_matches(&offsets, &targets, &masks, &motif_edges),
            expected_matches,
            "Fix: motif cpu_ref_matches oracle case {case} must match the independent existence oracle."
        );

        let expected_count = oracle_motif_participation_count(node_count, &expected);
        assert_eq!(
            motif::cpu_ref_participation_count(
                node_count,
                &offsets,
                &targets,
                &masks,
                &motif_edges
            ),
            expected_count,
            "Fix: motif participation count oracle case {case} must match the independent dedup endpoint oracle."
        );

        let mut reused = vec![0xCAFE_BABE; node_count as usize + 4];
        motif::cpu_ref_into(
            node_count,
            &offsets,
            &targets,
            &masks,
            &motif_edges,
            &mut reused,
        );
        assert_eq!(
            reused, expected,
            "Fix: motif cpu_ref_into oracle case {case} must clear stale witness capacity before writing."
        );

        let witness_count = count_witness_participants(&actual)
            .expect("Fix: generated motif witness must satisfy the boolean contract.");
        assert_eq!(
            witness_count, expected_count,
            "Fix: motif witness participant count oracle case {case} must agree with participation count."
        );
    }
}

fn oracle_motif_witness(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> Vec<u32> {
    let mut witness = vec![0u32; node_count as usize];
    if !oracle_motif_all_edges_present(edge_offsets, edge_targets, edge_kind_mask, motif_edges) {
        return witness;
    }
    for motif_edge in motif_edges {
        if motif_edge.from < node_count {
            witness[motif_edge.from as usize] = 1;
        }
        if motif_edge.to < node_count {
            witness[motif_edge.to as usize] = 1;
        }
    }
    witness
}

fn oracle_motif_all_edges_present(
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> bool {
    for motif_edge in motif_edges {
        let Some(start) = edge_offsets.get(motif_edge.from as usize).copied() else {
            return false;
        };
        let Some(end) = edge_offsets.get(motif_edge.from as usize + 1).copied() else {
            return false;
        };
        let start = start as usize;
        let end = end as usize;
        let mut found = false;
        for edge_idx in start..end {
            let Some(dst) = edge_targets.get(edge_idx).copied() else {
                break;
            };
            let Some(kind) = edge_kind_mask.get(edge_idx).copied() else {
                break;
            };
            if dst == motif_edge.to && (kind & motif_edge.kind_mask) != 0 {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

fn oracle_motif_participation_count(node_count: u32, witness: &[u32]) -> u32 {
    witness
        .iter()
        .take(node_count as usize)
        .filter(|&&value| value != 0)
        .count() as u32
}

fn generated_csr(seed: u64) -> (u32, Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut rng = seed;
    let node_count = 1 + next_u32(&mut rng) % 96;
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
    (node_count, offsets, targets, masks)
}

fn generated_motif_edges(seed: u64, node_count: u32) -> Vec<MotifEdge> {
    let mut rng = seed;
    let motif_len = 1 + (next_u32(&mut rng) % 5) as usize;
    let mut motif_edges = Vec::with_capacity(motif_len);
    for _ in 0..motif_len {
        motif_edges.push(MotifEdge {
            from: next_u32(&mut rng) % node_count,
            kind_mask: 1u32 << (next_u32(&mut rng) % 5),
            to: next_u32(&mut rng) % node_count,
        });
    }
    motif_edges
}

fn next_u32(rng: &mut u64) -> u32 {
    *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*rng >> 16) as u32
}
