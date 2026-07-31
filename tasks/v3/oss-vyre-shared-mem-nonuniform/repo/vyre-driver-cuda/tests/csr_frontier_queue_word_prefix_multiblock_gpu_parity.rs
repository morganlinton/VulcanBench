//! Multi-block word-prefix CUDA parity for resident CSR frontier queues.

#![cfg(test)]

mod common;

use common::{bytes_u32, live_backend};
use vyre_driver_cuda::{CudaBackend, CudaOptimizerDispatcher};
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::csr_frontier_queue::{
    csr_queue_forward_traverse_cpu, frontier_to_queue_cpu,
};
use vyre_self_substrate::csr_frontier_queue_resident::{
    run_resident_csr_queue_query_into, upload_resident_csr_queue_graph, ResidentCsrQueueScratch,
};

const NODE_COUNT: u32 = 32_897;
const MANY_BLOCK_NODE_COUNT: u32 = 262_177;
const QUEUE_CAPACITY: u32 = 4_096;
const GENERATED_CASES: u32 = 512;
const ALLOW_MASKS: [u32; 8] = [0, 1, 2, 4, 3, 5, 6, 7];

#[test]
fn generated_resident_csr_queue_word_prefix_multiblock_matches_cpu_oracle_on_live_cuda() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    assert!(
        bitset_words(NODE_COUNT) > 1024,
        "Fix: this test must exercise more than one word-prefix scan block."
    );
    let (edge_offsets, edge_targets, edge_kind_mask) = generated_csr_graph(NODE_COUNT);
    let graph = upload_resident_csr_queue_graph(
        &dispatcher,
        NODE_COUNT,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .expect("Fix: generated multi-block resident CSR queue graph upload failed.");
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output = Vec::new();

    backend.reset_telemetry();
    for case_index in 0..GENERATED_CASES {
        let frontier = generated_frontier(NODE_COUNT, case_index);
        let allow_mask = ALLOW_MASKS[(case_index as usize) % ALLOW_MASKS.len()];
        let (expected_queue, expected_len) =
            frontier_to_queue_cpu(&frontier, NODE_COUNT, QUEUE_CAPACITY as usize);
        let expected_out = csr_queue_forward_traverse_cpu(
            &expected_queue,
            expected_len,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            NODE_COUNT,
            allow_mask,
        );

        run_resident_csr_queue_query_into(
            &dispatcher,
            &graph,
            &mut scratch,
            &frontier,
            QUEUE_CAPACITY,
            allow_mask,
            &mut output,
        )
        .unwrap_or_else(|error| {
            panic!(
                "Fix: generated multi-block resident CSR queue case {case_index} failed: {error}"
            )
        });
        assert_eq!(
            bytes_u32(&output),
            expected_out,
            "Fix: generated multi-block resident CSR queue case {case_index} diverged from CPU oracle for allow_mask={allow_mask:#x}."
        );
    }

    assert_multi_case_telemetry(&backend);
    scratch
        .free(&dispatcher)
        .expect("Fix: generated multi-block resident CSR queue scratch free failed.");
    graph
        .free(&dispatcher)
        .expect("Fix: generated multi-block resident CSR queue graph free failed.");
}

fn assert_multi_case_telemetry(backend: &CudaBackend) {
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches,
        u64::from(GENERATED_CASES) * 4,
        "Fix: small multi-block word-prefix queue must stay at clear + word-scan + inline-offset queue-scatter + traverse per query."
    );
    assert_eq!(
        telemetry.sync_points,
        u64::from(GENERATED_CASES),
        "Fix: generated multi-block word-prefix queue must fence once per resident query."
    );
    let expected_frontier_bytes =
        u64::from(GENERATED_CASES) * u64::from(bitset_words(NODE_COUNT)) * 4;
    assert_eq!(
        telemetry
            .host_to_device_bytes
            .saturating_sub(telemetry.param_upload_bytes),
        expected_frontier_bytes,
        "Fix: generated multi-block word-prefix queue must upload only frontier payloads after graph residency."
    );
    assert_eq!(
        telemetry.readback_bytes, expected_frontier_bytes,
        "Fix: generated multi-block word-prefix queue must read back only frontier_out."
    );
}

