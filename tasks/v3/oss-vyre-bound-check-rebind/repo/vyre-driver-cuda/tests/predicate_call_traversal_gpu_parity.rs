//! Parity test: vyre-primitives predicate edge-traversal wrappers
//! (arg_of, call_to, return_value_of) match their CPU oracles.
//!
//! All three delegate to csr_forward_traverse / csr_backward_traverse
//! with a fixed edge-kind mask.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_primitives::graph::csr_backward_traverse::csr_backward_traverse_dispatch_grid;
use vyre_primitives::graph::csr_forward_traverse::csr_forward_traverse_dispatch_grid;
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::predicate::arg_of::{arg_of, cpu_ref as arg_of_cpu};
use vyre_primitives::predicate::call_to::{call_to, cpu_ref as call_to_cpu};
use vyre_primitives::predicate::edge_kind;
use vyre_primitives::predicate::return_value_of::{
    cpu_ref as return_value_of_cpu, return_value_of,
};

/// Run a forward-traversal wrapper (call_to, return_value_of).
fn run_forward<B>(
    backend: &CudaBackend,
    program_builder: B,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier: &[u32],
) -> Vec<u32>
where
    B: FnOnce(ProgramGraphShape, &str, &str) -> vyre::Program,
{
    let words = node_count.div_ceil(32).max(1);
    let pg_nodes = vec![0u32; node_count as usize];
    let pg_node_tags = vec![0u32; node_count as usize];
    let edge_count = edge_targets.len() as u32;
    let program = program_builder(
        ProgramGraphShape::new(node_count, edge_count.max(1)),
        "frontier_in",
        "frontier_out",
    );
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(&pg_nodes),
        u32_bytes(edge_offsets),
        u32_bytes(edge_targets),
        u32_bytes(edge_kind_mask),
        u32_bytes(&pg_node_tags),
        u32_bytes(frontier),
        vec![0u8; words as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(csr_forward_traverse_dispatch_grid(node_count));
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(words as usize);
    out
}

/// Run a backward-traversal wrapper (arg_of).
fn run_backward<B>(
    backend: &CudaBackend,
    program_builder: B,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier: &[u32],
) -> Vec<u32>
where
    B: FnOnce(ProgramGraphShape, &str, &str) -> vyre::Program,
{
    let words = node_count.div_ceil(32).max(1);
    let pg_nodes = vec![0u32; node_count as usize];
    let pg_node_tags = vec![0u32; node_count as usize];
    let edge_count = edge_targets.len() as u32;
    let program = program_builder(
        ProgramGraphShape::new(node_count, edge_count.max(1)),
        "frontier_in",
        "frontier_out",
    );
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(&pg_nodes),
        u32_bytes(edge_offsets),
        u32_bytes(edge_targets),
        u32_bytes(edge_kind_mask),
        u32_bytes(&pg_node_tags),
        u32_bytes(frontier),
        vec![0u8; words as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(csr_backward_traverse_dispatch_grid(node_count));
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(words as usize);
    out
}

#[test]
fn cuda_call_to_one_step() {
    with_live_backend("cuda_call_to_one_step", |backend| {
        // Caller 0 -> callee 1 via CALL_ARG. Edge kind mask = CALL_ARG.
        let edge_offsets = vec![0u32, 1, 1];
        let edge_targets = vec![1u32];
        let edge_kind_mask = vec![edge_kind::CALL_ARG];
        let frontier = vec![0b01u32]; // {0}
        let cpu = call_to_cpu(2, &edge_offsets, &edge_targets, &edge_kind_mask, &frontier);
        let gpu = run_forward(
            backend,
            call_to,
            2,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &frontier,
        );
        assert_eq!(gpu, cpu);
        assert_eq!(gpu, vec![0b10u32]);
    });
}

#[test]
fn cuda_call_to_skips_non_call_edges() {
    with_live_backend("cuda_call_to_skips_non_call_edges", |backend| {
        // Edge has kind ASSIGNMENT, not CALL_ARG. call_to must skip it.
        let edge_offsets = vec![0u32, 1, 1];
        let edge_targets = vec![1u32];
        let edge_kind_mask = vec![edge_kind::ASSIGNMENT];
        let frontier = vec![0b01u32];
        let cpu = call_to_cpu(2, &edge_offsets, &edge_targets, &edge_kind_mask, &frontier);
        let gpu = run_forward(
            backend,
            call_to,
            2,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &frontier,
        );
        assert_eq!(gpu, cpu);
        assert_eq!(gpu, vec![0u32]);
    });
}

#[test]
fn cuda_return_value_of_one_step() {
    with_live_backend("cuda_return_value_of_one_step", |backend| {
        // Callsite 0 → return-binding 1 via RETURN edge.
        let edge_offsets = vec![0u32, 1, 1];
        let edge_targets = vec![1u32];
        let edge_kind_mask = vec![edge_kind::RETURN];
        let frontier = vec![0b01u32];
        let cpu = return_value_of_cpu(2, &edge_offsets, &edge_targets, &edge_kind_mask, &frontier);
        let gpu = run_forward(
            backend,
            return_value_of,
            2,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &frontier,
        );
        assert_eq!(gpu, cpu);
        assert_eq!(gpu, vec![0b10u32]);
    });
}

#[test]
fn cuda_return_value_of_ignores_call_arg_edges() {
    with_live_backend("cuda_return_value_of_ignores_call_arg_edges", |backend| {
        let edge_offsets = vec![0u32, 1, 1];
        let edge_targets = vec![1u32];
        let edge_kind_mask = vec![edge_kind::CALL_ARG];
        let frontier = vec![0b01u32];
        let cpu = return_value_of_cpu(2, &edge_offsets, &edge_targets, &edge_kind_mask, &frontier);
        let gpu = run_forward(
            backend,
            return_value_of,
            2,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &frontier,
        );
        assert_eq!(gpu, cpu);
        assert_eq!(gpu, vec![0u32]);
    });
}

#[test]
fn cuda_arg_of_unspecified_one_step_backward() {
    with_live_backend("cuda_arg_of_unspecified_one_step_backward", |backend| {
        // Caller 0 -> arg-expr 1 via CALL_ARG. arg_of from {1} → {0}.
        let edge_offsets = vec![0u32, 1, 1];
        let edge_targets = vec![1u32];
        let edge_kind_mask = vec![edge_kind::CALL_ARG];
        let frontier = vec![0b10u32]; // {1}
        let cpu = arg_of_cpu(2, &edge_offsets, &edge_targets, &edge_kind_mask, &frontier);
        let gpu = run_backward(
            backend,
            arg_of,
            2,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &frontier,
        );
        assert_eq!(gpu, cpu);
        assert_eq!(gpu, vec![0b01u32]);
    });
}

#[test]
fn cuda_arg_of_kind_filtered_out() {
    with_live_backend("cuda_arg_of_kind_filtered_out", |backend| {
        // Edge is RETURN, not CALL_ARG. arg_of must not pick it up.
        let edge_offsets = vec![0u32, 1, 1];
        let edge_targets = vec![1u32];
        let edge_kind_mask = vec![edge_kind::RETURN];
        let frontier = vec![0b10u32];
        let cpu = arg_of_cpu(2, &edge_offsets, &edge_targets, &edge_kind_mask, &frontier);
        let gpu = run_backward(
            backend,
            arg_of,
            2,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &frontier,
        );
        assert_eq!(gpu, cpu);
        assert_eq!(gpu, vec![0u32]);
    });
}

#[test]
fn cuda_arg_of_reaches_source_past_first_workgroup() {
    with_live_backend(
        "cuda_arg_of_reaches_source_past_first_workgroup",
        |backend| {
            let node_count = 513u32;
            let words = node_count.div_ceil(32) as usize;
            let mut edge_offsets = vec![0u32; node_count as usize + 1];
            for offset in edge_offsets.iter_mut().skip(301) {
                *offset = 1;
            }
            let edge_targets = vec![512u32];
            let edge_kind_mask = vec![edge_kind::CALL_ARG];
            let mut frontier = vec![0u32; words];
            frontier[512 / 32] |= 1u32 << (512 % 32);

            let cpu = arg_of_cpu(
                node_count,
                &edge_offsets,
                &edge_targets,
                &edge_kind_mask,
                &frontier,
            );
            let gpu = run_backward(
                backend,
                arg_of,
                node_count,
                &edge_offsets,
                &edge_targets,
                &edge_kind_mask,
                &frontier,
            );

            let mut expected = vec![0u32; words];
            expected[300 / 32] |= 1u32 << (300 % 32);
            assert_eq!(gpu, cpu);
            assert_eq!(gpu, expected);
        },
    );
}
