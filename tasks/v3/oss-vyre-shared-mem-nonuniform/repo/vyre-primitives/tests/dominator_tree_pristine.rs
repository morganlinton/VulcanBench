//! Dominator-tree 10-tier pristine pipeline - tiers 1, 2, 4, 5, 6, 9.
//!
//! Tier 1 : Positive truth   → 10+ hand-built CFG fixtures vs LT reference.
//! Tier 2 : Negative precision → empty / single / disconnected degenerates.
//! Tier 4 : Cross-primitive  → csr_forward / csr_backward / dominator_frontier.
//! Tier 5 : GPU vs CPU oracle → reference_eval parity on random graphs.
//! Tier 6 : Edge cases       → irreducible loops, self-loops, cycles, multi-entry.
//! Tier 9 : Differential     → LT vs CHK vs external algorithm path.
#![cfg(feature = "graph")]
#![cfg(feature = "cpu-parity")]

mod common;

use common::reference_eval_idoms;
use vyre_primitives::graph::dominator_frontier::cpu_ref as df_cpu_ref;
use vyre_primitives::graph::dominator_tree::*;

// ------------------------------------------------------------------
// Tier 1 - Positive truth: 10+ canonical CFG fixtures
// ------------------------------------------------------------------

#[test]
fn t1_empty_graph() {
    let idoms = cpu_ref(0, 0, &[]);
    assert!(idoms.is_empty());
}

#[test]
fn t1_single_node() {
    let idoms = cpu_ref(1, 0, &[]);
    assert_eq!(idoms, vec![Some(0)]);
}