#[test]
fn generated_resident_csr_queue_many_block_word_prefix_uses_block_offset_scan_on_live_cuda() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    assert!(
        bitset_words(MANY_BLOCK_NODE_COUNT) > 8 * 1024,
        "Fix: this test must exercise the many-block word-prefix offset-scan path."
    );
    let (edge_offsets, edge_targets, edge_kind_mask) = generated_csr_graph(MANY_BLOCK_NODE_COUNT);
    let graph = upload_resident_csr_queue_graph(
        &dispatcher,
        MANY_BLOCK_NODE_COUNT,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .expect("Fix: generated many-block resident CSR queue graph upload failed.");
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output = Vec::new();

    backend.reset_telemetry();
    for case_index in 0..16u32 {
        let frontier = generated_frontier(MANY_BLOCK_NODE_COUNT, case_index);
        let allow_mask = ALLOW_MASKS[(case_index as usize) % ALLOW_MASKS.len()];
        let (expected_queue, expected_len) =
            frontier_to_queue_cpu(&frontier, MANY_BLOCK_NODE_COUNT, QUEUE_CAPACITY as usize);
        let expected_out = csr_queue_forward_traverse_cpu(
            &expected_queue,
            expected_len,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            MANY_BLOCK_NODE_COUNT,
            allow_mask,
        );

        run_resident_csr_queue_query_into(
            &dispatcher,
            &graph,
            &mut scratch,
            &frontier,
            QUEUE_CAPACITY,
            allow_mask,
            &mut output,
        )
        .unwrap_or_else(|error| {
            panic!("Fix: generated many-block resident CSR queue case {case_index} failed: {error}")
        });
        assert_eq!(
            bytes_u32(&output),
            expected_out,
            "Fix: generated many-block resident CSR queue case {case_index} diverged from CPU oracle for allow_mask={allow_mask:#x}."
        );
    }

    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches,
        16 * 5,
        "Fix: many-block word-prefix queue must run clear + word-scan + block-offset scan + queue-scatter + traverse per query."
    );
    assert_eq!(
        telemetry.sync_points, 16,
        "Fix: generated many-block word-prefix queue must fence once per resident query."
    );
    scratch
        .free(&dispatcher)
        .expect("Fix: generated many-block resident CSR queue scratch free failed.");
    graph
        .free(&dispatcher)
        .expect("Fix: generated many-block resident CSR queue graph free failed.");
}

fn generated_csr_graph(node_count: u32) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::new();
    let mut edge_kind_mask = Vec::new();
    edge_offsets.push(0);
    for src in 0..node_count {
        if src % 3 != 0 {
            edge_targets.push(src.wrapping_mul(17).wrapping_add(11) % node_count);
            edge_kind_mask.push(1);
        }
        if src % 7 == 0 {
            edge_targets.push(src.wrapping_mul(29).wrapping_add(31) % node_count);
            edge_kind_mask.push(2);
        }
        if (src ^ node_count) % 17 == 0 {
            edge_targets.push(src.wrapping_mul(43).wrapping_add(5) % node_count);
            edge_kind_mask.push(4);
        }
        edge_offsets.push(edge_targets.len() as u32);
    }
    (edge_offsets, edge_targets, edge_kind_mask)
}

fn generated_frontier(node_count: u32, case_index: u32) -> Vec<u32> {
    let mut frontier = vec![0_u32; bitset_words(node_count) as usize];
    let salt = mix32(case_index ^ node_count.rotate_left(11));
    let period = 23 + (case_index % 37);
    for node in 0..node_count {
        let h = mix32(node.wrapping_mul(0x85eb_ca6b) ^ salt);
        if h % period == 0 {
            set_node(&mut frontier, node);
        }
    }
    set_node(&mut frontier, salt % node_count);
    if case_index % 19 == 0 {
        set_node(&mut frontier, 0);
    }
    if case_index % 23 == 0 {
        set_node(&mut frontier, 32_768);
    }
    if case_index % 29 == 0 {
        set_node(&mut frontier, node_count - 1);
    }
    frontier
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
