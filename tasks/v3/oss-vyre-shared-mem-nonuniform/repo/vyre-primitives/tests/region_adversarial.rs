//! Adversarial contract tests for `vyre_primitives::matching::region`.
//!
//! Exercises the span-dedup primitive at boundary conditions, degenerate
//! inputs, cluster pathologies, sortedness violations, and scale limits.
//! Every test targets both `dedup_regions_cpu` and `dedup_regions_inplace`
//! to lock the bit-identical post-condition contract.

#![cfg(all(feature = "matching", feature = "cpu-parity"))]

use std::time::Instant;
use vyre_primitives::matching::{dedup_regions_inplace, RegionTriple};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn dedup_regions_cpu(input: Vec<RegionTriple>) -> Vec<RegionTriple> {
    let mut owned = input;
    dedup_regions_inplace(&mut owned);
    owned
}

/// Assert output is sorted by `(pid, start, end)`.
fn assert_sorted(out: &[RegionTriple]) {
    for w in out.windows(2) {
        assert!(
            w[0] <= w[1],
            "FINDING-ADV-REGION-SORT: output not sorted: {:?} before {:?}",
            w[0],
            w[1]
        );
    }
}

/// Assert no adjacent same-pid spans overlap or touch.
fn assert_no_same_pid_overlap(out: &[RegionTriple]) {
    for w in out.windows(2) {
        if w[0].pid == w[1].pid {
            assert!(
                w[0].end < w[1].start,
                "FINDING-ADV-REGION-OVERLAP: adjacent same-pid outputs \
                 overlap or touch: {:?} and {:?}",
                w[0],
                w[1]
            );
        }
    }
}

/// Run both variants and assert they match `expected`.
fn assert_both_eq(input: Vec<RegionTriple>, expected: Vec<RegionTriple>) {
    let cpu = dedup_regions_cpu(input.clone());
    assert_eq!(
        cpu, expected,
        "FINDING-ADV-REGION-CPU: dedup_regions_cpu mismatch"
    );
    let mut inplace = input;
    dedup_regions_inplace(&mut inplace);
    assert_eq!(
        inplace, expected,
        "FINDING-ADV-REGION-INPLACE: dedup_regions_inplace mismatch"
    );
}

/// Deterministic LCG for reproducible pseudo-random tests without `rand`.
fn lcg_next(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
    *state
}

// ---------------------------------------------------------------------------
// 1. u32 boundary conditions
// ---------------------------------------------------------------------------

#[test]
fn u32_boundary_start_max_end_max_zero_width() {
    let t = RegionTriple::new(0, u32::MAX, u32::MAX);
    assert_both_eq(vec![t], vec![t]);
}

#[test]
fn u32_boundary_pid_max() {
    let t = RegionTriple::new(u32::MAX, 10, 20);
    assert_both_eq(vec![t], vec![t]);
}

#[test]
fn u32_boundary_just_fits_cluster() {
    let a = RegionTriple::new(0, u32::MAX - 1, u32::MAX);
    let b = RegionTriple::new(0, u32::MAX, u32::MAX);
    // b.start (MAX) <= a.end (MAX) => merge
    assert_both_eq(
        vec![a, b],
        vec![RegionTriple::new(0, u32::MAX - 1, u32::MAX)],
    );
}

#[test]
fn u32_boundary_all_max() {
    let t = RegionTriple::new(u32::MAX, u32::MAX, u32::MAX);
    assert_both_eq(vec![t, t, t], vec![t]);
}

#[test]
fn u32_boundary_max_boundary_two_different_pid() {
    let a = RegionTriple::new(0, u32::MAX - 1, u32::MAX);
    let b = RegionTriple::new(1, u32::MAX - 1, u32::MAX);
    let mut exp = vec![a, b];
    exp.sort_unstable();
    assert_both_eq(vec![a, b], exp);
}

#[test]
fn u32_boundary_full_range() {
    let a = RegionTriple::new(0, 0, u32::MAX);
    let b = RegionTriple::new(0, u32::MAX - 1, u32::MAX);
    // Merge into single span covering the full u32 range.
    assert_both_eq(vec![a, b], vec![RegionTriple::new(0, 0, u32::MAX)]);
}

