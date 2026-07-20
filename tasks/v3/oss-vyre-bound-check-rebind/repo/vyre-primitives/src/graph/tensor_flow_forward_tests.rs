use super::program_graph::ProgramGraphShape;
use super::tensor_flow_forward::{
    tensor_flow_forward_dispatch_grid, tensor_words, try_tensor_flow_forward,
    try_tensor_flow_forward_cpu, try_tensor_flow_forward_cpu_into,
    TENSOR_FLOW_FORWARD_WORKGROUP_SIZE,
};

fn tensor_bit_index(node: u32, ctx: u32, fld: u32, context_limit: u32, field_limit: u32) -> u32 {
    node * context_limit * field_limit + ctx * field_limit + fld
}

fn tensor_bit_is_set(words: &[u32], bit: u32) -> bool {
    words
        .get((bit / 32) as usize)
        .copied()
        .is_some_and(|word| (word & (1u32 << (bit % 32))) != 0)
}

fn set_tensor_bit(words: &mut [u32], bit: u32) {
    if let Some(word) = words.get_mut((bit / 32) as usize) {
        *word |= 1u32 << (bit % 32);
    }
}

#[allow(clippy::too_many_arguments)]
fn explicit_edge_first_reference(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    tensor_in_words: &[u32],
    context_limit: u32,
    field_limit: u32,
    allow_mask: u32,
) -> Vec<u32> {
    let mut out = vec![0u32; tensor_words(node_count, context_limit, field_limit) as usize];
    for src in 0..node_count {
        let start = edge_offsets[src as usize] as usize;
        let end = edge_offsets[src as usize + 1] as usize;
        for edge in start..end {
            if (edge_kind_mask[edge] & allow_mask) == 0 {
                continue;
            }
            let dst = edge_targets[edge];
            if dst >= node_count {
                continue;
            }
            for ctx in 0..context_limit {
                for fld in 0..field_limit {
                    let src_bit = tensor_bit_index(src, ctx, fld, context_limit, field_limit);
                    if tensor_bit_is_set(tensor_in_words, src_bit) {
                        let dst_bit = tensor_bit_index(dst, ctx, fld, context_limit, field_limit);
                        set_tensor_bit(&mut out, dst_bit);
                    }
                }
            }
        }
    }
    out
}

#[test]
fn generated_tensor_flow_cpu_matches_edge_first_reference() {
    let mut state = 0x7E11_50F0_u32;
    for case in 0..2048u32 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let node_count = state % 41 + 1;
        let context_limit = state.rotate_left(5) % 5 + 1;
        let field_limit = state.rotate_left(9) % 7 + 1;
        let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
        let mut edge_targets = Vec::new();
        let mut edge_kind_mask = Vec::new();
        edge_offsets.push(0);
        for src in 0..node_count {
            state = state.rotate_left(3) ^ src.wrapping_mul(0x9E37_79B9);
            let degree = state % 6;
            for edge in 0..degree {
                state = state.rotate_left(7) ^ edge.wrapping_mul(0x85EB_CA6B);
                let target = match edge % 5 {
                    0 => state % node_count,
                    1 => node_count,
                    2 => u32::MAX,
                    _ => state % (node_count + 3),
                };
                edge_targets.push(target);
                edge_kind_mask.push(1u32 << (state & 7));
            }
            edge_offsets.push(edge_targets.len() as u32);
        }

        let mut tensor_in =
            vec![0u32; tensor_words(node_count, context_limit, field_limit) as usize];
        for src in 0..node_count {
            for ctx in 0..context_limit {
                for fld in 0..field_limit {
                    state = state.rotate_left(11)
                        ^ src.wrapping_mul(17)
                        ^ ctx.wrapping_mul(31)
                        ^ fld.wrapping_mul(43);
                    if (state & 3) != 0 {
                        let bit = tensor_bit_index(src, ctx, fld, context_limit, field_limit);
                        set_tensor_bit(&mut tensor_in, bit);
                    }
                }
            }
        }
        let allow_mask = if case % 13 == 0 {
            0
        } else {
            (1u32 << (case & 7)) | (1u32 << ((case + 5) & 7))
        };

        let expected = explicit_edge_first_reference(
            node_count,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &tensor_in,
            context_limit,
            field_limit,
            allow_mask,
        );
        let actual = try_tensor_flow_forward_cpu(
            node_count,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &tensor_in,
            context_limit,
            field_limit,
            allow_mask,
        )
        .expect("Fix: generated tensor-flow CPU oracle case must be valid");

        assert_eq!(actual, expected, "generated tensor-flow case {case}");
    }
}

