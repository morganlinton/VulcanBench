//! CUDA parity for device-side active-frontier queue sparse traversal.

#![cfg(test)]


mod common;
#[path = "csr_frontier_queue_gpu_parity/manual_sequence_contracts.rs"]
mod manual_sequence_contracts;
#[path = "csr_frontier_queue_gpu_parity/delta_contracts.rs"]
mod delta_contracts;
#[path = "csr_frontier_queue_gpu_parity/resident_graph_contracts.rs"]
mod resident_graph_contracts;
#[path = "csr_frontier_queue_gpu_parity/batch_contracts.rs"]
mod batch_contracts;

use common::{bytes_u32, live_backend, u32_bytes};
use vyre_driver_cuda::CudaOptimizerDispatcher;
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::csr_frontier_queue::{
    csr_queue_forward_traverse, csr_queue_forward_traverse_cpu, frontier_to_queue,
    frontier_to_queue_cpu, frontier_to_queue_parallel,
};
use vyre_primitives::graph::csr_queue_delta::{
    csr_queue_delta_enqueue, csr_queue_delta_enqueue_cpu, csr_queue_delta_strided_dispatch_grid,
    csr_queue_delta_strided_enqueue,
};
use vyre_primitives::graph::csr_queue_split::CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD;
use vyre_self_substrate::csr_frontier_queue_batch_resident::{
    run_resident_csr_queue_batch_budgeted_into, run_resident_csr_queue_batch_into,
    ResidentCsrQueueBatchScratch,
};
use vyre_self_substrate::csr_frontier_queue_resident::{
    run_resident_csr_queue_query_into, upload_resident_csr_queue_graph, ResidentCsrQueueScratch,
};
use vyre_self_substrate::optimizer::dispatcher::{
    OptimizerDispatcher, ResidentDispatchStep, ResidentReadRange,
};

fn pack_nodes(bits: &[u32], node_count: u32) -> Vec<u32> {
    let mut out = vec![0u32; bitset_words(node_count) as usize];
    for &bit in bits {
        out[bit as usize / 32] |= 1u32 << (bit % 32);
    }
    out
}

fn skewed_high_degree_graph(node_count: u32) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    assert!(
        node_count > 40,
        "Fix: skewed CUDA CSR queue parity graph needs enough target nodes."
    );
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::with_capacity(CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD as usize + 8);
    let mut edge_kind_mask = Vec::with_capacity(edge_targets.capacity());
    edge_offsets.push(0);
    for src in 0..node_count {
        if src == 0 {
            for edge in 0..CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD {
                edge_targets.push(edge.wrapping_mul(7).wrapping_add(11) % node_count);
                edge_kind_mask.push(if edge % 5 == 0 { 2 } else { 1 });
            }
        } else if (1..=8).contains(&src) {
            edge_targets.push((src + 23) % node_count);
            edge_kind_mask.push(if src % 3 == 0 { 2 } else { 1 });
        }
        edge_offsets.push(edge_targets.len() as u32);
    }
    (edge_offsets, edge_targets, edge_kind_mask)
}

