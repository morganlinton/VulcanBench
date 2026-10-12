//! Parity tests for math::scallop_join, math::scallop_join_wide,
//! and graph::ddnnf_evaluate (single-node bottom-up step).

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_primitives::graph::knowledge_compile::{
    ddnnf_evaluate, ddnnf_evaluate_cpu, ddnnf_evaluate_dispatch_grid, LITERAL_FALSE, LITERAL_TRUE,
};
use vyre_primitives::math::scallop_join::{
    cpu_ref as scallop_cpu, scallop_join, scallop_join_dispatch_grid,
};
use vyre_primitives::math::scallop_join_wide::{
    cpu_ref as scallop_wide_cpu, scallop_join_wide, scallop_join_wide_dispatch_grid,
};

// ---------------------------------------------------------------------
// scallop_join (single-word lineage). Iterates Datalog fixpoint inside
// one dispatch via persistent_fixpoint + semiring_gemm Lineage.
// ---------------------------------------------------------------------

fn run_scallop_join(
    backend: &CudaBackend,
    state: &[u32],
    join_rules: &[u32],
    n: u32,
    max_iters: u32,
) -> Vec<u32> {
    let words = (n * n) as usize;
    let program = scallop_join("state", "next", "join_rules", "changed", n, max_iters);
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(state),
        vec![0u8; words * 4],
        vec![0u8; 4],
        u32_bytes(join_rules),
    ];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(scallop_join_dispatch_grid(n));
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(words);
    out
}

#[test]
fn cuda_scallop_join_high_cell_chain_converges() {
    with_live_backend("cuda_scallop_join_high_cell_chain_converges", |backend| {
        let n = 17u32;
        let words = (n * n) as usize;
        let mut state = vec![0u32; words];
        let mut join_rules = vec![0u32; words];
        state[(0 * n + 1) as usize] = 0b0001;
        join_rules[(1 * n + 16) as usize] = 0b0010;

        let (cpu, _iters) = scallop_cpu(&state, &join_rules, n, 4);
        let gpu = run_scallop_join(backend, &state, &join_rules, n, 4);

        assert_eq!(scallop_join_dispatch_grid(n), [2, 1, 1]);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu[(0 * n + 16) as usize] & 0b0011, 0b0011);
    });
}

#[test]
fn cuda_scallop_join_two_node_chain_converges() {
    with_live_backend("cuda_scallop_join_two_node_chain_converges", |backend| {
        // 2 relations, lineage clause-bit 0 propagates from (0,1) via join.
        let n = 2u32;
        // state[i,j] = clause bitset for direct edge.
        // state[0,1] = {clause 0}, no other edges.
        let mut state = vec![0u32; (n * n) as usize];
        state[1] = 1; // (i=0, j=1) → clause bit 0
                      // join_rules[0,1] = {clause 0}: derive (i,1) from (i,0) under clause 0.
        let mut join_rules = vec![0u32; (n * n) as usize];
        join_rules[1] = 1;
        let (cpu, _iters) = scallop_cpu(&state, &join_rules, n, 8);
        let gpu = run_scallop_join(backend, &state, &join_rules, n, 8);
        assert_eq!(gpu, cpu);
    });
}

#[test]
fn cuda_scallop_join_zero_state_stays_zero() {
    with_live_backend("cuda_scallop_join_zero_state_stays_zero", |backend| {
        let n = 3u32;
        let state = vec![0u32; (n * n) as usize];
        let join_rules = vec![0xFu32; (n * n) as usize];
        let (cpu, _) = scallop_cpu(&state, &join_rules, n, 4);
        let gpu = run_scallop_join(backend, &state, &join_rules, n, 4);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu, vec![0u32; (n * n) as usize]);
    });
}

// ---------------------------------------------------------------------
// scallop_join_wide (W-word lineage).
// ---------------------------------------------------------------------

