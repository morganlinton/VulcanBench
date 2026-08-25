//! Sweep oracle matrix for self-substrate CSR/graph CPU references.
//!
//! Compares substrate reference wrappers against independent bitset oracles
//! across hostile CSR shapes. Uses CPU reference paths only - no mock
//! dispatchers.

#![forbid(unsafe_code)]

use vyre_primitives::graph::exploded::build_cpu_reference;
use vyre_self_substrate::exploded::{
    build_ifds_csr_via, reference_build_ifds_csr, reference_canonicalize_csr_within_rows,
};
use vyre_self_substrate::graph::csr_bidirectional::reference_bidirectional_step;
use vyre_self_substrate::graph::csr_forward_or_changed::reference_forward_step_with_change_flag;
use vyre_self_substrate::graph::persistent_bfs::bfs_expand;
use vyre_self_substrate::optimizer::dispatcher::oracle::CpuOracleDispatcher;

const CASES_PER_FAMILY: u64 = 512;

#[derive(Clone, Copy)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 7;
        x ^= x >> 9;
        x ^= x << 8;
        self.0 = x;
        (x >> 16) as u32
    }

    fn range(&mut self, upper: u32) -> u32 {
        if upper == 0 {
            0
        } else {
            self.next_u32() % upper
        }
    }
}

fn bitset_words(node_count: u32) -> usize {
    node_count.div_ceil(32) as usize
}

fn generated_csr(seed: u64) -> (u32, Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>, u32) {
    let mut rng = Rng::new(seed);
    let node_count = 1 + rng.range(96);
    let words = bitset_words(node_count);
    let mut offsets = Vec::with_capacity(node_count as usize + 1);
    let mut targets = Vec::new();
    let mut masks = Vec::new();
    offsets.push(0);
    for _ in 0..node_count {
        let degree = rng.range(5);
        for _ in 0..degree {
            targets.push(rng.range(node_count));
            let bit = 1u32 << rng.range(5);
            let noise = if rng.next_u32() & 7 == 0 {
                1u32 << rng.range(5)
            } else {
                0
            };
            masks.push(bit | noise);
        }
        offsets.push(targets.len() as u32);
    }
    let mut frontier = vec![0u32; words];
    for node in 0..node_count {
        if rng.next_u32() & 3 == 0 {
            frontier[(node / 32) as usize] |= 1u32 << (node % 32);
        }
    }
    if rng.next_u32() & 1 == 0 {
        let word = (node_count - 1) / 32;
        let used = node_count % 32;
        if used != 0 {
            frontier[word as usize] |= !((1u32 << used) - 1);
        }
    }
    let allow_mask = match rng.range(6) {
        0 => 0,
        1 => 1,
        2 => 0b10,
        3 => 0b101,
        _ => 0xFFFF_FFFF,
    };
    (node_count, offsets, targets, masks, frontier, allow_mask)
}

fn bit_is_set(words: &[u32], node: u32) -> bool {
    let word = (node / 32) as usize;
    let bit = 1u32 << (node % 32);
    words.get(word).is_some_and(|value| value & bit != 0)
}

fn oracle_forward_or_changed(
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
    masks: &[u32],
    frontier: &[u32],
    allow_mask: u32,
) -> (Vec<u32>, u32) {
    let words = bitset_words(node_count);
    let mut out = frontier.to_vec();
    out.resize(words, 0);
    let mut changed = 0;
    for src in 0..node_count {
        if !bit_is_set(&out, src) {
            continue;
        }
        let start = offsets[src as usize] as usize;
        let end = offsets[src as usize + 1] as usize;
        for edge in start..end {
            if masks[edge] & allow_mask == 0 {
                continue;
            }
            let dst = targets[edge];
            let word = (dst / 32) as usize;
            let bit = 1u32 << (dst % 32);
            let before = out[word];
            out[word] |= bit;
            if out[word] != before {
                changed = 1;
            }
        }
    }
    (out, changed)
}

fn snapshot_successors(
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
    masks: &[u32],
    frontier: &[u32],
    allow_mask: u32,
) -> Vec<u32> {
    let mut out = vec![0u32; bitset_words(node_count)];
    for src in 0..node_count {
        if !bit_is_set(frontier, src) {
            continue;
        }
        let start = offsets[src as usize] as usize;
        let end = offsets[src as usize + 1] as usize;
        for edge in start..end {
            if masks[edge] & allow_mask == 0 {
                continue;
            }
            let dst = targets[edge];
            out[(dst / 32) as usize] |= 1u32 << (dst % 32);
        }
    }
    out
}

