use super::region::*;

fn cluster_metadata_for_sorted(input: &[RegionTriple]) -> (Vec<u32>, Vec<u32>) {
    let mut survivors = vec![0u32; input.len()];
    let mut merged_ends = input.iter().map(|region| region.end).collect::<Vec<_>>();

    for i in 0..input.len() {
        let current = input[i];
        let has_prev_overlap = input[..i]
            .iter()
            .any(|prior| prior.pid == current.pid && prior.end >= current.start);
        if has_prev_overlap {
            continue;
        }

        survivors[i] = 1;
        let mut merged_end = current.end;
        for next in &input[i + 1..] {
            if next.pid != current.pid || next.start > merged_end {
                break;
            }
            merged_end = merged_end.max(next.end);
        }
        merged_ends[i] = merged_end;
    }

    (survivors, merged_ends)
}

fn compact_cluster_metadata(
    sorted: &[RegionTriple],
    survivors: &[u32],
    merged_ends: &[u32],
) -> Vec<RegionTriple> {
    sorted
        .iter()
        .zip(survivors.iter())
        .zip(merged_ends.iter())
        .filter_map(|((&region, &survivor), &merged_end)| {
            (survivor != 0).then(|| RegionTriple::new(region.pid, region.start, merged_end))
        })
        .collect()
}

#[test]
fn empty_input() {
    assert!(dedup_regions_cpu(vec![]).is_empty());
}

#[test]
fn single_pass_through() {
    let r = RegionTriple::new(0, 5, 10);
    assert_eq!(dedup_regions_cpu(vec![r]), vec![r]);
}

#[test]
fn exact_duplicate_collapses() {
    let r = RegionTriple::new(0, 5, 10);
    assert_eq!(dedup_regions_cpu(vec![r, r]), vec![r]);
}

#[test]
fn overlapping_same_pid_merges() {
    let a = RegionTriple::new(0, 5, 10);
    let b = RegionTriple::new(0, 7, 12);
    assert_eq!(
        dedup_regions_cpu(vec![a, b]),
        vec![RegionTriple::new(0, 5, 12)]
    );
}

#[test]
fn touching_same_pid_merges() {
    let a = RegionTriple::new(0, 5, 10);
    let b = RegionTriple::new(0, 10, 15);
    assert_eq!(
        dedup_regions_cpu(vec![a, b]),
        vec![RegionTriple::new(0, 5, 15)]
    );
}

#[test]
fn different_pids_never_merge() {
    let a = RegionTriple::new(0, 5, 10);
    let b = RegionTriple::new(1, 5, 10);
    let mut got = dedup_regions_cpu(vec![a, b]);
    got.sort_unstable();
    assert_eq!(got, vec![a, b]);
}

#[test]
fn unsorted_input_handled() {
    let a = RegionTriple::new(0, 5, 10);
    let b = RegionTriple::new(0, 7, 12);
    let c = RegionTriple::new(1, 3, 4);
    let got = dedup_regions_cpu(vec![b, a, c]);
    assert_eq!(got, vec![RegionTriple::new(0, 5, 12), c]);
}

#[test]
fn cluster_of_three_merges() {
    let a = RegionTriple::new(0, 1, 3);
    let b = RegionTriple::new(0, 2, 5);
    let c = RegionTriple::new(0, 4, 8);
    assert_eq!(
        dedup_regions_cpu(vec![a, b, c]),
        vec![RegionTriple::new(0, 1, 8)]
    );
}

#[test]
fn zero_width_matches_preserved() {
    let a = RegionTriple::new(0, 5, 5);
    let b = RegionTriple::new(1, 5, 5);
    let mut got = dedup_regions_cpu(vec![a, b]);
    got.sort_unstable();
    assert_eq!(got, vec![a, b]);
}

#[test]
fn cluster_metadata_handles_nested_short_previous_span() {
    let sorted = vec![
        RegionTriple::new(7, 0, 10),
        RegionTriple::new(7, 2, 3),
        RegionTriple::new(7, 9, 12),
        RegionTriple::new(7, 20, 25),
    ];
    let (survivors, merged_ends) = cluster_metadata_for_sorted(&sorted);

    assert_eq!(survivors, vec![1, 0, 0, 1]);
    assert_eq!(
        compact_cluster_metadata(&sorted, &survivors, &merged_ends),
        vec![RegionTriple::new(7, 0, 12), RegionTriple::new(7, 20, 25)]
    );
}