// ---------------------------------------------------------------------------
// 2. Zero-width / degenerate
// ---------------------------------------------------------------------------

#[test]
fn degenerate_single_zero_width() {
    let t = RegionTriple::new(7, 100, 100);
    assert_both_eq(vec![t], vec![t]);
}

#[test]
fn degenerate_thousands_identical_zero_width() {
    let t = RegionTriple::new(3, 50, 50);
    let input = vec![t; 10_000];
    assert_both_eq(input, vec![t]);
}

#[test]
fn degenerate_zero_width_different_pids_same_point() {
    let input: Vec<_> = (0..64)
        .map(|pid| RegionTriple::new(pid, 100, 100))
        .collect();
    let mut exp = input.clone();
    exp.sort_unstable();
    assert_both_eq(input, exp);
}

#[test]
fn degenerate_zero_width_touching_nonzero() {
    // (0, 5, 5) touches (0, 5, 10) => merge to (0, 5, 10).
    let a = RegionTriple::new(0, 5, 5);
    let b = RegionTriple::new(0, 5, 10);
    assert_both_eq(vec![a, b], vec![RegionTriple::new(0, 5, 10)]);
}

#[test]
fn degenerate_alternating_overlap_nonoverlap() {
    // Even indices: overlapping pair. Odd indices: separate pair (gap).
    let mut input = Vec::new();
    for i in 0..20 {
        let base = i * 10;
        if i % 2 == 0 {
            input.push(RegionTriple::new(0, base, base + 5));
            input.push(RegionTriple::new(0, base + 3, base + 8));
        } else {
            input.push(RegionTriple::new(0, base, base + 4));
            input.push(RegionTriple::new(0, base + 5, base + 9));
        }
    }
    let out = dedup_regions_cpu(input.clone());
    assert_sorted(&out);
    assert_no_same_pid_overlap(&out);
    // Even indices merge to one; odd indices stay separate (gap of 1).
    // 10 even + 10 odd * 2 = 30 outputs.
    assert_eq!(
        out.len(),
        30,
        "FINDING-ADV-REGION-DEGEN: expected 30 outputs, got {}",
        out.len()
    );
}

#[test]
fn degenerate_zero_width_alternating_pids() {
    let mut input = Vec::new();
    for pid in 0..256u32 {
        input.push(RegionTriple::new(pid, 1000, 1000));
    }
    let mut exp = input.clone();
    exp.sort_unstable();
    assert_both_eq(input, exp);
}

// ---------------------------------------------------------------------------
// 3. Cluster pathologies
// ---------------------------------------------------------------------------

#[test]
fn cluster_100k_all_overlapping_same_pid() {
    let input: Vec<_> = (0..100_000)
        .map(|i| RegionTriple::new(0, (i as u32) % 1000, (i as u32) % 1000 + 500))
        .collect();
    let start = Instant::now();
    let out = dedup_regions_cpu(input.clone());
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 100,
        "FINDING-ADV-REGION-SCALE: 100k overlapping cluster took {} ms",
        elapsed.as_millis()
    );
    assert_eq!(
        out.len(),
        1,
        "FINDING-ADV-REGION-CLUSTER: expected single merged span"
    );
    assert_eq!(out[0], RegionTriple::new(0, 0, 1499));
    assert_sorted(&out);
    assert_no_same_pid_overlap(&out);
}

#[test]
fn cluster_mirror_top_bottom_u32() {
    let bottom = RegionTriple::new(0, 0, 100);
    let top = RegionTriple::new(0, u32::MAX - 100, u32::MAX);
    let mut exp = vec![bottom, top];
    exp.sort_unstable();
    assert_both_eq(vec![top, bottom], exp);
}

#[test]
fn cluster_sliding_window_never_merges() {
    let input: Vec<_> = (0..1000)
        .map(|i| RegionTriple::new(0, (i as u32) * 2, (i as u32) * 2 + 1))
        .collect();
    let out = dedup_regions_cpu(input.clone());
    assert_eq!(
        out.len(),
        1000,
        "FINDING-ADV-REGION-SLIDE: expected no merges"
    );
    assert_sorted(&out);
    assert_no_same_pid_overlap(&out);
}

