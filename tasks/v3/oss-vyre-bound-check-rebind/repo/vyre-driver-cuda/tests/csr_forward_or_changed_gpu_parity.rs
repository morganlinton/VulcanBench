//! Parity test: GPU iterated forward closure matches CPU iterated closure.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_cuda_optimizer_dispatcher, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::graph::csr_forward_or_changed::{
    csr_forward_or_changed_parallel, csr_forward_or_changed_parallel_batch,
    csr_forward_or_changed_parallel_batch_grid, csr_forward_or_changed_parallel_grid,
};
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_self_substrate::csr_forward_or_changed::{
    forward_closure_via_change_flag_gpu, reference_forward_closure_via_change_flag,
};

fn set_bit(words: &mut [u32], node: u32) {
    let word = (node / 32) as usize;
    let bit = node & 31;
    words[word] |= 1 << bit;
}

fn assert_forward_closure_matches(
    label: &str,
    n: u32,
    off: &[u32],
    tgt: &[u32],
    msk: &[u32],
    seed: &[u32],
    allow: u32,
    max_iters: u32,
) -> Vec<u32> {
    let cpu = reference_forward_closure_via_change_flag(n, off, tgt, msk, seed, allow, max_iters);
    with_cuda_optimizer_dispatcher(label, |dispatcher| {
        let gpu = forward_closure_via_change_flag_gpu(
            dispatcher, n, off, tgt, msk, seed, allow, max_iters,
        )
        .expect("dispatch");
        assert_eq!(gpu, cpu, "{label}: closure divergence");
        gpu
    })
}

#[test]
fn cuda_parallel_primitive_reaches_sources_past_first_block() {
    let node_count = 513;
    let words = ((node_count + 31) / 32) as usize;
    let mut offsets = vec![0u32; node_count as usize + 1];
    for offset in offsets.iter_mut().skip(301) {
        *offset = 1;
    }
    let targets = vec![512u32];
    let masks = vec![1u32];
    let mut frontier = vec![0u32; words];
    set_bit(&mut frontier, 300);

    let pg_nodes = vec![0u32; node_count as usize];
    let pg_node_tags = vec![0u32; node_count as usize];
    let program = csr_forward_or_changed_parallel(
        ProgramGraphShape::new(node_count, targets.len() as u32),
        "frontier",
        "changed",
        0xFFFF_FFFF,
    );
    let inputs = vec![
        u32_bytes(&pg_nodes),
        u32_bytes(&offsets),
        u32_bytes(&targets),
        u32_bytes(&masks),
        u32_bytes(&pg_node_tags),
        u32_bytes(&frontier),
        vec![0u8; 4],
    ];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(csr_forward_or_changed_parallel_grid(node_count));

    let outputs = with_live_backend("parallel CSR forward primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .expect("Fix: CUDA CSR forward primitive dispatch should succeed")
    });
    let mut expected = vec![0u32; words];
    set_bit(&mut expected, 300);
    set_bit(&mut expected, 512);
    let mut observed = bytes_u32(&outputs[0]);
    observed.truncate(words);
    assert_eq!(observed, expected);
    assert_eq!(bytes_u32(&outputs[1])[0], 1);
}