fn oracle_bidirectional_step(
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
    masks: &[u32],
    frontier: &[u32],
    allow_mask: u32,
) -> Vec<u32> {
    let words = bitset_words(node_count);
    let mut out = vec![0u32; words];
    for src in 0..node_count {
        let src_word = (src / 32) as usize;
        let src_bit = 1u32 << (src % 32);
        let src_in_frontier = bit_is_set(frontier, src);
        let edge_start = offsets[src as usize] as usize;
        let edge_end = offsets[src as usize + 1] as usize;
        let mut backward_hit = false;
        for edge in edge_start..edge_end {
            if masks[edge] & allow_mask == 0 {
                continue;
            }
            let dst = targets[edge];
            if src_in_frontier && dst < node_count {
                out[(dst / 32) as usize] |= 1u32 << (dst % 32);
            }
            if bit_is_set(frontier, dst) {
                backward_hit = true;
            }
        }
        if backward_hit {
            out[src_word] |= src_bit;
        }
    }
    out
}

fn oracle_persistent_bfs(
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
    masks: &[u32],
    frontier: &[u32],
    allow_mask: u32,
    max_iters: u32,
) -> (Vec<u32>, u32) {
    let words = bitset_words(node_count);
    let mut out = frontier.to_vec();
    out.resize(words, 0);
    let mut changed = 0;
    for _ in 0..max_iters {
        let step = snapshot_successors(node_count, offsets, targets, masks, &out, allow_mask);
        let mut step_changed = false;
        for word in 0..words {
            let before = out[word];
            out[word] |= step[word];
            if out[word] != before {
                step_changed = true;
            }
        }
        if step_changed {
            changed = 1;
        } else {
            break;
        }
    }
    (out, changed)
}

fn canonical_ifds_csr(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra: &[(u32, u32, u32)],
    inter: &[(u32, u32, u32, u32)],
    gen: &[(u32, u32, u32)],
    kill: &[(u32, u32, u32)],
) -> (Vec<u32>, Vec<u32>) {
    let (row_ptr, col_idx) = build_cpu_reference(
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra,
        inter,
        gen,
        kill,
    );
    reference_canonicalize_csr_within_rows(&row_ptr, &col_idx)
}

fn generated_ifds_rules(
    seed: u64,
) -> (
    u32,
    u32,
    u32,
    Vec<(u32, u32, u32)>,
    Vec<(u32, u32, u32, u32)>,
    Vec<(u32, u32, u32)>,
    Vec<(u32, u32, u32)>,
) {
    let mut rng = Rng::new(seed);
    let num_procs = 1 + rng.range(4);
    let blocks_per_proc = 1 + rng.range(8);
    let facts_per_proc = 1 + rng.range(8);
    let mut intra_edges = Vec::new();
    let mut inter_edges = Vec::new();
    let mut flow_gen = Vec::new();
    let mut flow_kill = Vec::new();

    for p in 0..num_procs {
        for b in 0..blocks_per_proc {
            if blocks_per_proc > 1 && rng.next_u32() & 1 == 0 {
                intra_edges.push((p, b, (b + 1) % blocks_per_proc));
            }
            let fact = rng.range(facts_per_proc);
            if rng.next_u32() % 3 == 0 {
                flow_gen.push((p, b, fact));
            }
            if rng.next_u32() % 5 == 0 && fact != 0 {
                flow_kill.push((p, b, fact));
            }
        }
    }
    if num_procs > 1 {
        for p in 0..num_procs - 1 {
            if rng.next_u32() & 1 == 0 {
                inter_edges.push((p, 0, p + 1, 0));
            }
        }
    }

    (
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_edges,
        inter_edges,
        flow_gen,
        flow_kill,
    )
}

#[test]
fn sweep_csr_forward_or_changed_matches_independent_oracle_matrix() {
    let mut assertions = 0usize;
    for case in 0..CASES_PER_FAMILY {
        let (node_count, offsets, targets, masks, frontier, allow_mask) =
            generated_csr(0xF0C5_0001 ^ case.wrapping_mul(0x9E37_79B9));
        let expected = oracle_forward_or_changed(
            node_count, &offsets, &targets, &masks, &frontier, allow_mask,
        );
        let actual = reference_forward_step_with_change_flag(
            node_count, &offsets, &targets, &masks, &frontier, allow_mask,
        );
        assert_eq!(
            actual, expected,
            "Fix: csr_forward_or_changed case {case} node_count={node_count} must match independent oracle."
        );
        assert_eq!(actual.0.len(), bitset_words(node_count));
        assertions += 2;
    }
    assert_eq!(assertions, CASES_PER_FAMILY as usize * 2);
}