#[test]
fn tensor_flow_launch_packs_source_nodes_into_workgroups() {
    let program = try_tensor_flow_forward(
        ProgramGraphShape::new(513, 1),
        "tensor_in",
        "tensor_out",
        2,
        3,
        1,
    )
    .expect("Fix: tensor-flow builder should accept a large node-parallel shape");

    assert_eq!(program.workgroup_size(), TENSOR_FLOW_FORWARD_WORKGROUP_SIZE);
    assert_eq!(tensor_flow_forward_dispatch_grid(0), [1, 1, 1]);
    assert_eq!(tensor_flow_forward_dispatch_grid(1), [1, 1, 1]);
    assert_eq!(tensor_flow_forward_dispatch_grid(256), [1, 1, 1]);
    assert_eq!(tensor_flow_forward_dispatch_grid(257), [2, 1, 1]);
    assert_eq!(tensor_flow_forward_dispatch_grid(513), [3, 1, 1]);
}

#[test]
fn checked_tensor_flow_cpu_into_reuses_output_and_truncates_stale_tail() {
    let mut out = Vec::with_capacity(4);
    out.extend_from_slice(&[99, 98, 97, 96]);
    let capacity = out.capacity();

    try_tensor_flow_forward_cpu_into(2, &[0, 1, 1], &[1], &[1], &[1], 1, 1, 1, &mut out)
        .expect("Fix: valid tensor-flow CPU oracle should reuse output storage");

    assert_eq!(out, vec![0b10]);
    assert_eq!(out.capacity(), capacity);

    try_tensor_flow_forward_cpu_into(1, &[0, 0], &[], &[], &[1], 1, 1, 1, &mut out)
        .expect("Fix: smaller tensor-flow CPU oracle should truncate stale output");

    assert_eq!(out, vec![0]);
    assert_eq!(out.capacity(), capacity);
}

#[test]
fn checked_tensor_flow_builder_rejects_tensor_shape_overflow() {
    let error = try_tensor_flow_forward(
        ProgramGraphShape::new(u32::MAX, 0),
        "tin",
        "tout",
        u32::MAX,
        2,
        1,
    )
    .expect_err("checked tensor-flow builder must reject tensor bit-count overflow");

    assert!(
        error.contains("overflows"),
        "error should describe tensor shape overflow: {error}"
    );
}

#[test]
fn checked_tensor_words_rejects_lane_overflow() {
    let error = super::tensor_flow_forward::try_tensor_words(1, u32::MAX, 2)
        .expect_err("checked tensor word count must reject per-node lane overflow");

    assert!(
        error.contains("overflows per-node tensor lane count"),
        "error should describe per-node tensor lane overflow: {error}"
    );
}

#[test]
fn checked_tensor_flow_cpu_rejects_malformed_csr() {
    let error = try_tensor_flow_forward_cpu(2, &[0, 3, 1], &[0, 1, 0], &[1, 1, 1], &[1], 1, 1, 1)
        .expect_err("checked tensor-flow oracle must reject non-monotonic CSR offsets");

    assert!(
        error.contains("non-monotonic CSR offsets"),
        "error should describe malformed CSR offsets: {error}"
    );
}
