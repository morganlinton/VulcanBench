//! Handwritten oracle matrix for `graph::path_reconstruct`.
//!
//! Compares production single-target and batched CPU references against an
//! independent parent-walk oracle across thousands of generated parent trees.

#![forbid(unsafe_code)]
#![cfg(feature = "cpu-parity")]

use vyre_primitives::graph::path_reconstruct;

#[test]
fn path_reconstruct_matches_independent_parent_walk_oracle_matrix() {
    for case in 0..8192usize {
        let seed = case as u64 ^ 0xFACEFE00_00000000;
        let (parent, targets, max_depth) = generated_parent_batch(seed);

        for (index, &target) in targets.iter().enumerate() {
            let (expected_len, expected_path) = oracle_path_reconstruct(&parent, target, max_depth);
            let mut scratch = Vec::new();
            let actual_len = path_reconstruct::cpu_ref(&parent, target, max_depth, &mut scratch);
            assert_eq!(
                actual_len, expected_len,
                "Fix: path_reconstruct length oracle case {case} target_index={index} target={target} must match the independent oracle."
            );
            assert_eq!(
                scratch, expected_path,
                "Fix: path_reconstruct path oracle case {case} target_index={index} target={target} must match the independent oracle."
            );
        }

        let mut batched_paths = Vec::new();
        let mut batched_lens = Vec::new();
        path_reconstruct::cpu_ref_batched(
            &parent,
            &targets,
            max_depth,
            &mut batched_paths,
            &mut batched_lens,
        );
        assert_eq!(
            batched_lens.len(),
            targets.len(),
            "Fix: batched path lens oracle case {case} must emit one length per target."
        );
        assert_eq!(
            batched_paths.len(),
            targets.len() * max_depth as usize,
            "Fix: batched path matrix oracle case {case} must reserve max_depth words per target."
        );

        for (index, &target) in targets.iter().enumerate() {
            let (_, expected_path) = oracle_path_reconstruct(&parent, target, max_depth);
            let start = index * max_depth as usize;
            let end = start + max_depth as usize;
            assert_eq!(
                &batched_paths[start..end],
                expected_path.as_slice(),
                "Fix: batched path segment oracle case {case} target_index={index} target={target} must match the independent oracle."
            );
            assert_eq!(
                batched_lens[index],
                oracle_path_reconstruct(&parent, target, max_depth).0,
                "Fix: batched path length oracle case {case} target_index={index} must match single-target oracle."
            );
        }
    }
}

fn oracle_path_reconstruct(parent: &[u32], target: u32, max_depth: u32) -> (u32, Vec<u32>) {
    let mut path = Vec::new();
    let mut current = target;
    let mut len = 0u32;
    let cap = max_depth as usize;
    while (len as usize) < cap {
        path.push(current);
        len += 1;
        let next = parent.get(current as usize).copied().unwrap_or(current);
        if next == current {
            break;
        }
        current = next;
    }
    while path.len() < cap {
        path.push(0);
    }
    (len, path)
}

fn generated_parent_batch(seed: u64) -> (Vec<u32>, Vec<u32>, u32) {
    let mut rng = seed;
    let len = 1 + next_u32(&mut rng) % 128;
    let mut parent = Vec::with_capacity(len as usize);
    for node in 0..len {
        let p = if node == 0 {
            0
        } else {
            next_u32(&mut rng) % (node + 1)
        };
        parent.push(p);
    }
    let target_count = 1 + next_u32(&mut rng) % 16;
    let mut targets = Vec::with_capacity(target_count as usize);
    for _ in 0..target_count {
        let target = if next_u32(&mut rng) & 15 == 0 {
            len + next_u32(&mut rng) % 8
        } else {
            next_u32(&mut rng) % len
        };
        targets.push(target);
    }
    let max_depth = 1 + next_u32(&mut rng) % 64;
    (parent, targets, max_depth)
}

fn next_u32(rng: &mut u64) -> u32 {
    *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*rng >> 16) as u32
}
