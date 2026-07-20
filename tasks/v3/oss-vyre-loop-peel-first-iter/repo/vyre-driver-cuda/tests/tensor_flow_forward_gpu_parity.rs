//! Parity test: GPU tensor_flow_forward reaches source lanes across workgroups.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::graph::tensor_flow_forward::{
    tensor_flow_forward, tensor_flow_forward_dispatch_grid, tensor_words,
    try_tensor_flow_forward_cpu,
};

fn tensor_bit_index(node: u32, ctx: u32, fld: u32, context_limit: u32, field_limit: u32) -> u32 {
    node * context_limit * field_limit + ctx * field_limit + fld
}

fn set_tensor_bit(words: &mut [u32], bit: u32) {
    words[(bit / 32) as usize] |= 1u32 << (bit % 32);
}

#[allow(clippy::too_many_arguments)]
fn run_tensor_flow(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    tensor_in: &[u32],
    context_limit: u32,
    field_limit: u32,
    allow_mask: u32,
) -> Vec<u32> {
    let word_count = tensor_words(node_count, context_limit, field_limit) as usize;
    let pg_nodes = vec![0u32; node_count as usize];
    let pg_node_tags = vec![0u32; node_count as usize];
    let program = tensor_flow_forward(
        ProgramGraphShape::new(node_count, edge_targets.len().max(1) as u32),
        "tensor_in",
        "tensor_out",
        context_limit,
        field_limit,
        allow_mask,
    );
    let inputs = vec![
        u32_bytes(&pg_nodes),
        u32_bytes(edge_offsets),
        u32_bytes(edge_targets),
        u32_bytes(edge_kind_mask),
        u32_bytes(&pg_node_tags),
        u32_bytes(tensor_in),
        vec![0u8; word_count * 4],
    ];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(tensor_flow_forward_dispatch_grid(node_count));
    let outputs = with_live_backend("tensor_flow_forward", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA tensor-flow dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(word_count);
    out
}

#[test]
fn cuda_tensor_flow_reaches_source_past_first_workgroup() {
    let node_count = 513;
    let context_limit = 2;
    let field_limit = 3;
    let mut offsets = vec![0u32; node_count as usize + 1];
    for offset in offsets.iter_mut().skip(301) {
        *offset = 1;
    }
    let targets = vec![512u32];
    let masks = vec![0b10u32];
    let mut tensor_in = vec![0u32; tensor_words(node_count, context_limit, field_limit) as usize];
    set_tensor_bit(
        &mut tensor_in,
        tensor_bit_index(300, 1, 2, context_limit, field_limit),
    );

    let expected = try_tensor_flow_forward_cpu(
        node_count,
        &offsets,
        &targets,
        &masks,
        &tensor_in,
        context_limit,
        field_limit,
        0b10,
    )
    .expect("Fix: CPU tensor-flow oracle should accept the large CSR fixture");
    let gpu = run_tensor_flow(
        node_count,
        &offsets,
        &targets,
        &masks,
        &tensor_in,
        context_limit,
        field_limit,
        0b10,
    );

    let mut expected_probe = vec![0u32; expected.len()];
    set_tensor_bit(
        &mut expected_probe,
        tensor_bit_index(512, 1, 2, context_limit, field_limit),
    );
    assert_eq!(tensor_flow_forward_dispatch_grid(node_count), [3, 1, 1]);
    assert_eq!(expected, expected_probe);
    assert_eq!(gpu, expected);
}

#[test]
fn cuda_tensor_flow_respects_edge_mask_without_false_output() {
    let node_count = 513;
    let context_limit = 2;
    let field_limit = 3;
    let mut offsets = vec![0u32; node_count as usize + 1];
    for offset in offsets.iter_mut().skip(301) {
        *offset = 1;
    }
    let targets = vec![512u32];
    let masks = vec![0b10u32];
    let mut tensor_in = vec![0u32; tensor_words(node_count, context_limit, field_limit) as usize];
    set_tensor_bit(
        &mut tensor_in,
        tensor_bit_index(300, 1, 2, context_limit, field_limit),
    );

    let gpu = run_tensor_flow(
        node_count,
        &offsets,
        &targets,
        &masks,
        &tensor_in,
        context_limit,
        field_limit,
        0b01,
    );

    assert_eq!(
        gpu,
        vec![0u32; tensor_words(node_count, context_limit, field_limit) as usize]
    );
}
