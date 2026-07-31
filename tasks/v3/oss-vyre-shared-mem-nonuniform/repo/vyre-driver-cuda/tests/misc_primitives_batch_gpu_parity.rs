//! Parity tests for a batch of remaining U32 primitives:
//! bitset_popcount, predicate::edge, line_splice_classify,
//! planar_rewrite_schedule, rle_segment_lengths.

#![cfg(test)]

mod common;
#[path = "common/misc_line_splice_generated.rs"]
mod misc_line_splice_generated;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::bitset::popcount::{bitset_popcount, cpu_ref as popcount_cpu};
use vyre_primitives::decode::rle_segment_lengths::{
    rle_segment_lengths, rle_segment_lengths_cpu, rle_segment_lengths_dispatch_grid,
    MAX_SEGMENT_LENGTH,
};
use vyre_primitives::graph::csr_forward_traverse::csr_forward_traverse_dispatch_grid;
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::parsing::line_splice_classify::{
    line_splice_classify, line_splice_classify_dispatch_grid, line_splice_classify_u8,
    reference_line_splice_classify,
};
use vyre_primitives::parsing::planar_rewrite::{
    planar_rewrite_schedule, reference_planar_rewrite_schedule,
};
use vyre_primitives::predicate::edge::{cpu_ref as edge_cpu, edge};
use vyre_primitives::predicate::edge_kind;

// ---------------------------------------------------------------------
// bitset_popcount: per-word popcount.
// ---------------------------------------------------------------------

fn run_popcount(input: &[u32]) -> Vec<u32> {
    let words = input.len() as u32;
    let program = bitset_popcount("input", "count_words", words);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(input), vec![0u8; words as usize * 4]];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((words + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("bitset popcount batch", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA bitset-popcount dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(words as usize);
    out
}

#[test]
fn cuda_bitset_popcount_basic() {
    let input = vec![0xFFFF_FFFFu32, 0u32, 0b1010_1010_u32, 0xAA55u32];
    let cpu = popcount_cpu(&input);
    let gpu = run_popcount(&input);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![32, 0, 4, 8]);
}

#[test]
fn cuda_bitset_popcount_all_zero() {
    let input = vec![0u32; 16];
    let cpu = popcount_cpu(&input);
    let gpu = run_popcount(&input);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0u32; 16]);
}

// ---------------------------------------------------------------------
// predicate::edge  -  bare CSR forward traversal under a kind mask.
// ---------------------------------------------------------------------

fn run_edge(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier: &[u32],
    allow_mask: u32,
) -> Vec<u32> {
    let words = node_count.div_ceil(32).max(1);
    let pg_nodes = vec![0u32; node_count as usize];
    let pg_node_tags = vec![0u32; node_count as usize];
    let edge_count = edge_targets.len() as u32;
    let program = edge(
        ProgramGraphShape::new(node_count, edge_count.max(1)),
        "frontier_in",
        "frontier_out",
        allow_mask,
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
    let outputs = with_live_backend("predicate edge batch", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA predicate-edge dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(words as usize);
    out
}

#[test]
fn cuda_predicate_edge_one_step() {
    // 0 -> 1 via ASSIGNMENT.
    let edge_offsets = vec![0u32, 1, 1];
    let edge_targets = vec![1u32];
    let edge_kind_mask = vec![edge_kind::ASSIGNMENT];
    let frontier = vec![0b01u32];
    let allow = edge_kind::ASSIGNMENT;
    let cpu = edge_cpu(
        2,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        allow,
    );
    let gpu = run_edge(
        2,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        allow,
    );
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0b10u32]);
}

#[test]
fn cuda_predicate_edge_kind_mask_skips() {
    let edge_offsets = vec![0u32, 1, 1];
    let edge_targets = vec![1u32];
    let edge_kind_mask = vec![edge_kind::ASSIGNMENT];
    let frontier = vec![0b01u32];
    let allow = edge_kind::CALL_ARG;
    let cpu = edge_cpu(
        2,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        allow,
    );
    let gpu = run_edge(
        2,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        allow,
    );
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0u32]);
}

#[test]
fn cuda_predicate_edge_reaches_source_past_first_workgroup() {
    let node_count = 513u32;
    let words = node_count.div_ceil(32) as usize;
    let mut edge_offsets = vec![0u32; node_count as usize + 1];
    for offset in edge_offsets.iter_mut().skip(301) {
        *offset = 1;
    }
    let edge_targets = vec![512u32];
    let edge_kind_mask = vec![edge_kind::ASSIGNMENT];
    let mut frontier = vec![0u32; words];
    frontier[300 / 32] |= 1u32 << (300 % 32);
    let allow = edge_kind::ASSIGNMENT;

    let cpu = edge_cpu(
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        allow,
    );
    let gpu = run_edge(
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        allow,
    );

    let mut expected = vec![0u32; words];
    expected[512 / 32] |= 1u32 << (512 % 32);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, expected);
}

// ---------------------------------------------------------------------
// line_splice_classify
// ---------------------------------------------------------------------