#[test]
fn sweep_csr_bidirectional_step_matches_independent_oracle_matrix() {
    let mut assertions = 0usize;
    for case in 0..CASES_PER_FAMILY {
        let (node_count, offsets, targets, masks, frontier, allow_mask) =
            generated_csr(0xB1D1_0002 ^ case.wrapping_mul(0xD1B5_4A32));
        let expected = oracle_bidirectional_step(
            node_count, &offsets, &targets, &masks, &frontier, allow_mask,
        );
        let actual = reference_bidirectional_step(
            node_count, &offsets, &targets, &masks, &frontier, allow_mask,
        );
        assert_eq!(
            actual, expected,
            "Fix: csr_bidirectional case {case} node_count={node_count} must match independent oracle."
        );
        assert_ne!(
            actual.len(),
            0,
            "Fix: csr_bidirectional case {case} must return bitset words."
        );
        assertions += 2;
    }
    assert_eq!(assertions, CASES_PER_FAMILY as usize * 2);
}

#[test]
fn sweep_persistent_bfs_matches_independent_oracle_matrix() {
    let mut assertions = 0usize;
    for case in 0..CASES_PER_FAMILY {
        let (node_count, offsets, targets, masks, frontier, allow_mask) =
            generated_csr(0xBFC0_0003 ^ case.wrapping_mul(0xA24B_AED4));
        let max_iters = (case as u32 % 9) + 1;
        let expected = oracle_persistent_bfs(
            node_count, &offsets, &targets, &masks, &frontier, allow_mask, max_iters,
        );
        let actual = bfs_expand(
            node_count, &offsets, &targets, &masks, &frontier, allow_mask, max_iters,
        );
        assert_eq!(
            actual, expected,
            "Fix: persistent_bfs case {case} node_count={node_count} max_iters={max_iters} must match independent oracle."
        );
        assert_eq!(actual.0.len(), bitset_words(node_count));
        assertions += 2;
    }
    assert_eq!(assertions, CASES_PER_FAMILY as usize * 2);
}

#[test]
fn sweep_exploded_ifds_substrate_matches_primitive_oracle_matrix() {
    let mut assertions = 0usize;
    for case in 0..CASES_PER_FAMILY {
        let (num_procs, blocks_per_proc, facts_per_proc, intra, inter, gen, kill) =
            generated_ifds_rules(0x1F05_0004 ^ case.wrapping_mul(0x85EB_CA6B));
        let expected = canonical_ifds_csr(
            num_procs,
            blocks_per_proc,
            facts_per_proc,
            &intra,
            &inter,
            &gen,
            &kill,
        );
        let (row_ptr, col_idx) = reference_build_ifds_csr(
            num_procs,
            blocks_per_proc,
            facts_per_proc,
            &intra,
            &inter,
            &gen,
            &kill,
        );
        let actual = reference_canonicalize_csr_within_rows(&row_ptr, &col_idx);
        assert_eq!(
            actual, expected,
            "Fix: exploded IFDS substrate reference case {case} procs={num_procs} blocks={blocks_per_proc} facts={facts_per_proc} must match primitive CPU oracle."
        );
        assertions += 2;
    }
    assert_eq!(assertions, CASES_PER_FAMILY as usize * 2);
}

#[test]
fn sweep_exploded_ifds_via_matches_cpu_oracle_matrix() {
    let dispatcher = CpuOracleDispatcher::new();
    let mut assertions = 0usize;
    for case in 0..CASES_PER_FAMILY {
        let (num_procs, blocks_per_proc, facts_per_proc, intra, inter, gen, kill) =
            generated_ifds_rules(0x1F05_0005 ^ case.wrapping_mul(0xC2B2_AE35));
        let expected = canonical_ifds_csr(
            num_procs,
            blocks_per_proc,
            facts_per_proc,
            &intra,
            &inter,
            &gen,
            &kill,
        );
        let actual = build_ifds_csr_via(
            &dispatcher,
            num_procs,
            blocks_per_proc,
            facts_per_proc,
            &intra,
            &inter,
            &gen,
            &kill,
        )
        .unwrap_or_else(|error| {
            panic!("Fix: exploded IFDS via CPU oracle case {case} must dispatch: {error:?}")
        });
        assert_eq!(
            actual, expected,
            "Fix: exploded IFDS via CPU oracle case {case} procs={num_procs} blocks={blocks_per_proc} facts={facts_per_proc} must match reference CSR."
        );
        assertions += 2;
    }
    assert_eq!(assertions, CASES_PER_FAMILY as usize * 2);
}