#[test]
fn t1_linear_chain_four() {
    // 0 -> 1 -> 2 -> 3
    let idoms = cpu_ref(4, 0, &[(0, 1), (1, 2), (2, 3)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(1));
    assert_eq!(idoms[3], Some(2));
}

#[test]
fn t1_diamond() {
    // 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
    let idoms = cpu_ref(4, 0, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(0));
    assert_eq!(idoms[3], Some(0));
}

#[test]
fn t1_triangle() {
    // 0 -> 1, 1 -> 2, 2 -> 0
    let idoms = cpu_ref(3, 0, &[(0, 1), (1, 2), (2, 0)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(1));
}

#[test]
fn t1_if_then_else() {
    // 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3, 3 -> 4
    let idoms = cpu_ref(5, 0, &[(0, 1), (0, 2), (1, 3), (2, 3), (3, 4)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(0));
    assert_eq!(idoms[3], Some(0));
    assert_eq!(idoms[4], Some(3));
}

#[test]
fn t1_nested_if() {
    // 0 -> 1, 1 -> 2, 1 -> 3, 2 -> 4, 3 -> 4
    let idoms = cpu_ref(5, 0, &[(0, 1), (1, 2), (1, 3), (2, 4), (3, 4)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(1));
    assert_eq!(idoms[3], Some(1));
    assert_eq!(idoms[4], Some(1));
}

#[test]
fn t1_while_loop() {
    // 0 -> 1, 1 -> 2, 2 -> 1, 1 -> 3
    let idoms = cpu_ref(4, 0, &[(0, 1), (1, 2), (2, 1), (1, 3)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(1));
    assert_eq!(idoms[3], Some(1));
}

#[test]
fn t1_do_while_loop() {
    // 0 -> 1, 1 -> 2, 2 -> 1
    let idoms = cpu_ref(3, 0, &[(0, 1), (1, 2), (2, 1)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(1));
}

#[test]
fn t1_switch_with_fallthrough() {
    // 0 -> 1, 0 -> 2, 0 -> 3, 1 -> 4, 2 -> 4, 3 -> 4
    let idoms = cpu_ref(5, 0, &[(0, 1), (0, 2), (0, 3), (1, 4), (2, 4), (3, 4)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(0));
    assert_eq!(idoms[3], Some(0));
    assert_eq!(idoms[4], Some(0));
}

#[test]
fn t1_irreducible_loop() {
    // Classic irreducible: 0 -> 1, 0 -> 2, 1 -> 2, 2 -> 1
    let idoms = cpu_ref(3, 0, &[(0, 1), (0, 2), (1, 2), (2, 1)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(0));
}

#[test]
fn t1_irreducible_with_header() {
    // 0 -> 1, 1 -> 2, 1 -> 3, 2 -> 3, 3 -> 2, 2 -> 4, 3 -> 4
    let idoms = cpu_ref(
        5,
        0,
        &[(0, 1), (1, 2), (1, 3), (2, 3), (3, 2), (2, 4), (3, 4)],
    );
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(1));
    assert_eq!(idoms[3], Some(1));
    assert_eq!(idoms[4], Some(1));
}

#[test]
fn t1_deep_chain() {
    // 0 -> 1 -> 2 -> ... -> 99
    let edges: Vec<(u32, u32)> = (0..99).map(|i| (i, i + 1)).collect();
    let idoms = cpu_ref(100, 0, &edges);
    assert_eq!(idoms[0], Some(0));
    for i in 1..100 {
        assert_eq!(
            idoms[i],
            Some((i - 1) as u32),
            "idom[{i}] should be {}",
            i - 1
        );
    }
}

// ------------------------------------------------------------------
// Tier 2 - Negative precision: degenerate behaviour
// ------------------------------------------------------------------

#[test]
fn t2_empty_graph_all_none() {
    let idoms = cpu_ref(0, 0, &[]);
    assert!(idoms.is_empty());
}

#[test]
fn t2_single_node_no_edges_self_idom() {
    let idoms = cpu_ref(1, 0, &[]);
    assert_eq!(idoms, vec![Some(0)]);
}

#[test]
fn t2_two_nodes_no_edges_entry_zero() {
    // Node 1 is disconnected from entry 0.
    let idoms = cpu_ref(2, 0, &[]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], None);
}

#[test]
fn t2_disconnected_component_preserves_none() {
    // Component A: 0 -> 1. Component B: 2 -> 3.
    let idoms = cpu_ref(4, 0, &[(0, 1), (2, 3)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], None);
    assert_eq!(idoms[3], None);
}

#[test]
fn t2_entry_out_of_range_returns_all_none() {
    let idoms = cpu_ref(3, 5, &[(0, 1)]);
    assert!(idoms.iter().all(|x| x.is_none()));
}

#[test]
fn t2_self_loop_single_node() {
    let idoms = cpu_ref(1, 0, &[(0, 0)]);
    assert_eq!(idoms[0], Some(0));
}

// ------------------------------------------------------------------
// Tier 4 - Cross-primitive: data flow with csr_forward / backward / df
// ------------------------------------------------------------------

#[test]
fn t4_dominator_tree_idoms_feed_dominator_frontier() {
    // CFG: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3.
    // idoms: 0->0, 1->0, 2->0, 3->0.
    // Dominator sets (closure by dominator):
    //   0: {0,1,2,3}, 1: {1}, 2: {2}, 3: {3}
    // Predecessors:
    //   0: {}, 1: {0}, 2: {0}, 3: {1,2}
    // DF(1): 3 because 1 dominates pred 1 of 3, but 1 does not strictly dominate 3.
    let idoms = cpu_ref(4, 0, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
    let dom_sets = idoms_to_dominator_sets(&idoms, 4);

    // Build dominance-closure CSR from idoms.
    // Row n contains every node dominated by n (including n).
    let mut dom_offsets = vec![0u32; 5];
    let mut dom_targets: Vec<u32> = Vec::new();
    for n in 0..4 {
        for m in 0..4 {
            if dom_sets[m].contains(&n) {
                dom_targets.push(m as u32);
            }
        }
        dom_offsets[n as usize + 1] = dom_targets.len() as u32;
    }

    let pred_offsets = vec![0u32, 0, 1, 2, 4];
    let pred_targets = vec![0u32, 0, 1, 2];

    let df = df_cpu_ref(
        4,
        &dom_offsets,
        &dom_targets,
        &pred_offsets,
        &pred_targets,
        &[0b0010], // seed = {1}
    );
    assert_eq!(df, vec![0b1000], "DF(1) must be {{3}}");
}

#[test]
fn t4_idom_tree_to_forward_csr_roundtrip() {
    // 0 -> 1 -> 2, 0 -> 3 -> 2
    let idoms = cpu_ref(4, 0, &[(0, 1), (0, 3), (1, 2), (3, 2)]);
    assert_eq!(idoms[2], Some(0)); // join node

    // Walk idom tree: children of 0 are {1, 2, 3}
    let mut children: Vec<Vec<u32>> = vec![Vec::new(); 4];
    for v in 0..4 {
        if let Some(p) = idoms[v] {
            if p != v as u32 {
                children[p as usize].push(v as u32);
            }
        }
    }
    assert!(children[0].contains(&1));
    assert!(children[0].contains(&2));
    assert!(children[0].contains(&3));
}

// ------------------------------------------------------------------
// Tier 5 - GPU vs CPU oracle: reference_eval parity
// ------------------------------------------------------------------

fn gpu_idoms_via_reference(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
) -> Vec<Option<u32>> {
    let program = dominator_tree_program(
        node_count,
        edge_targets.len() as u32,
        pred_targets.len() as u32,
        "idom",
    );
    let raw = reference_eval_idoms(
        &program,
        node_count,
        edge_offsets,
        edge_targets,
        pred_offsets,
        pred_targets,
    );
    raw.into_iter()
        .map(|v| if v == IDOM_NONE { None } else { Some(v) })
        .collect()
}

#[test]
fn t5_gpu_parity_linear_chain() {
    let n = 4;
    let edge_offsets = vec![0u32, 1, 2, 3, 3];
    let edge_targets = vec![1u32, 2, 3];
    let pred_offsets = vec![0u32, 0, 1, 2, 3];
    let pred_targets = vec![0u32, 1, 2];
    let cpu = cpu_ref(n, 0, &[(0, 1), (1, 2), (2, 3)]);
    let gpu = gpu_idoms_via_reference(
        n,
        &edge_offsets,
        &edge_targets,
        &pred_offsets,
        &pred_targets,
    );
    assert_eq!(cpu, gpu, "GPU parity failure on linear chain");
}

#[test]
fn t5_gpu_parity_diamond() {
    let n = 4;
    let edge_offsets = vec![0u32, 2, 3, 4, 4];
    let edge_targets = vec![1u32, 2, 3, 3];
    let pred_offsets = vec![0u32, 0, 1, 2, 4];
    let pred_targets = vec![0u32, 0, 1, 2];
    let cpu = cpu_ref(n, 0, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
    let gpu = gpu_idoms_via_reference(
        n,
        &edge_offsets,
        &edge_targets,
        &pred_offsets,
        &pred_targets,
    );
    assert_eq!(cpu, gpu, "GPU parity failure on diamond");
}

#[test]
fn t5_gpu_parity_while_loop() {
    let n = 4;
    let edge_offsets = vec![0u32, 1, 3, 3, 3];
    let edge_targets = vec![1u32, 2, 3];
    let pred_offsets = vec![0u32, 0, 1, 2, 3];
    let pred_targets = vec![0u32, 1, 2];
    let cpu = cpu_ref(n, 0, &[(0, 1), (1, 2), (2, 3)]);
    let gpu = gpu_idoms_via_reference(
        n,
        &edge_offsets,
        &edge_targets,
        &pred_offsets,
        &pred_targets,
    );
    assert_eq!(cpu, gpu, "GPU parity failure on while-loop-like chain");
}

#[test]
fn t5_gpu_parity_irreducible() {
    let n = 3;
    // 0->1, 0->2, 1->2, 2->1
    let edge_offsets = vec![0u32, 2, 3, 4];
    let edge_targets = vec![1u32, 2, 2, 1];
    // preds(0)={}, preds(1)={0,2}, preds(2)={0,1}
    let pred_offsets = vec![0u32, 0, 2, 4];
    let pred_targets = vec![0u32, 2, 0, 1];
    let cpu = cpu_ref(n, 0, &[(0, 1), (0, 2), (1, 2), (2, 1)]);
    let gpu = gpu_idoms_via_reference(
        n,
        &edge_offsets,
        &edge_targets,
        &pred_offsets,
        &pred_targets,
    );
    assert_eq!(cpu, gpu, "GPU parity failure on irreducible loop");
}

// ------------------------------------------------------------------
// Tier 6 - Edge cases: irreducible, multi-entry, self-loops, cycles
// ------------------------------------------------------------------

#[test]
fn t6_irreducible_two_entry_loop() {
    // 0 -> 1, 0 -> 2, 1 -> 2, 2 -> 1
    let idoms = cpu_ref(3, 0, &[(0, 1), (0, 2), (1, 2), (2, 1)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(0));
}

#[test]
fn t6_multi_entry_cfg_is_not_rejected_but_undefined() {
    // Graph has two entry-like nodes: 0 and 1 both have no preds.
    // Entry is declared as 0, but 1 is unreachable from 0.
    let idoms = cpu_ref(3, 0, &[(1, 2)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], None); // unreachable from entry 0
    assert_eq!(idoms[2], None); // unreachable from entry 0
}

#[test]
fn t6_self_loop_non_entry() {
    // 0 -> 1, 1 -> 1
    let idoms = cpu_ref(2, 0, &[(0, 1), (1, 1)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
}

#[test]
fn t6_cycle_of_three() {
    // 0 -> 1, 1 -> 2, 2 -> 0
    let idoms = cpu_ref(3, 0, &[(0, 1), (1, 2), (2, 0)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(1));
}

#[test]
fn t6_multiple_edges_between_same_nodes() {
    // 0 -> 1 (twice), 1 -> 2
    let idoms = cpu_ref(3, 0, &[(0, 1), (0, 1), (1, 2)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(1));
}

#[test]
fn t6_complete_graph_three_nodes() {
    // Every node -> every other node
    let edges = vec![(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)];
    let idoms = cpu_ref(3, 0, &edges);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(0));
}

// ------------------------------------------------------------------
// Tier 9 - Differential: LT vs CHK (external algorithm path)
// ------------------------------------------------------------------

#[test]

fn t9_differential_lt_vs_chk_all_fixtures() {
    let fixtures: Vec<(u32, u32, Vec<(u32, u32)>)> = vec![
        (0, 0, vec![]),
        (1, 0, vec![]),
        (2, 0, vec![(0, 1)]),
        (4, 0, vec![(0, 1), (1, 2), (2, 3)]),
        (4, 0, vec![(0, 1), (0, 2), (1, 3), (2, 3)]),
        (4, 0, vec![(0, 1), (1, 2), (2, 1), (1, 3)]),
        (3, 0, vec![(0, 1), (0, 2), (1, 2), (2, 1)]),
        (5, 0, vec![(0, 1), (1, 2), (1, 3), (2, 4), (3, 4)]),
        (
            5,
            0,
            vec![(0, 1), (1, 2), (1, 3), (2, 3), (3, 2), (2, 4), (3, 4)],
        ),
    ];

    for (n, entry, edges) in fixtures {
        let lt = lengauer_tarjan_idoms(n, entry, &edges);
        let chk = cooper_harvey_kennedy_idoms(n, entry, &edges);
        assert_eq!(
            lt, chk,
            "LT/CHK differential failed for n={n} entry={entry} edges={edges:?}"
        );
    }
}

#[test]
fn t9_differential_deterministic_stress() {
    // Deterministic stress using a simple LCG so no external rand dep.
    let mut state: u64 = 0x1234_5678_9abc_def0;
    for _ in 0..50 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let n = ((state >> 32) % 30 + 2) as u32; // 2..31
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let e = ((state >> 32) % (n as u64 * 3).max(2)) as u32;
        let mut edges = Vec::new();
        for _ in 0..e {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u = ((state >> 32) % n as u64) as u32;
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let v = ((state >> 32) % n as u64) as u32;
            edges.push((u, v));
        }
        let lt = lengauer_tarjan_idoms(n, 0, &edges);
        let chk = cooper_harvey_kennedy_idoms(n, 0, &edges);
        assert_eq!(lt, chk, "LT/CHK mismatch on stress graph n={n}");
    }
}
