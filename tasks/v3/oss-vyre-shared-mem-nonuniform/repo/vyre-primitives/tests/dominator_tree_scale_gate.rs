//! Tier 8 - Scale gate: 10M+ node graph, no OOM, no panic, completes within budget.
#![cfg(feature = "graph")]
#![cfg(feature = "cpu-parity")]

use std::time::Instant;
use vyre_primitives::graph::dominator_tree::{
    cooper_harvey_kennedy_idoms, cpu_ref, try_dominator_tree_program,
};

/// Build a linear chain of `n` nodes: 0 -> 1 -> 2 -> ... -> n-1.
fn linear_chain_edges(n: u32) -> Vec<(u32, u32)> {
    (0..n.saturating_sub(1)).map(|i| (i, i + 1)).collect()
}

/// Build a sparse random-like graph with `n` nodes and ~2n edges.
fn sparse_random_edges(n: u32) -> Vec<(u32, u32)> {
    let mut edges = Vec::new();
    for i in 0..n.saturating_sub(1) {
        edges.push((i, i + 1));
        if i + 2 < n {
            edges.push((i, i + 2));
        }
    }
    edges
}

#[test]
fn scale_gate_10m_nodes_linear_chain_cpu_ref() {
    let n = 10_000_000u32;
    let edges = linear_chain_edges(n);

    let start = Instant::now();
    let idoms = cpu_ref(n, 0, &edges);
    let elapsed = start.elapsed();

    assert_eq!(idoms.len(), n as usize);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[n as usize - 1], Some(n - 2));

    // Budget: 60 seconds on host for 10M linear chain.
    assert!(
        elapsed.as_secs() < 60,
        "10M linear chain took {:?}, exceeding 60s budget",
        elapsed
    );
}

#[test]
fn scale_gate_50k_nodes_sparse_random_chk() {
    // CHK uses dense bitsets (n * ceil(n/32) * 4 bytes).
    // 10 M nodes would need ~12.5 TB, so we gate CHK at 50 K (~312 MB).
    let n = 50_000u32;
    let edges = sparse_random_edges(n);

    let start = Instant::now();
    let idoms = cooper_harvey_kennedy_idoms(n, 0, &edges);
    let elapsed = start.elapsed();

    assert_eq!(idoms.len(), n as usize);
    assert_eq!(idoms[0], Some(0));

    // Budget: 60 seconds on host for 50 K sparse graph (debug builds are slow).
    assert!(
        elapsed.as_secs() < 60,
        "50 K sparse random CHK took {:?}, exceeding 60 s budget",
        elapsed
    );
}

#[test]
fn scale_gate_program_builds_for_10m_without_panic() {
    let n = 10_000_000u32;
    let e = n * 2;
    let p = try_dominator_tree_program(n, e, e, "idom").expect("10M-node program must build");
    assert_eq!(
        p.buffers().len(),
        6,
        "dominator tree program declares six buffers: four CSR inputs \
         (pg_edge_offsets, pg_edge_targets, pred_offsets, pred_targets) plus the \
         idom_out result and the depth scratch buffer (Cooper-Harvey-Kennedy \
         depth-based intersection); both program builders declare exactly these six"
    );
}

#[test]
fn scale_gate_1m_nodes_linear_chain_cpu_ref() {
    let n = 1_000_000u32;
    let edges = linear_chain_edges(n);

    let start = Instant::now();
    let idoms = cpu_ref(n, 0, &edges);
    let elapsed = start.elapsed();

    assert_eq!(idoms.len(), n as usize);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[n as usize - 1], Some(n - 2));

    // Budget: 5 seconds for 1M.
    assert!(
        elapsed.as_secs() < 5,
        "1M linear chain took {:?}, exceeding 5s budget",
        elapsed
    );
}