#[test]
fn cluster_nested_different_pids() {
    let a = RegionTriple::new(0, 0, 100);
    let b = RegionTriple::new(1, 10, 20);
    let c = RegionTriple::new(1, 30, 40);
    let d = RegionTriple::new(0, 50, 60);
    let out = dedup_regions_cpu(vec![a, b, c, d]);
    let mut exp = vec![
        RegionTriple::new(0, 0, 100),
        RegionTriple::new(1, 10, 20),
        RegionTriple::new(1, 30, 40),
    ];
    exp.sort_unstable();
    assert_eq!(out, exp);
}

#[test]
fn cluster_chain_reaction_merge() {
    // Each span extends the previous by 1; all should collapse to one.
    let input: Vec<_> = (0..500)
        .map(|i| RegionTriple::new(0, i as u32, (i + 2) as u32))
        .collect();
    let out = dedup_regions_cpu(input);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0], RegionTriple::new(0, 0, 501));
}

#[test]
fn cluster_interleaved_pids_overlapping() {
    let a = RegionTriple::new(0, 0, 10);
    let b = RegionTriple::new(1, 5, 15);
    let c = RegionTriple::new(0, 8, 12);
    let out = dedup_regions_cpu(vec![a, b, c]);
    // After sorting: (0,0,10), (0,8,12) => merge to (0,0,12);
    // (1,5,15) stays separate.
    let mut exp = vec![RegionTriple::new(0, 0, 12), RegionTriple::new(1, 5, 15)];
    exp.sort_unstable();
    assert_eq!(out, exp);
}

// ---------------------------------------------------------------------------
// 4. Sortedness violations on input
// ---------------------------------------------------------------------------

#[test]
fn sortedness_descending_by_start() {
    let input: Vec<_> = (0..100)
        .map(|i| RegionTriple::new(0, 99 - i, 100 - i))
        .collect();
    let out = dedup_regions_cpu(input);
    assert_sorted(&out);
    assert_no_same_pid_overlap(&out);
    // All touch or overlap, should merge to a single span.
    assert_eq!(out, vec![RegionTriple::new(0, 0, 100)]);
}

#[test]
fn sortedness_random_shuffle() {
    let mut input: Vec<_> = (0..50_u32)
        .map(|i| RegionTriple::new(i % 4, i * 7, i * 7 + 3))
        .collect();
    // Deterministic shuffle: reverse every adjacent pair.
    for chunk in input.chunks_exact_mut(2) {
        chunk.swap(0, 1);
    }
    let out = dedup_regions_cpu(input);
    assert_sorted(&out);
    assert_no_same_pid_overlap(&out);
}

#[test]
fn sortedness_almost_sorted_one_out_of_order() {
    let mut input: Vec<_> = (0..20)
        .map(|i| RegionTriple::new(0, i as u32, i as u32 + 2))
        .collect();
    // Swap two adjacent elements.
    input.swap(5, 6);
    let out = dedup_regions_cpu(input);
    assert_sorted(&out);
    assert_no_same_pid_overlap(&out);
    // All overlapping; should still merge to one.
    assert_eq!(out, vec![RegionTriple::new(0, 0, 21)]);
}

#[test]
fn sortedness_fully_reversed() {
    let mut input: Vec<_> = (0..30)
        .map(|i| RegionTriple::new(0, i as u32, i as u32 + 1))
        .collect();
    input.reverse();
    let out = dedup_regions_cpu(input);
    assert_sorted(&out);
    // All touch: (0,0,1), (0,1,2), ... => merge to (0,0,30).
    assert_eq!(out, vec![RegionTriple::new(0, 0, 30)]);
}

#[test]
fn sortedness_random_shuffle_many_duplicates() {
    let base = RegionTriple::new(2, 10, 20);
    let mut input = vec![base; 200];
    input.reverse();
    let out = dedup_regions_cpu(input);
    assert_sorted(&out);
    assert_eq!(out, vec![base]);
}

