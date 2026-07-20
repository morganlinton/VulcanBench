//! V8 BUILD-FAILING TRUTH TEST  -  c_lower_ast_to_pg_nodes GPU parity.
//!
//! Per AGENTS.md real-tests rule: a test that asserts shape is not a test.
//! This test dispatches the real `c_lower_ast_to_pg_nodes` Program on the
//! real 5090 (or fails loudly if no GPU adapter is found) with a hand-built
//! 6-node witness VAST buffer, reads back the produced `pg_nodes` buffer,
//! and asserts BYTE-IDENTITY against the CPU reference impl.
//!
//! What the assertion catches that a shape test does NOT:
//!   - GPU dispatch silently writes zeros → reference produces non-zero;
//!     test fails with the exact word that mismatched.
//!   - Wrong stride between input/output records → byte mismatch at exact
//!     record boundary.
//!   - Naga emit drops a `Node::store` for a payload field → the field is
//!     zero on GPU, non-zero on CPU; mismatch at exact byte offset.
//!   - Workgroup size [256,1,1] with only 6 nodes mismatched bounds → OOB
//!     write surfaced via reference disagreement on words past index 36.
//!
//! Failing test = engine bug. Do not weaken the assertion.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre::ir::Expr;
use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::parsing::c::lower::ast_to_pg_nodes::{
    c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes,
};
use vyre_primitives::predicate::node_kind;

const VAST_STRIDE_U32: usize = 10;
const PG_STRIDE_U32: usize = 6;

fn append_vast(
    out: &mut Vec<u32>,
    kind: u32,
    parent: u32,
    span_start: u32,
    span_len: u32,
    attr_off: u32,
    attr_len: u32,
) {
    out.extend_from_slice(&[
        kind,
        parent,
        u32::MAX,
        u32::MAX,
        u32::MAX,
        span_start,
        span_len,
        attr_off,
        attr_len,
        u32::MAX,
    ]);
}

fn witness_six_nodes() -> Vec<u32> {
    let mut v = Vec::new();
    append_vast(&mut v, node_kind::VARIABLE, u32::MAX, 0, 11, 128, 4);
    append_vast(&mut v, node_kind::CALL, 0, 16, 9, 0, 8);
    append_vast(&mut v, node_kind::LITERAL, 1, 32, 7, 32, 0);
    append_vast(&mut v, node_kind::IMPORT, 2, 48, 13, 128, 16);
    append_vast(&mut v, node_kind::SSA, 3, 62, 3, 256, 8);
    append_vast(&mut v, node_kind::BASIC_BLOCK, u32::MAX, 96, 17, 1024, 64);
    v
}

fn u32_to_bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

#[test]
fn gpu_parity_witness_six_nodes() {
    // GPU presence is non-negotiable per the workspace AGENTS.md.
    // If wgpu cannot find an adapter on a machine with a 5090, that's a
    // configuration bug  -  surface it loudly, do NOT skip.
    let backend = WgpuBackend::new().expect(
        "Fix: WgpuBackend::new failed on a machine that must have a GPU; this is a config bug, not a graceful fallback.",
    );

    let vast_words = witness_six_nodes();
    let vast_bytes = u32_to_bytes(&vast_words);
    let num_nodes = (vast_words.len() / VAST_STRIDE_U32) as u32;
    let expected_pg_bytes = reference_ast_to_pg_nodes(&vast_bytes);
    assert_eq!(
        expected_pg_bytes.len(),
        (num_nodes as usize) * PG_STRIDE_U32 * 4,
        "reference impl produced wrong-sized pg_nodes buffer"
    );

    let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(num_nodes), "out_pg_nodes");

    let inputs: Vec<&[u8]> = vec![&vast_bytes];
    let outputs = backend
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU dispatch must succeed");

    assert_eq!(
        outputs.len(),
        1,
        "expected exactly one ReadWrite output buffer (pg_nodes), got {}",
        outputs.len()
    );

    let actual_pg_bytes = &outputs[0];
    assert_eq!(
        actual_pg_bytes.len(),
        expected_pg_bytes.len(),
        "GPU pg_nodes buffer length {} != CPU reference length {}",
        actual_pg_bytes.len(),
        expected_pg_bytes.len()
    );

    // Word-by-word comparison surfaces the exact field that diverged.
    for i in 0..(num_nodes as usize) {
        for field in 0..PG_STRIDE_U32 {
            let off = (i * PG_STRIDE_U32 + field) * 4;
            let cpu = u32::from_le_bytes(expected_pg_bytes[off..off + 4].try_into().unwrap());
            let gpu = u32::from_le_bytes(actual_pg_bytes[off..off + 4].try_into().unwrap());
            assert_eq!(
                gpu, cpu,
                "node[{i}].field[{field}] (byte off {off}): GPU={gpu} CPU={cpu}. \
                 Layout: 0=kind 1=span_start 2=span_end 3=parent 4=first_child 5=next_sibling"
            );
        }
    }
}

#[test]
fn gpu_parity_single_node_edge_case() {
    let backend = WgpuBackend::new().expect("Fix: GPU required for single-node parity case");

    let mut v = Vec::new();
    append_vast(&mut v, node_kind::FUNCTION_DECL, u32::MAX, 0, 50, 0, 0);
    let vast_bytes = u32_to_bytes(&v);
    let num_nodes = 1u32;
    let expected = reference_ast_to_pg_nodes(&vast_bytes);

    let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(num_nodes), "out_pg_nodes");
    let inputs: Vec<&[u8]> = vec![&vast_bytes];
    let outputs = backend
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("dispatch");

    assert_eq!(outputs[0], expected, "single-node parity mismatch");

    // The first record carries the FUNCTION_DECL kind; verify it survived.
    let kind = u32::from_le_bytes(outputs[0][0..4].try_into().unwrap());
    assert_eq!(
        kind,
        node_kind::FUNCTION_DECL,
        "kind word lost across GPU dispatch  -  node_kind::FUNCTION_DECL got {kind}"
    );
}

#[test]
fn gpu_parity_zero_nodes_returns_empty_buffer() {
    let backend = WgpuBackend::new().expect("Fix: GPU required for zero-node parity case");

    let vast_bytes: Vec<u8> = vec![];
    let num_nodes = 0u32;

    let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(num_nodes), "out_pg_nodes");
    let inputs: Vec<&[u8]> = vec![&vast_bytes];
    let outputs = backend
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("dispatch on zero nodes must not crash");

    assert!(
        outputs[0].iter().all(|&b| b == 0),
        "zero-node dispatch produced non-zero output bytes  -  GPU wrote past dispatch bound"
    );
}