#[test]
fn cuda_parallel_batch_primitive_reaches_sources_past_first_block() {
    let node_count = 513;
    let query_count = 3;
    let words = ((node_count + 31) / 32) as usize;
    let mut offsets = vec![0u32; node_count as usize + 1];
    for offset in offsets.iter_mut().take(401).skip(301) {
        *offset = 1;
    }
    for offset in offsets.iter_mut().skip(401) {
        *offset = 2;
    }
    let targets = vec![512u32, 17];
    let masks = vec![1u32, 1];
    let mut frontiers = vec![0u32; words * query_count as usize];
    set_bit(&mut frontiers[0..words], 300);
    set_bit(&mut frontiers[words..(2 * words)], 400);
    set_bit(&mut frontiers[(2 * words)..(3 * words)], 1);

    let pg_nodes = vec![0u32; node_count as usize];
    let pg_node_tags = vec![0u32; node_count as usize];
    let program = csr_forward_or_changed_parallel_batch(
        ProgramGraphShape::new(node_count, targets.len() as u32),
        "frontiers",
        "changed",
        0xFFFF_FFFF,
        query_count,
    );
    let inputs = vec![
        u32_bytes(&pg_nodes),
        u32_bytes(&offsets),
        u32_bytes(&targets),
        u32_bytes(&masks),
        u32_bytes(&pg_node_tags),
        u32_bytes(&frontiers),
        vec![0u8; query_count as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(csr_forward_or_changed_parallel_batch_grid(
        node_count,
        query_count,
    ));

    let outputs = with_live_backend("parallel batched CSR forward primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .expect("Fix: CUDA batched CSR forward primitive dispatch should succeed")
    });
    let mut expected = vec![0u32; words * query_count as usize];
    set_bit(&mut expected[0..words], 300);
    set_bit(&mut expected[0..words], 512);
    set_bit(&mut expected[words..(2 * words)], 400);
    set_bit(&mut expected[words..(2 * words)], 17);
    set_bit(&mut expected[(2 * words)..(3 * words)], 1);
    let mut observed = bytes_u32(&outputs[0]);
    observed.truncate(words * query_count as usize);
    assert_eq!(observed, expected);
    let mut changed = bytes_u32(&outputs[1]);
    changed.truncate(query_count as usize);
    assert_eq!(changed, vec![1, 1, 0]);
}

#[test]
fn cuda_forward_closure_chain_matches_cpu() {
    // Chain 0 -> 1 -> 2 -> 3.
    let off = vec![0u32, 1, 2, 3, 3];
    let tgt = vec![1u32, 2, 3];
    let msk = vec![1u32, 1, 1];
    let seed = vec![0b0001u32];
    let gpu = assert_forward_closure_matches(
        "chain closure",
        4,
        &off,
        &tgt,
        &msk,
        &seed,
        0xFFFF_FFFF,
        16,
    );
    assert_eq!(gpu, vec![0b1111u32]);
}

#[test]
fn cuda_forward_closure_reaches_source_past_first_block() {
    let node_count = 513;
    let words = ((node_count + 31) / 32) as usize;
    let mut off = vec![0u32; node_count as usize + 1];
    for offset in off.iter_mut().skip(301) {
        *offset = 1;
    }
    let tgt = vec![512u32];
    let msk = vec![1u32];
    let mut seed = vec![0u32; words];
    set_bit(&mut seed, 300);
    let gpu = assert_forward_closure_matches(
        "block-packed closure",
        node_count,
        &off,
        &tgt,
        &msk,
        &seed,
        0xFFFF_FFFF,
        4,
    );
    let mut expected = vec![0u32; words];
    set_bit(&mut expected, 300);
    set_bit(&mut expected, 512);
    assert_eq!(gpu, expected);
}

#[test]
fn cuda_forward_closure_disconnected() {
    // 0 -> 1, 2 -> 3, no cross.
    let off = vec![0u32, 1, 1, 2, 2];
    let tgt = vec![1u32, 3];
    let msk = vec![1u32, 1];
    let seed = vec![0b0001u32]; // only {0}
    let gpu = assert_forward_closure_matches(
        "disconnected closure",
        4,
        &off,
        &tgt,
        &msk,
        &seed,
        0xFFFF_FFFF,
        16,
    );
    assert_eq!(gpu, vec![0b0011u32]);
}

#[test]
fn cuda_forward_closure_self_loop_terminates() {
    let off = vec![0u32, 1, 1];
    let tgt = vec![0u32];
    let msk = vec![1u32];
    let seed = vec![0b01u32];
    assert_forward_closure_matches(
        "self-loop closure",
        2,
        &off,
        &tgt,
        &msk,
        &seed,
        0xFFFF_FFFF,
        50,
    );
}

#[test]
fn cuda_forward_closure_allow_mask_filters() {
    let off = vec![0u32, 1, 1];
    let tgt = vec![1u32];
    let msk = vec![0b0010u32];
    let seed = vec![0b01u32];
    let allow = 0b0001;
    let gpu =
        assert_forward_closure_matches("allow-mask closure", 2, &off, &tgt, &msk, &seed, allow, 16);
    assert_eq!(gpu, vec![0b01u32]);
}

#[test]
fn cuda_forward_closure_diamond() {
    // Diamond 0 -> {1, 2} -> 3.
    let off = vec![0u32, 2, 3, 4, 4];
    let tgt = vec![1u32, 2, 3, 3];
    let msk = vec![1u32, 1, 1, 1];
    let seed = vec![0b0001u32];
    let gpu = assert_forward_closure_matches(
        "diamond closure",
        4,
        &off,
        &tgt,
        &msk,
        &seed,
        0xFFFF_FFFF,
        16,
    );
    assert_eq!(gpu, vec![0b1111u32]);
}