fn pack_bytes(bytes: &[u8]) -> Vec<u32> {
    let mut padded = bytes.to_vec();
    if padded.is_empty() {
        padded.push(0);
    }
    while padded.len() % 4 != 0 {
        padded.push(0);
    }
    padded
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn run_line_splice(source: &[u8]) -> Vec<u32> {
    let byte_count = source.len() as u32;
    let words = pack_bytes(source);
    let program = line_splice_classify(byte_count);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(&words), vec![0u8; byte_count.max(1) as usize * 4]];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(line_splice_classify_dispatch_grid(byte_count));
    let outputs = with_live_backend("line splice classify", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA line-splice classify dispatch failed: {error}")
            })
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(byte_count as usize);
    out
}

fn run_line_splice_u8(source: &[u8]) -> Vec<u32> {
    let byte_count = source.len() as u32;
    let program = line_splice_classify_u8(byte_count);
    let inputs: Vec<Vec<u8>> = vec![source.to_vec(), vec![0u8; byte_count.max(1) as usize * 4]];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(line_splice_classify_dispatch_grid(byte_count));
    let outputs = with_live_backend("raw-u8 line splice classify", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA raw-u8 line-splice classify dispatch failed: {error}")
            })
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(byte_count as usize);
    out
}

#[test]
fn cuda_line_splice_classify_keeps_plain_text() {
    let source = b"abcd";
    let cpu = reference_line_splice_classify(source);
    let gpu = run_line_splice(source);
    let gpu_u8 = run_line_splice_u8(source);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
    assert_eq!(gpu, vec![1u32, 1, 1, 1]);
    assert_eq!(gpu_u8, vec![1u32, 1, 1, 1]);
}

#[test]
fn cuda_line_splice_classify_drops_backslash_lf() {
    // "ab\\\ncd"  -  backslash + LF should be dropped (kept_mask = 0).
    let source = b"ab\\\ncd";
    let cpu = reference_line_splice_classify(source);
    let gpu = run_line_splice(source);
    let gpu_u8 = run_line_splice_u8(source);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
}

#[test]
fn cuda_line_splice_classify_empty_input() {
    let source = b"";
    let cpu = reference_line_splice_classify(source);
    let gpu = run_line_splice(source);
    let gpu_u8 = run_line_splice_u8(source);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
    assert!(gpu.is_empty());
    assert!(gpu_u8.is_empty());
}

#[test]
fn cuda_line_splice_classify_drops_backslash_cr_lf() {
    let source = b"a\\\r\nb";
    let cpu = reference_line_splice_classify(source);
    let gpu = run_line_splice(source);
    let gpu_u8 = run_line_splice_u8(source);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
    assert_eq!(gpu, vec![1, 0, 0, 0, 1]);
    assert_eq!(gpu_u8, vec![1, 0, 0, 0, 1]);
}

#[test]
fn cuda_line_splice_classify_crosses_packed_word_boundary() {
    let source = b"abc\\\nz";
    let cpu = reference_line_splice_classify(source);
    let gpu = run_line_splice(source);
    let gpu_u8 = run_line_splice_u8(source);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
    assert_eq!(gpu, vec![1, 1, 1, 0, 0, 1]);
    assert_eq!(gpu_u8, vec![1, 1, 1, 0, 0, 1]);
}

#[test]
fn cuda_line_splice_classify_crosses_workgroup_boundary() {
    let mut source = vec![b'x'; 260];
    source[254] = b'\\';
    source[255] = b'\r';
    source[256] = b'\n';

    let cpu = reference_line_splice_classify(&source);
    let gpu = run_line_splice(&source);
    let gpu_u8 = run_line_splice_u8(&source);

    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
    assert_eq!(&gpu[252..258], &[1, 1, 0, 0, 0, 1]);
    assert_eq!(&gpu_u8[252..258], &[1, 1, 0, 0, 0, 1]);
}

#[test]
fn cuda_line_splice_classify_generated_multi_block_corpus() {
    let mut source = Vec::with_capacity(4101);
    while source.len() < 4101 {
        let line = source.len() / 53;
        match line % 6 {
            0 => source.extend_from_slice(b"#define JOIN(a, b) \\\n  a ## b\n"),
            1 => source.extend_from_slice(b"char slash = '\\\\';\n"),
            2 => source.extend_from_slice(b"int crlf = 1;\\\r\nint next = 2;\n"),
            3 => source.extend_from_slice(b"plain tokens with / at end /\n"),
            4 => source.extend_from_slice(b"continued\\\rmac_style\n"),
            _ => source.extend_from_slice(b"two\\\\\nslashes\n"),
        }
    }
    source.truncate(4101);

    let cpu = reference_line_splice_classify(&source);
    let gpu = run_line_splice(&source);
    let gpu_u8 = run_line_splice_u8(&source);

    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
    assert!(
        gpu.iter().any(|kept| *kept == 0),
        "Fix: generated CUDA line-splice corpus must exercise deleted bytes."
    );
    assert!(
        gpu.iter().any(|kept| *kept == 1),
        "Fix: generated CUDA line-splice corpus must exercise kept bytes."
    );
}

