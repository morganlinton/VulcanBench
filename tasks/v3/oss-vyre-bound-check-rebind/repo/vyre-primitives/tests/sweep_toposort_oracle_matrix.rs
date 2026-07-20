//! Handwritten oracle matrix for `graph::toposort` CSR reference.
//!
//! Compares production LIFO Kahn topological sort against an independent
//! oracle on randomly generated DAGs across thousands of CSR shapes.

#![forbid(unsafe_code)]
#![cfg(feature = "cpu-parity")]

use vyre_primitives::graph::toposort::{
    toposort_csr, toposort_csr_into_with_scratch, validate_toposort_csr_order, ToposortCsrScratch,
};

#[test]
fn toposort_csr_matches_independent_lifo_kahn_oracle_matrix() {
    let mut order = Vec::new();
    let mut scratch = ToposortCsrScratch::new();

    for case in 0..8192usize {
        let (node_count, offsets, targets) = generated_dag_csr(case as u64 ^ 0x7E57_1D00);
        let expected = oracle_lifo_kahn(node_count, &offsets, &targets);
        let actual = toposort_csr(node_count, &offsets, &targets)
            .expect("Fix: generated lower-triangular CSR graph must be a valid DAG.");
        assert_eq!(
            actual, expected,
            "Fix: toposort_csr adversarial case {case} node_count={node_count} must match the independent LIFO Kahn oracle."
        );
        validate_toposort_csr_order(node_count, &offsets, &targets, &actual)
            .expect("Fix: production topological order must satisfy the CSR contract.");

        toposort_csr_into_with_scratch(node_count, &offsets, &targets, &mut order, &mut scratch)
            .expect("Fix: scratch-backed oracle must accept every generated valid DAG.");
        assert_eq!(
            order, expected,
            "Fix: toposort_csr_into_with_scratch adversarial case {case} must match the independent oracle."
        );
    }
}

fn oracle_lifo_kahn(node_count: u32, offsets: &[u32], targets: &[u32]) -> Vec<u32> {
    if node_count == 0 {
        return Vec::new();
    }
    let n = node_count as usize;
    let mut indeg = vec![0u32; n];
    for &target in targets {
        indeg[target as usize] += 1;
    }
    let mut queue = Vec::new();
    for node in 0..node_count {
        if indeg[node as usize] == 0 {
            queue.push(node);
        }
    }
    let mut order = Vec::with_capacity(n);
    while let Some(node) = queue.pop() {
        order.push(node);
        let start = offsets[node as usize] as usize;
        let end = offsets[node as usize + 1] as usize;
        for &dependent in &targets[start..end] {
            let slot = &mut indeg[dependent as usize];
            *slot -= 1;
            if *slot == 0 {
                queue.push(dependent);
            }
        }
    }
    assert_eq!(
        order.len(),
        n,
        "oracle LIFO Kahn expected full permutation for generated DAG"
    );
    order
}

fn generated_dag_csr(seed: u64) -> (u32, Vec<u32>, Vec<u32>) {
    let mut rng = seed;
    let node_count = 1 + (rng as u32 % 96);
    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
    let mut offsets = Vec::with_capacity(node_count as usize + 1);
    let mut targets = Vec::new();
    offsets.push(0);
    for src in 0..node_count {
        let max_dst = node_count.saturating_sub(src + 1);
        let degree = if max_dst == 0 {
            0
        } else {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            rng as u32 % (max_dst.min(5) + 1)
        };
        for _ in 0..degree {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let dst = src + 1 + (rng as u32 % max_dst);
            targets.push(dst);
        }
        offsets.push(targets.len() as u32);
    }
    (node_count, offsets, targets)
}
