//! Generated live CUDA parity for resident adaptive sparse-queue traversal.

#![cfg(test)]

mod common;

use common::live_dispatcher;
use vyre_driver_cuda::{CudaBackend, CudaOptimizerDispatcher};
use vyre_self_substrate::adaptive_traverse::{
    adaptive_traverse_resident_sparse_queue_step_with_scratch_into,
    upload_resident_adaptive_sparse_queue_graph, AdaptiveTraversalPlanCacheSnapshot,
    AdaptiveTraversalResidentScratch,
};

const GENERATED_CASES: u32 = 1024;
const ALLOW_MASKS: [u32; 8] = [0, 1, 2, 4, 3, 5, 6, 7];

#[test]
fn generated_resident_adaptive_sparse_queue_atomic_matrix_matches_csr_oracle_on_live_cuda() {
    let backend = live_dispatcher();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    run_generated_sparse_queue_matrix(&backend, &dispatcher, 512, "atomic-node-scan", 3);
}

#[test]
fn generated_resident_adaptive_sparse_queue_word_prefix_matrix_matches_csr_oracle_on_live_cuda() {
    let backend = live_dispatcher();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    run_generated_sparse_queue_matrix(&backend, &dispatcher, 8_193, "word-prefix", 4);
}

fn run_generated_sparse_queue_matrix(
    backend: &CudaBackend,
    dispatcher: &CudaOptimizerDispatcher<'_>,
    node_count: u32,
    materializer_name: &str,
    kernels_per_case: u64,
) {
    let (edge_offsets, edge_targets, edge_kind_mask) = generated_sparse_graph(node_count);
    let graph = upload_resident_adaptive_sparse_queue_graph(
        dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .unwrap_or_else(|error| {
        panic!("Fix: generated {materializer_name} sparse queue graph upload failed: {error}")
    });
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut out = Vec::new();

    backend.reset_telemetry();
    for case_index in 0..GENERATED_CASES {
        let frontier_in = generated_frontier(node_count, case_index);
        let allow_mask = ALLOW_MASKS[(case_index as usize) % ALLOW_MASKS.len()];
        let expected = csr_sparse_queue_oracle(
            &frontier_in,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            node_count,
            allow_mask,
        );

        adaptive_traverse_resident_sparse_queue_step_with_scratch_into(
            dispatcher,
            &graph,
            &frontier_in,
            allow_mask,
            &mut scratch,
            &mut out,
        )
        .unwrap_or_else(|error| {
            panic!(
                "Fix: generated {materializer_name} sparse queue case {case_index} failed: {error}"
            )
        });
        assert_eq!(
            out, expected,
            "Fix: generated {materializer_name} sparse queue case {case_index} diverged from CSR oracle for allow_mask={allow_mask:#x}."
        );
    }

    let snapshot = scratch.plan_cache_snapshot();
    let expected_entries = kernels_per_case as usize - 1 + ALLOW_MASKS.len();
    assert_eq!(
        snapshot,
        AdaptiveTraversalPlanCacheSnapshot {
            entries: expected_entries,
            hits: u64::from(GENERATED_CASES) * kernels_per_case - expected_entries as u64,
            misses: expected_entries as u64,
        },
        "Fix: generated {materializer_name} sparse queue matrix must reuse resident Programs across frontier changes."
    );
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches,
        u64::from(GENERATED_CASES) * kernels_per_case,
        "Fix: generated {materializer_name} sparse queue matrix must keep the release path at {kernels_per_case} kernels per resident step."
    );
    assert_eq!(
        telemetry.sync_points,
        u64::from(GENERATED_CASES),
        "Fix: generated {materializer_name} sparse queue matrix must fence once per upload/dispatch/readback sequence."
    );
    let expected_frontier_bytes =
        u64::from(GENERATED_CASES) * u64::from(bitset_words(node_count)) * 4;
    assert_eq!(
        telemetry
            .host_to_device_bytes
            .saturating_sub(telemetry.param_upload_bytes),
        expected_frontier_bytes,
        "Fix: generated {materializer_name} sparse queue matrix must upload only frontier payloads after graph residency."
    );
    assert_eq!(
        telemetry.readback_bytes, expected_frontier_bytes,
        "Fix: generated {materializer_name} sparse queue matrix must read back only frontier_out."
    );

    scratch
        .free(dispatcher)
        .unwrap_or_else(|error| panic!("Fix: generated {materializer_name} scratch free: {error}"));
    graph
        .free(dispatcher)
        .unwrap_or_else(|error| panic!("Fix: generated {materializer_name} graph free: {error}"));
}

fn bitset_words(node_count: u32) -> u32 {
    node_count.div_ceil(32).max(1)
}

fn generated_sparse_graph(node_count: u32) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::new();
    let mut edge_kind_mask = Vec::new();
    edge_offsets.push(0);
    for src in 0..node_count {
        if src % 5 != 0 {
            edge_targets.push(src.wrapping_mul(17).wrapping_add(13) % node_count);
            edge_kind_mask.push(1);
        }
        if src % 7 == 0 {
            edge_targets.push(src.wrapping_mul(31).wrapping_add(5) % node_count);
            edge_kind_mask.push(2);
        }
        if (src ^ node_count) % 11 == 0 {
            edge_targets.push(src.wrapping_mul(47).wrapping_add(29) % node_count);
            edge_kind_mask.push(4);
        }
        edge_offsets.push(edge_targets.len() as u32);
    }
    (edge_offsets, edge_targets, edge_kind_mask)
}

fn generated_frontier(node_count: u32, case_index: u32) -> Vec<u32> {
    let mut frontier = vec![0_u32; bitset_words(node_count) as usize];
    let salt = mix32(case_index ^ node_count.rotate_left(7));
    let period = 19 + (case_index % 29);
    for node in 0..node_count {
        let h = mix32(node.wrapping_mul(0x9e37_79b1) ^ salt);
        if h % period == 0 {
            set_node(&mut frontier, node);
        }
    }
    set_node(&mut frontier, salt % node_count);
    if case_index % 16 == 0 {
        set_node(&mut frontier, 0);
    }
    if case_index % 31 == 0 {
        set_node(&mut frontier, node_count - 1);
    }
    if node_count > 32 && case_index % 17 == 0 {
        set_node(&mut frontier, 32);
    }
    frontier
}

fn csr_sparse_queue_oracle(
    frontier_in: &[u32],
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    node_count: u32,
    allow_mask: u32,
) -> Vec<u32> {
    let mut out = vec![0_u32; bitset_words(node_count) as usize];
    for src in 0..node_count {
        if !frontier_has_node(frontier_in, src) {
            continue;
        }
        let edge_start = edge_offsets[src as usize] as usize;
        let edge_end = edge_offsets[src as usize + 1] as usize;
        for edge_index in edge_start..edge_end {
            if edge_kind_mask[edge_index] & allow_mask != 0 {
                set_node(&mut out, edge_targets[edge_index]);
            }
        }
    }
    out
}

fn frontier_has_node(frontier: &[u32], node: u32) -> bool {
    frontier[node as usize / 32] & (1_u32 << (node & 31)) != 0
}

fn set_node(frontier: &mut [u32], node: u32) {
    frontier[node as usize / 32] |= 1_u32 << (node & 31);
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}