// ---------------------------------------------------------------------
// planar_rewrite_schedule
// ---------------------------------------------------------------------

fn run_planar(candidates: &[u32], h: u32, w: u32, k: u32) -> Vec<u32> {
    let cells = (h * w) as usize;
    let program = planar_rewrite_schedule("c", "ch", h, w, k);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(candidates), vec![0u8; cells * 4]];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((cells as u32 + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("planar rewrite schedule", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA planar-rewrite dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(cells);
    out
}

#[test]
fn cuda_planar_rewrite_schedule_no_candidates() {
    let h = 3u32;
    let w = 3u32;
    let k = 1u32;
    let candidates = vec![0u32; (h * w) as usize];
    let cpu = reference_planar_rewrite_schedule(&candidates, h, w, k);
    let gpu = run_planar(&candidates, h, w, k);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0u32; (h * w) as usize]);
}

#[test]
fn cuda_planar_rewrite_schedule_isolated_candidates() {
    let h = 4u32;
    let w = 4u32;
    let k = 1u32;
    // Diagonal candidates spaced by 2  -  none touch each other within k=1.
    let mut candidates = vec![0u32; (h * w) as usize];
    candidates[0] = 1;
    candidates[10] = 1;
    let cpu = reference_planar_rewrite_schedule(&candidates, h, w, k);
    let gpu = run_planar(&candidates, h, w, k);
    assert_eq!(gpu, cpu);
}

// ---------------------------------------------------------------------
// rle_segment_lengths
// ---------------------------------------------------------------------

fn run_rle(segments: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let count = segments.len() as u32;
    let program = rle_segment_lengths(count);
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(segments),
        vec![0u8; count as usize * 4],
        vec![0u8; count as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(rle_segment_lengths_dispatch_grid(count));
    let outputs = with_live_backend("RLE segment lengths", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA RLE segment-length dispatch failed: {error}"))
    });
    let mut lengths = bytes_u32(&outputs[0]);
    let mut values = bytes_u32(&outputs[1]);
    lengths.truncate(count as usize);
    values.truncate(count as usize);
    (lengths, values)
}

#[test]
fn cuda_rle_segment_lengths_basic() {
    // pack (length=5, value=0xAA) and (length=10, value=0x55).
    let segments = vec![(5u32 << 8) | 0xAA, (10u32 << 8) | 0x55];
    let (cpu_lengths, cpu_values) = rle_segment_lengths_cpu(&segments);
    let (gpu_lengths, gpu_values) = run_rle(&segments);
    assert_eq!(gpu_lengths, cpu_lengths);
    assert_eq!(gpu_values, cpu_values);
    assert_eq!(gpu_lengths, vec![5, 10]);
    assert_eq!(gpu_values, vec![0xAA, 0x55]);
}

#[test]
fn cuda_rle_segment_lengths_zero_length() {
    let segments = vec![0u32, (1u32 << 8) | 0xFF];
    let (cpu_lengths, cpu_values) = rle_segment_lengths_cpu(&segments);
    let (gpu_lengths, gpu_values) = run_rle(&segments);
    assert_eq!(gpu_lengths, cpu_lengths);
    assert_eq!(gpu_values, cpu_values);
    assert_eq!(gpu_lengths, vec![0, 1]);
    assert_eq!(gpu_values, vec![0, 0xFF]);
}

#[test]
fn cuda_rle_segment_lengths_multi_block_mixed_runs() {
    let count = 1025u32;
    let mut segments = Vec::with_capacity(count as usize);
    for idx in 0..count {
        let length = match idx {
            0 => MAX_SEGMENT_LENGTH,
            255 => 0,
            256 => 1,
            511 => 4096,
            512 => 7,
            1024 => MAX_SEGMENT_LENGTH - 1,
            _ => (idx.wrapping_mul(17) ^ idx.rotate_left(3)) & 0x1FFF,
        };
        let value = (idx.wrapping_mul(37) ^ idx.rotate_right(5)) & 0xFF;
        segments.push((length << 8) | value);
    }

    let (cpu_lengths, cpu_values) = rle_segment_lengths_cpu(&segments);
    let (gpu_lengths, gpu_values) = run_rle(&segments);

    assert_eq!(rle_segment_lengths_dispatch_grid(count), [5, 1, 1]);
    assert_eq!(gpu_lengths, cpu_lengths);
    assert_eq!(gpu_values, cpu_values);
    assert_eq!(gpu_lengths[0], MAX_SEGMENT_LENGTH);
    assert_eq!(gpu_lengths[255], 0);
    assert_eq!(gpu_lengths[256], 1);
    assert_eq!(gpu_lengths[1024], MAX_SEGMENT_LENGTH - 1);
    assert_eq!(
        gpu_values[512],
        (512u32.wrapping_mul(37) ^ 512u32.rotate_right(5)) & 0xFF
    );
}