fn run_scallop_join_wide(
    backend: &CudaBackend,
    state: &[u32],
    join_rules: &[u32],
    n: u32,
    w: u32,
    max_iters: u32,
) -> Vec<u32> {
    let words = (n * n * w) as usize;
    let program = scallop_join_wide("state", "next", "join_rules", "changed", n, w, max_iters);
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(state),
        vec![0u8; words * 4],
        vec![0u8; 4],
        u32_bytes(join_rules),
    ];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(scallop_join_wide_dispatch_grid(n, w));
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(words);
    out
}

#[test]
fn cuda_scallop_join_wide_basic() {
    with_live_backend("cuda_scallop_join_wide_basic", |backend| {
        let n = 2u32;
        let w = 2u32;
        let words = (n * n * w) as usize;
        let mut state = vec![0u32; words];
        state[(w) as usize] = 1;
        let mut join_rules = vec![0u32; words];
        join_rules[(w) as usize] = 1;
        let (cpu, _iters) = scallop_wide_cpu(&state, &join_rules, n, w, 8);
        let gpu = run_scallop_join_wide(backend, &state, &join_rules, n, w, 8);
        assert_eq!(gpu, cpu);
    });
}

#[test]
fn cuda_scallop_join_wide_copies_high_words_past_cell_lane_count() {
    with_live_backend(
        "cuda_scallop_join_wide_copies_high_words_past_cell_lane_count",
        |backend| {
            let n = 17u32;
            let w = 2u32;
            let words = (n * n * w) as usize;
            let mut state = vec![0u32; words];
            let mut join_rules = vec![0u32; words];
            let cell_word = |row: u32, col: u32, word: u32| ((row * n + col) * w + word) as usize;

            state[cell_word(16, 0, 0)] = 0b0001;
            join_rules[cell_word(0, 16, 1)] = 0b0010;

            let (cpu, _iters) = scallop_wide_cpu(&state, &join_rules, n, w, 4);
            let gpu = run_scallop_join_wide(backend, &state, &join_rules, n, w, 4);

            assert_eq!(scallop_join_wide_dispatch_grid(n, w), [2, 1, 1]);
            assert_eq!(gpu, cpu);
            assert_eq!(gpu[cell_word(16, 16, 0)] & 0b0001, 0b0001);
            assert_eq!(gpu[cell_word(16, 16, 1)] & 0b0010, 0b0010);
        },
    );
}

// ---------------------------------------------------------------------
// ddnnf_evaluate (single bottom-up step). To stay race-free we evaluate
// a single-level circuit (literals only). Multi-level evaluation needs
// level_wave wrapping; we leave that path to the dedicated bench.
// ---------------------------------------------------------------------

