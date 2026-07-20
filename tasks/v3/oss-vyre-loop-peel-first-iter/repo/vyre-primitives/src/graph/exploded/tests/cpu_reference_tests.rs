use super::*;

#[test]
fn generated_try_build_cpu_reference_emits_valid_csr_shapes() {
    for procs in 1u32..=4 {
        for blocks in 1u32..=16 {
            for facts in 1u32..=16 {
                let intra: Vec<(u32, u32, u32)> = (0..procs)
                    .flat_map(|proc_id| {
                        (0..blocks.saturating_sub(1)).map(move |block| (proc_id, block, block + 1))
                    })
                    .collect();
                let inter: Vec<(u32, u32, u32, u32)> = if procs > 1 {
                    (0..procs - 1)
                        .map(|proc_id| (proc_id, blocks - 1, proc_id + 1, 0))
                        .collect()
                } else {
                    Vec::new()
                };
                let gen_rules: Vec<(u32, u32, u32)> = (0..procs)
                    .flat_map(|proc_id| {
                        (0..blocks).filter_map(move |block| {
                            if facts > 1 {
                                Some((proc_id, block, (block % (facts - 1)) + 1))
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
                let kill_rules: Vec<(u32, u32, u32)> = (0..procs)
                    .flat_map(|proc_id| {
                        (0..blocks).filter_map(move |block| {
                            (facts > 2 && block % 3 == 0).then_some((proc_id, block, 1))
                        })
                    })
                    .collect();
                let (row_ptr, col_idx) = try_build_cpu_reference(
                    procs,
                    blocks,
                    facts,
                    &intra,
                    &inter,
                    &gen_rules,
                    &kill_rules,
                )
                .unwrap();
                let total_nodes = procs as usize * blocks as usize * facts as usize;
                assert_eq!(row_ptr.len(), total_nodes + 1);
                assert_eq!(row_ptr[total_nodes] as usize, col_idx.len());
                for window in row_ptr.windows(2) {
                    assert!(window[0] <= window[1]);
                }
                for &dst in &col_idx {
                    assert!((dst as usize) < total_nodes);
                }
            }
        }
    }
}

#[test]
fn try_build_cpu_reference_rejects_empty_domain_without_panicking() {
    let err = try_build_cpu_reference(0, 0, 0, &[], &[], &[], &[]).unwrap_err();
    assert!(err.contains("nonzero"));
}

#[test]
fn try_build_cpu_reference_into_reuses_output_and_workspace() {
    let mut row_ptr = Vec::with_capacity(32);
    row_ptr.extend_from_slice(&[9, 8, 7]);
    let mut col_idx = Vec::with_capacity(32);
    col_idx.extend_from_slice(&[6, 5, 4]);
    let mut scratch = ExplodedIfdsCpuScratch {
        edges_flat: Vec::with_capacity(32),
        killed: Vec::with_capacity(32),
        gen_offsets: Vec::with_capacity(16),
        gen_cursor: Vec::with_capacity(16),
        gen_facts: Vec::with_capacity(16),
        cursor: Vec::with_capacity(32),
    };
    scratch.edges_flat.extend_from_slice(&[(99, 98), (97, 96)]);
    scratch.killed.extend_from_slice(&[true, true]);
    scratch.gen_offsets.extend_from_slice(&[11, 12]);
    scratch.gen_cursor.extend_from_slice(&[13, 14]);
    scratch.gen_facts.extend_from_slice(&[15, 16]);
    scratch.cursor.extend_from_slice(&[17, 18]);
    let capacities = (
        row_ptr.capacity(),
        col_idx.capacity(),
        scratch.edges_flat.capacity(),
        scratch.killed.capacity(),
        scratch.gen_offsets.capacity(),
        scratch.gen_cursor.capacity(),
        scratch.gen_facts.capacity(),
        scratch.cursor.capacity(),
    );

    let expected = build_cpu_reference(1, 2, 4, &[(0, 0, 1)], &[], &[(0, 0, 2)], &[(0, 0, 3)]);
    try_build_cpu_reference_into(
        1,
        2,
        4,
        &[(0, 0, 1)],
        &[],
        &[(0, 0, 2)],
        &[(0, 0, 3)],
        &mut row_ptr,
        &mut col_idx,
        &mut scratch,
    )
    .expect("Fix: valid exploded IFDS graph must build with reusable workspace.");

    assert_eq!((row_ptr.clone(), col_idx.clone()), expected);
    assert_eq!(
        (
            row_ptr.capacity(),
            col_idx.capacity(),
            scratch.edges_flat.capacity(),
            scratch.killed.capacity(),
            scratch.gen_offsets.capacity(),
            scratch.gen_cursor.capacity(),
            scratch.gen_facts.capacity(),
            scratch.cursor.capacity(),
        ),
        capacities
    );

    try_build_cpu_reference_into(
        1,
        1,
        1,
        &[],
        &[],
        &[],
        &[],
        &mut row_ptr,
        &mut col_idx,
        &mut scratch,
    )
    .expect("Fix: smaller exploded IFDS graph must reuse the same workspace.");

    assert_eq!(row_ptr, vec![0, 0]);
    assert!(col_idx.is_empty());
    assert_eq!(
        (
            row_ptr.capacity(),
            col_idx.capacity(),
            scratch.edges_flat.capacity(),
            scratch.killed.capacity(),
            scratch.gen_offsets.capacity(),
            scratch.gen_cursor.capacity(),
            scratch.gen_facts.capacity(),
            scratch.cursor.capacity(),
        ),
        capacities
    );
}

#[test]
fn try_build_cpu_reference_into_validates_before_mutating_storage() {
    let mut row_ptr = vec![9, 8, 7];
    let mut col_idx = vec![6, 5, 4];
    let mut scratch = ExplodedIfdsCpuScratch {
        edges_flat: vec![(1, 2)],
        killed: vec![true],
        gen_offsets: vec![3],
        gen_cursor: vec![4],
        gen_facts: vec![5],
        cursor: vec![6],
    };

    let err = try_build_cpu_reference_into(
        0,
        0,
        0,
        &[],
        &[],
        &[],
        &[],
        &mut row_ptr,
        &mut col_idx,
        &mut scratch,
    )
    .expect_err("Fix: empty exploded IFDS domain must be rejected.");

    assert!(err.contains("nonzero"));
    assert_eq!(row_ptr, vec![9, 8, 7]);
    assert_eq!(col_idx, vec![6, 5, 4]);
    assert_eq!(scratch.edges_flat, vec![(1, 2)]);
    assert_eq!(scratch.killed, vec![true]);
    assert_eq!(scratch.gen_offsets, vec![3]);
    assert_eq!(scratch.gen_cursor, vec![4]);
    assert_eq!(scratch.gen_facts, vec![5]);
    assert_eq!(scratch.cursor, vec![6]);
}

#[test]
fn generated_try_build_cpu_reference_into_matches_allocating_reference() {
    let mut row_ptr = Vec::new();
    let mut col_idx = Vec::new();
    let mut scratch = ExplodedIfdsCpuScratch::new();

    for case in 0..1024usize {
        let num_procs = 1 + (case % 3) as u32;
        let blocks_per_proc = 1 + ((case / 3) % 5) as u32;
        let facts_per_proc = 1 + ((case / 15) % 5) as u32;
        let mut intra_edges = Vec::new();
        let mut inter_edges = Vec::new();
        let mut flow_gen = Vec::new();
        let mut flow_kill = Vec::new();

        for p in 0..num_procs {
            for b in 0..blocks_per_proc {
                let next_b = (b + 1) % blocks_per_proc;
                let mixed = case
                    .wrapping_mul(37)
                    .wrapping_add((p as usize).wrapping_mul(11))
                    .wrapping_add((b as usize).wrapping_mul(7));
                if blocks_per_proc > 1 && mixed % 2 == 0 {
                    intra_edges.push((p, b, next_b));
                }
                let fact = (mixed as u32) % facts_per_proc;
                if mixed % 3 == 0 {
                    flow_gen.push((p, b, fact));
                }
                if mixed % 5 == 0 && fact != 0 {
                    flow_kill.push((p, b, fact));
                }
            }
        }
        if num_procs > 1 {
            for p in 0..num_procs - 1 {
                if (case + p as usize) % 2 == 0 {
                    inter_edges.push((p, 0, p + 1, 0));
                }
            }
        }

        let expected = try_build_cpu_reference(
            num_procs,
            blocks_per_proc,
            facts_per_proc,
            &intra_edges,
            &inter_edges,
            &flow_gen,
            &flow_kill,
        )
        .expect("Fix: generated exploded IFDS graph must build through allocating oracle.");
        try_build_cpu_reference_into(
            num_procs,
            blocks_per_proc,
            facts_per_proc,
            &intra_edges,
            &inter_edges,
            &flow_gen,
            &flow_kill,
            &mut row_ptr,
            &mut col_idx,
            &mut scratch,
        )
        .expect("Fix: generated exploded IFDS graph must build through reusable oracle.");
        assert_eq!(
            (row_ptr.clone(), col_idx.clone()),
            expected,
            "Fix: reusable exploded IFDS oracle diverged at generated case {case}."
        );
    }
}