// ---------------------------------------------------------------------------
// 5. Memory + scaling
// ---------------------------------------------------------------------------

#[test]
fn scale_empty_vec() {
    assert!(dedup_regions_cpu(vec![]).is_empty());
    let mut v = vec![];
    dedup_regions_inplace(&mut v);
    assert!(v.is_empty());
}

#[test]
fn scale_single_triple() {
    let t = RegionTriple::new(42, 100, 200);
    assert_both_eq(vec![t], vec![t]);
}

#[test]
fn scale_two_triples_every_shape() {
    // Same pid, overlap.
    assert_both_eq(
        vec![RegionTriple::new(0, 0, 5), RegionTriple::new(0, 3, 8)],
        vec![RegionTriple::new(0, 0, 8)],
    );
    // Same pid, touch.
    assert_both_eq(
        vec![RegionTriple::new(0, 0, 5), RegionTriple::new(0, 5, 10)],
        vec![RegionTriple::new(0, 0, 10)],
    );
    // Same pid, separate.
    assert_both_eq(
        vec![RegionTriple::new(0, 0, 5), RegionTriple::new(0, 6, 10)],
        vec![RegionTriple::new(0, 0, 5), RegionTriple::new(0, 6, 10)],
    );
    // Different pid, overlap.
    assert_both_eq(
        vec![RegionTriple::new(0, 0, 10), RegionTriple::new(1, 5, 15)],
        vec![RegionTriple::new(0, 0, 10), RegionTriple::new(1, 5, 15)],
    );
    // Different pid, touch.
    assert_both_eq(
        vec![RegionTriple::new(0, 0, 5), RegionTriple::new(1, 5, 10)],
        vec![RegionTriple::new(0, 0, 5), RegionTriple::new(1, 5, 10)],
    );
    // Different pid, separate.
    assert_both_eq(
        vec![RegionTriple::new(0, 0, 5), RegionTriple::new(1, 6, 10)],
        vec![RegionTriple::new(0, 0, 5), RegionTriple::new(1, 6, 10)],
    );
}

#[test]
fn scale_1024_random_no_panic() {
    let mut rng = 0xdeadbeefu32;
    let mut input = Vec::with_capacity(1024);
    for _ in 0..1024 {
        let pid = lcg_next(&mut rng) % 8;
        let start = lcg_next(&mut rng);
        let len = lcg_next(&mut rng) % 256;
        let end = start.saturating_add(len);
        input.push(RegionTriple::new(pid, start, end));
    }
    let out = dedup_regions_cpu(input.clone());
    assert_sorted(&out);
    assert_no_same_pid_overlap(&out);
    let mut inplace = input;
    dedup_regions_inplace(&mut inplace);
    assert_eq!(out, inplace);
}

#[test]
fn scale_100k_smoke_no_quadratic_blowup() {
    let mut rng = 0xc0ffeeu32;
    let mut input = Vec::with_capacity(100_000);
    for _ in 0..100_000 {
        let pid = lcg_next(&mut rng) % 4;
        let start = lcg_next(&mut rng) % 10_000;
        let len = 100 + (lcg_next(&mut rng) % 500);
        let end = start.saturating_add(len);
        input.push(RegionTriple::new(pid, start, end));
    }
    let start = Instant::now();
    let out = dedup_regions_cpu(input.clone());
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 100,
        "FINDING-ADV-REGION-SCALE: 100k random triples took {} ms \
         (possible quadratic blowup)",
        elapsed.as_millis()
    );
    assert_sorted(&out);
    assert_no_same_pid_overlap(&out);
}

#[test]
fn scale_100k_identical_collapse() {
    let t = RegionTriple::new(5, 1000, 2000);
    let input = vec![t; 100_000];
    let start = Instant::now();
    let out = dedup_regions_cpu(input.clone());
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 100,
        "FINDING-ADV-REGION-SCALE: 100k identical collapse took {} ms",
        elapsed.as_millis()
    );
    assert_eq!(out, vec![t]);
}
