//! Contracts for GPU-resident region dedup survivor flags.
//!
//! Pulls in the CPU oracle (`dedup_regions_inplace`) which lives behind
//! `cpu-parity`; gate the test accordingly so cargo doesn't try to
//! resolve the import without the feature.

#![cfg(all(feature = "matching", feature = "cpu-parity"))]

use vyre_primitives::matching::{dedup_regions_flag_program, dedup_regions_inplace, RegionTriple};

fn dedup_regions_cpu(input: Vec<RegionTriple>) -> Vec<RegionTriple> {
    let mut owned = input;
    dedup_regions_inplace(&mut owned);
    owned
}

fn cpu_reference_flags(sorted: &[RegionTriple]) -> Vec<u32> {
    let mut flags = vec![0u32; sorted.len()];
    if sorted.is_empty() {
        return flags;
    }
    flags[0] = 1;
    for i in 1..sorted.len() {
        let cur = sorted[i];
        let prv = sorted[i - 1];
        let different_pid = cur.pid != prv.pid;
        let no_overlap = cur.start > prv.end;
        flags[i] = if different_pid || no_overlap { 1 } else { 0 };
    }
    flags
}

#[test]
fn flag_program_emitted_with_expected_buffer_count() {
    let prog = dedup_regions_flag_program("p", "s", "e", "f", 8);
    assert!(!prog.entry().is_empty());
    assert_eq!(prog.workgroup_size[1], 1);
    assert_eq!(prog.workgroup_size[2], 1);
    assert_eq!(prog.buffers.len(), 4);
    assert_eq!(prog.buffers[0].count, 8);
}

#[test]
fn flag_predicate_matches_cpu_reference_on_canonical_inputs() {
    let scenarios: &[Vec<RegionTriple>] = &[
        vec![],
        vec![RegionTriple::new(0, 5, 10)],
        vec![RegionTriple::new(0, 5, 10), RegionTriple::new(0, 5, 10)],
        vec![RegionTriple::new(0, 5, 10), RegionTriple::new(0, 7, 12)],
        vec![RegionTriple::new(0, 5, 10), RegionTriple::new(0, 10, 15)],
        vec![RegionTriple::new(0, 5, 10), RegionTriple::new(1, 5, 10)],
        vec![RegionTriple::new(0, 5, 5), RegionTriple::new(1, 5, 5)],
    ];
    for scenario in scenarios {
        let mut sorted = scenario.clone();
        sorted.sort_unstable();
        let flags = cpu_reference_flags(&sorted);
        let cpu_dedup = dedup_regions_cpu(scenario.clone());
        let expected_survivors = flags.iter().filter(|&&f| f == 1).count();
        assert_eq!(
            expected_survivors,
            cpu_dedup.len(),
            "Fix: flag-program survivor count must match CPU dedup output count for {scenario:?}"
        );
    }
}