fn run_ddnnf(
    backend: &CudaBackend,
    node_kinds: &[u32],
    node_var: &[u32],
    child_offsets: &[u32],
    child_counts: &[u32],
    children: &[u32],
    var_assignments: &[u32],
) -> Vec<u32> {
    let n_nodes = node_kinds.len() as u32;
    let n_children = children.len().max(1) as u32;
    let n_vars = var_assignments.len() as u32;
    let program = ddnnf_evaluate(
        "node_kinds",
        "node_var",
        "child_offsets",
        "child_counts",
        "children",
        "var_assignments",
        "out",
        n_nodes,
        n_children,
        n_vars,
    );
    let children_padded = if children.is_empty() {
        vec![0u32]
    } else {
        children.to_vec()
    };
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(node_kinds),
        u32_bytes(node_var),
        u32_bytes(child_offsets),
        u32_bytes(child_counts),
        u32_bytes(&children_padded),
        u32_bytes(var_assignments),
        vec![0u8; n_nodes as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(ddnnf_evaluate_dispatch_grid(n_nodes));
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(n_nodes as usize);
    out
}

#[test]
fn cuda_ddnnf_literal_with_var_assigned_true() {
    with_live_backend("cuda_ddnnf_literal_with_var_assigned_true", |backend| {
        // One literal-true node referencing var 0 = 1 → out = 1.
        let node_kinds = vec![LITERAL_TRUE];
        let node_var = vec![0u32];
        let child_offsets = vec![0u32];
        let child_counts = vec![0u32];
        let children: Vec<u32> = vec![];
        let var_assignments = vec![1u32];
        let nodes_cpu: Vec<(u32, u32, u32)> = vec![(LITERAL_TRUE, 0, 0)];
        let cpu = ddnnf_evaluate_cpu(&nodes_cpu, &node_var, &children, &var_assignments, &[0]);
        let gpu = run_ddnnf(
            backend,
            &node_kinds,
            &node_var,
            &child_offsets,
            &child_counts,
            &children,
            &var_assignments,
        );
        assert_eq!(gpu, cpu);
        assert_eq!(gpu[0], 1);
    });
}

#[test]
fn cuda_ddnnf_literal_with_var_assigned_false() {
    with_live_backend("cuda_ddnnf_literal_with_var_assigned_false", |backend| {
        let node_kinds = vec![LITERAL_TRUE];
        let node_var = vec![0u32];
        let child_offsets = vec![0u32];
        let child_counts = vec![0u32];
        let children: Vec<u32> = vec![];
        // Variable assigned = 0  -  the literal-true is unsatisfied.
        let var_assignments = vec![0u32];
        let nodes_cpu: Vec<(u32, u32, u32)> = vec![(LITERAL_TRUE, 0, 0)];
        let cpu = ddnnf_evaluate_cpu(&nodes_cpu, &node_var, &children, &var_assignments, &[0]);
        let gpu = run_ddnnf(
            backend,
            &node_kinds,
            &node_var,
            &child_offsets,
            &child_counts,
            &children,
            &var_assignments,
        );
        assert_eq!(gpu, cpu);
        assert_eq!(gpu[0], 0);
    });
}

#[test]
fn cuda_ddnnf_literal_wave_crosses_workgroup_boundaries() {
    with_live_backend(
        "cuda_ddnnf_literal_wave_crosses_workgroup_boundaries",
        |backend| {
            let n_nodes = 1025u32;
            let n_vars = 97u32;
            let node_kinds: Vec<u32> = (0..n_nodes)
                .map(|idx| {
                    if idx % 2 == 0 {
                        LITERAL_TRUE
                    } else {
                        LITERAL_FALSE
                    }
                })
                .collect();
            let node_var: Vec<u32> = (0..n_nodes).map(|idx| idx % n_vars).collect();
            let child_offsets = vec![0u32; n_nodes as usize];
            let child_counts = vec![0u32; n_nodes as usize];
            let children: Vec<u32> = vec![];
            let var_assignments: Vec<u32> = (0..n_vars)
                .map(|idx| match idx % 3 {
                    0 => 0,
                    1 => 1,
                    _ => u32::MAX,
                })
                .collect();
            let nodes_cpu: Vec<(u32, u32, u32)> = node_kinds
                .iter()
                .copied()
                .map(|kind| (kind, 0, 0))
                .collect();
            let topo_order: Vec<u32> = (0..n_nodes).collect();

            let cpu = ddnnf_evaluate_cpu(
                &nodes_cpu,
                &node_var,
                &children,
                &var_assignments,
                &topo_order,
            );
            let gpu = run_ddnnf(
                backend,
                &node_kinds,
                &node_var,
                &child_offsets,
                &child_counts,
                &children,
                &var_assignments,
            );

            assert_eq!(ddnnf_evaluate_dispatch_grid(n_nodes), [5, 1, 1]);
            assert_eq!(gpu, cpu);
            assert_eq!(gpu[0], 0);
            assert_eq!(gpu[1], 0);
            assert_eq!(gpu[2], 1);
            assert_eq!(gpu[256], cpu[256]);
            assert_eq!(gpu[512], cpu[512]);
            assert_eq!(gpu[1024], cpu[1024]);
        },
    );
}
