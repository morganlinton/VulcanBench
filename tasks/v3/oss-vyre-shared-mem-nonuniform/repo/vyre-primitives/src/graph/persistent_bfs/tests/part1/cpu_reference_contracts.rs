use super::*;

#[test]
fn persistent_bfs_reaches_closure() {
    let (frontier, changed) = cpu_ref(
        4,
        &[0, 2, 3, 4, 4],
        &[1, 2, 3, 3],
        &[1, 1, 1, 1],
        &[0b0001],
        0xFFFF_FFFF,
        4,
    );
    assert_eq!(frontier, vec![0b1111]);
    assert_eq!(changed, 1);
}

#[test]
fn cpu_ref_into_reuses_frontier_storage() {
    let mut frontier = Vec::with_capacity(8);
    let changed = cpu_ref_into(
        4,
        &[0, 1, 2, 3, 3],
        &[1, 2, 3],
        &[1, 1, 1],
        &[0b0001],
        0xFFFF_FFFF,
        8,
        &mut frontier,
    );
    let capacity = frontier.capacity();
    assert_eq!(frontier, vec![0b1111]);
    assert_eq!(changed, 1);

    let changed = cpu_ref_into(
        4,
        &[0, 1, 2, 3, 3],
        &[1, 2, 3],
        &[1, 1, 1],
        &[0],
        0xFFFF_FFFF,
        8,
        &mut frontier,
    );
    assert_eq!(frontier.capacity(), capacity);
    assert_eq!(frontier, vec![0]);
    assert_eq!(changed, 0);
}

#[test]
fn try_cpu_ref_into_with_scratch_reuses_step_storage_and_clears_stale_state() {
    let mut frontier = Vec::with_capacity(8);
    let mut step = Vec::with_capacity(8);
    step.extend_from_slice(&[0xDEAD_BEEF, 0xCAFE_BABE, 0xBADC_0FFE]);
    let mut scratch = PersistentBfsCpuScratch { step };
    let frontier_capacity = frontier.capacity();
    let step_capacity = scratch.step.capacity();

    let changed = try_cpu_ref_into_with_scratch(
        4,
        &[0, 1, 2, 3, 3],
        &[1, 2, 3],
        &[1, 1, 1],
        &[0b0001],
        0xFFFF_FFFF,
        8,
        &mut frontier,
        &mut scratch,
    )
    .expect("Fix: valid persistent BFS chain must run with reusable scratch.");
    assert_eq!(frontier, vec![0b1111]);
    assert_eq!(changed, 1);
    assert_eq!(frontier.capacity(), frontier_capacity);
    assert_eq!(scratch.step.capacity(), step_capacity);
    assert_eq!(scratch.step.len(), 1);

    let changed = try_cpu_ref_into_with_scratch(
        4,
        &[0, 1, 2, 3, 3],
        &[1, 2, 3],
        &[1, 1, 1],
        &[0],
        0xFFFF_FFFF,
        8,
        &mut frontier,
        &mut scratch,
    )
    .expect("Fix: second persistent BFS run must clear stale step bits.");
    assert_eq!(frontier, vec![0]);
    assert_eq!(changed, 0);
    assert_eq!(frontier.capacity(), frontier_capacity);
    assert_eq!(scratch.step.capacity(), step_capacity);
    assert_eq!(
        scratch.step,
        vec![0],
        "Fix: reusable step scratch must be resized to live words and cleared by traversal."
    );
}

#[test]
fn try_cpu_ref_into_rejects_bad_input_without_clobbering_frontier() {
    let mut frontier = vec![0xDEAD_BEEF];
    let capacity = frontier.capacity();

    let err = try_cpu_ref_into(
        4,
        &[0, 1, 2],
        &[1, 2, 3],
        &[1, 1, 1],
        &[0b0001],
        0xFFFF_FFFF,
        8,
        &mut frontier,
    )
    .expect_err("Fix: fallible persistent BFS oracle must reject malformed CSR inputs");

    assert!(err.contains("CSR offsets"));
    assert_eq!(frontier, vec![0xDEAD_BEEF]);
    assert_eq!(frontier.capacity(), capacity);
}