#[test]
fn generated_cluster_metadata_matches_cpu_dedup() {
    let mut state = 0xC013_CADE_u32;
    for case in 0..4096u32 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let count = (state % 96) as usize;
        let mut input = Vec::with_capacity(count);
        for index in 0..count {
            state = state.rotate_left(5) ^ (index as u32).wrapping_mul(0x9E37_79B9);
            let pid = state % 5;
            state = state.rotate_left(7).wrapping_add(case);
            let start = state % 160;
            state = state.rotate_left(11) ^ 0x85EB_CA6B;
            let width = state % 24;
            input.push(RegionTriple::new(pid, start, start.saturating_add(width)));
        }

        let expected = dedup_regions_cpu(input.clone());
        let mut sorted = input;
        sort_regions_cpu(&mut sorted);
        let (survivors, merged_ends) = cluster_metadata_for_sorted(&sorted);
        let actual = compact_cluster_metadata(&sorted, &survivors, &merged_ends);

        assert_eq!(actual, expected, "generated region cluster case {case}");
    }
}

#[test]
fn sort_regions_cpu_matches_ord_impl() {
    let mut a = vec![
        RegionTriple::new(2, 0, 1),
        RegionTriple::new(0, 5, 10),
        RegionTriple::new(1, 3, 4),
        RegionTriple::new(0, 5, 8),
        RegionTriple::new(0, 5, 10),
    ];
    sort_regions_cpu(&mut a);
    assert_eq!(
        a,
        vec![
            RegionTriple::new(0, 5, 8),
            RegionTriple::new(0, 5, 10),
            RegionTriple::new(0, 5, 10),
            RegionTriple::new(1, 3, 4),
            RegionTriple::new(2, 0, 1),
        ]
    );
}

#[test]
fn sort_regions_cpu_is_stable_for_equal_triples() {
    let mut a = vec![
        RegionTriple::new(0, 5, 10),
        RegionTriple::new(0, 5, 10),
        RegionTriple::new(0, 5, 10),
    ];
    sort_regions_cpu(&mut a);
    assert_eq!(a.len(), 3);
    for r in &a {
        assert_eq!(*r, RegionTriple::new(0, 5, 10));
    }
}

#[test]
fn region_dedup_dispatch_grid_packs_large_match_buffers() {
    assert_eq!(region_dedup_dispatch_grid(0), [1, 1, 1]);
    assert_eq!(region_dedup_dispatch_grid(1), [1, 1, 1]);
    assert_eq!(region_dedup_dispatch_grid(256), [1, 1, 1]);
    assert_eq!(region_dedup_dispatch_grid(257), [2, 1, 1]);
    assert_eq!(region_dedup_dispatch_grid(513), [3, 1, 1]);
}

#[test]
fn dedup_regions_flag_program_emits_expected_buffers() {
    let p = dedup_regions_flag_program("pids", "starts", "ends", "survivors", 513);
    assert_eq!(p.workgroup_size, REGION_DEDUP_WORKGROUP_SIZE);
    let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
    assert_eq!(names, vec!["pids", "starts", "ends", "survivors"]);
    for buf in p.buffers.iter() {
        assert_eq!(buf.count(), 513);
    }
}

#[test]
fn dedup_regions_cluster_program_emits_survivor_and_merged_end_outputs() {
    let p = dedup_regions_cluster_program("pids", "starts", "ends", "survivors", "merged", 64);
    assert_eq!(p.workgroup_size, REGION_DEDUP_WORKGROUP_SIZE);
    let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
    assert_eq!(names, vec!["pids", "starts", "ends", "survivors", "merged"]);
    assert_eq!(
        p.buffers[3].access(),
        vyre_foundation::ir::BufferAccess::WriteOnly
    );
    assert_eq!(
        p.buffers[4].access(),
        vyre_foundation::ir::BufferAccess::WriteOnly
    );
}

#[test]
fn region_sort_program_emits_expected_buffers() {
    let p = region_sort_program("pi", "si", "ei", "po", "so", "eo", 64);
    assert_eq!(p.workgroup_size, [256, 1, 1]);
    let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
    assert_eq!(names, vec!["pi", "si", "ei", "po", "so", "eo"]);
    for buf in p.buffers.iter() {
        assert_eq!(buf.count(), 64);
    }
}

#[test]
fn region_sort_program_zero_count_traps() {
    let p = region_sort_program("pi", "si", "ei", "po", "so", "eo", 0);
    assert!(p.stats().trap());
}

#[test]
fn region_sort_program_pipeline_composes_with_dedup_cluster_metadata() {
    let sort_p = region_sort_program("pi", "si", "ei", "ps", "ss", "es", 32);
    let cluster_p = dedup_regions_cluster_program("ps", "ss", "es", "flags", "merged", 32);
    let sort_outputs: Vec<&str> = sort_p
        .buffers
        .iter()
        .filter(|b| b.access() == vyre_foundation::ir::BufferAccess::ReadWrite)
        .map(|b| b.name())
        .collect();
    assert_eq!(sort_outputs, vec!["ps", "ss", "es"]);
    let cluster_inputs: Vec<&str> = cluster_p
        .buffers
        .iter()
        .filter(|b| b.access() == vyre_foundation::ir::BufferAccess::ReadOnly)
        .map(|b| b.name())
        .collect();
    assert_eq!(cluster_inputs, vec!["ps", "ss", "es"]);
}