#[test]
fn try_cpu_ref_into_with_scratch_rejects_bad_input_without_clobbering_storage() {
    let mut frontier = vec![0xDEAD_BEEF];
    let mut scratch = PersistentBfsCpuScratch {
        step: vec![0xCAFE_BABE, 0xBADC_0FFE],
    };

    let err = try_cpu_ref_into_with_scratch(
        4,
        &[0, 1, 2],
        &[1, 2, 3],
        &[1, 1, 1],
        &[0b0001],
        0xFFFF_FFFF,
        8,
        &mut frontier,
        &mut scratch,
    )
    .expect_err("Fix: fallible persistent BFS oracle must reject malformed CSR inputs.");

    assert!(err.contains("CSR offsets"));
    assert_eq!(
        frontier,
        vec![0xDEAD_BEEF],
        "Fix: validation failures must not clobber reusable frontier output."
    );
    assert_eq!(
        scratch.step,
        vec![0xCAFE_BABE, 0xBADC_0FFE],
        "Fix: validation failures must not clear reusable step scratch."
    );
}

#[test]
fn fallible_cpu_ref_matches_compatibility_oracle_on_generated_chains() {
    for node_count in [0_u32, 1, 2, 3, 31, 32, 33, 64, 65, 257] {
        let mut offsets = Vec::with_capacity(node_count as usize + 1);
        let mut targets = Vec::new();
        let mut masks = Vec::new();
        offsets.push(0);
        for node in 0..node_count {
            if node + 1 < node_count {
                targets.push(node + 1);
                masks.push(1);
            }
            offsets.push(targets.len() as u32);
        }
        let words = bitset_words(node_count) as usize;
        let mut seed = vec![0; words];
        if node_count != 0 {
            seed[0] = 1;
        }

        let expected = cpu_ref(
            node_count,
            &offsets,
            &targets,
            &masks,
            &seed,
            0xFFFF_FFFF,
            node_count.saturating_add(1),
        );
        let actual = try_cpu_ref(
            node_count,
            &offsets,
            &targets,
            &masks,
            &seed,
            0xFFFF_FFFF,
            node_count.saturating_add(1),
        )
        .expect("Fix: generated valid persistent BFS chain should run fallibly");
        assert_eq!(actual, expected, "node_count={node_count}");
    }
}

#[test]
fn generated_try_cpu_ref_into_with_scratch_matches_allocating_reference() {
    let mut frontier = Vec::new();
    let mut scratch = PersistentBfsCpuScratch::new();

    for case in 0..1024usize {
        let node_count = (case % 67) as u32;
        let mut offsets = Vec::with_capacity(node_count as usize + 1);
        let mut targets = Vec::new();
        let mut masks = Vec::new();
        offsets.push(0);
        for src in 0..node_count {
            for dst in 0..node_count {
                let mixed = case
                    .wrapping_mul(43)
                    .wrapping_add((src as usize).wrapping_mul(17))
                    .wrapping_add((dst as usize).wrapping_mul(29));
                if src != dst && (mixed % 23 == 0 || (case % 19 == 0 && dst == src + 1)) {
                    targets.push(dst);
                    masks.push(if mixed % 2 == 0 { 1 } else { 2 });
                }
            }
            offsets.push(targets.len() as u32);
        }

        let words = bitset_words(node_count) as usize;
        let mut seed = vec![0; words];
        for node in 0..node_count {
            let mixed = case
                .wrapping_mul(11)
                .wrapping_add((node as usize).wrapping_mul(7));
            if mixed % 13 == 0 || (node == 0 && node_count != 0) {
                seed[(node / 32) as usize] |= 1u32 << (node % 32);
            }
        }
        let allow_mask = if case % 3 == 0 { 1 } else { 0xFFFF_FFFF };
        let max_iters = (case % 11) as u32;
        let expected = try_cpu_ref(
            node_count, &offsets, &targets, &masks, &seed, allow_mask, max_iters,
        )
        .expect("Fix: generated persistent BFS graph must be valid for allocating oracle.");
        let changed = try_cpu_ref_into_with_scratch(
            node_count,
            &offsets,
            &targets,
            &masks,
            &seed,
            allow_mask,
            max_iters,
            &mut frontier,
            &mut scratch,
        )
        .expect("Fix: generated persistent BFS graph must run with reusable scratch.");
        assert_eq!(
            (frontier.clone(), changed),
            expected,
            "Fix: scratch-backed persistent BFS diverged from allocating oracle at case {case}."
        );
    }
}
