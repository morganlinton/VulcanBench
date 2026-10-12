//! Parity tests for hash::fnv1a32_program, parsing::whitespace_classify_word,
//! hash::hypervector_xor_bind.

#![cfg(test)]

mod common;
#[path = "common/hash_parsing_fnv.rs"]
mod hash_parsing_fnv;

use common::{bytes_u32, u32_bytes, with_live_backend};
use std::num::NonZeroU32;
use vyre::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_primitives::hash::crc32::{
    crc32, crc32_chunk_program, crc32_combine_chunks, crc32_map_reduce_plan, crc32_pack_chunks_u32,
    crc32_pair_reduce_program, crc32_unpack_chunks_u32, Crc32Chunk, Crc32MapReduceStep,
    Crc32MapReduceStepKind,
};
use vyre_primitives::hash::hypervector::hypervector_xor_bind;
use vyre_primitives::parsing::whitespace_classify_word::{
    reference_whitespace_classify_word, whitespace_classify_word,
    whitespace_classify_word_dispatch_grid,
};

// ---------------------------------------------------------------------
// crc32_chunk_program
//
// Each invocation summarizes one fixed-size byte block as `(crc, len)`.
// The host-side combine is associative, so this is the GPU release path for
// turning serial CRC-32 into block-parallel summaries.
// ---------------------------------------------------------------------

fn run_crc32_chunks(
    backend: &CudaBackend,
    bytes: &[u8],
    chunk_size: NonZeroU32,
    step: Crc32MapReduceStep,
) -> Vec<Crc32Chunk> {
    assert_eq!(step.kind, Crc32MapReduceStepKind::ChunkSummary);
    let mut words: Vec<u32> = bytes.iter().map(|byte| u32::from(*byte)).collect();
    if words.is_empty() {
        words.push(0);
    }
    let n = bytes.len() as u32;
    let program = crc32_chunk_program("input", "out", n, chunk_size);
    let inputs = vec![u32_bytes(&words)];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(step.grid);
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("Fix: CUDA CRC32 chunk summary dispatch must succeed.");
    let mut pairs = bytes_u32(&outputs[0]);
    pairs.truncate(step.output_words as usize);
    crc32_unpack_chunks_u32(&pairs)
        .expect("Fix: CUDA CRC32 chunk summary output must use the primitive packed ABI.")
}

fn reduce_crc32_chunks(chunks: &[Crc32Chunk]) -> Crc32Chunk {
    chunks
        .iter()
        .copied()
        .reduce(|left, right| {
            crc32_combine_chunks(left, right)
                .expect("Fix: CUDA CRC32 generated chunk lengths must not overflow.")
        })
        .expect("Fix: CUDA CRC32 chunk program must always emit at least one summary.")
}

fn run_crc32_pair_reduce(
    backend: &CudaBackend,
    chunks: &[Crc32Chunk],
    step: Crc32MapReduceStep,
) -> Vec<Crc32Chunk> {
    assert_eq!(step.kind, Crc32MapReduceStepKind::PairReduce);
    let pair_count =
        NonZeroU32::new(chunks.len() as u32).expect("Fix: CRC32 pair-reduce input is non-empty.");
    assert_eq!(step.input_items, pair_count.get());
    let words = crc32_pack_chunks_u32(chunks)
        .expect("Fix: CUDA CRC32 test chunks must fit the primitive packed ABI.");
    let program = crc32_pair_reduce_program("pairs", "reduced", pair_count);
    let inputs = vec![u32_bytes(&words)];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(step.grid);
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("Fix: CUDA CRC32 pair-reduce dispatch must succeed.");
    let mut pairs = bytes_u32(&outputs[0]);
    pairs.truncate(step.output_words as usize);
    crc32_unpack_chunks_u32(&pairs)
        .expect("Fix: CUDA CRC32 pair-reduce output must use the primitive packed ABI.")
}

fn run_crc32_map_reduce_on_gpu(
    backend: &CudaBackend,
    bytes: &[u8],
    chunk_size: NonZeroU32,
) -> (Crc32Chunk, usize) {
    let plan = crc32_map_reduce_plan(bytes.len() as u32, chunk_size)
        .expect("Fix: CUDA CRC32 map-reduce plan should fit u32 shape accounting.");
    let mut chunks = run_crc32_chunks(backend, bytes, chunk_size, plan.steps[0]);
    let mut rounds = 0usize;
    for step in plan.steps.iter().copied().skip(1) {
        chunks = run_crc32_pair_reduce(backend, &chunks, step);
        rounds += 1;
    }

    assert_eq!(
        chunks.len(),
        1,
        "Fix: CUDA CRC32 map-reduce must converge to one summary chunk."
    );
    (chunks[0], rounds)
}

#[test]
fn cuda_crc32_chunk_summaries_reduce_to_direct_crc() {
    with_live_backend(
        "cuda_crc32_chunk_summaries_reduce_to_direct_crc",
        |backend| {
            let bytes = (0..1500)
                .map(|index| (index as u8).wrapping_mul(29).wrapping_add(7))
                .collect::<Vec<_>>();
            let chunk_size = NonZeroU32::new(64).expect("Fix: chunk size must be non-zero.");
            let plan = crc32_map_reduce_plan(bytes.len() as u32, chunk_size)
                .expect("Fix: CUDA CRC32 chunk plan should fit u32 shape accounting.");

            let chunks = run_crc32_chunks(backend, &bytes, chunk_size, plan.steps[0]);
            let reduced = reduce_crc32_chunks(&chunks);

            assert_eq!(reduced.len, bytes.len() as u64);
            assert_eq!(reduced.crc, crc32(&bytes));
            assert!(
                chunks.iter().all(|chunk| chunk.len <= 64),
                "Fix: CUDA CRC32 chunk summaries must not read past their assigned byte block."
            );
        },
    );
}

#[test]
fn cuda_crc32_chunk_summaries_reduce_on_gpu_to_direct_crc() {
    with_live_backend(
        "cuda_crc32_chunk_summaries_reduce_on_gpu_to_direct_crc",
        |backend| {
            let bytes = (0..160)
                .map(|index| (index as u8).wrapping_mul(17).wrapping_add(0x5B))
                .collect::<Vec<_>>();
            let chunk_size = NonZeroU32::new(96).expect("Fix: chunk size must be non-zero.");
            let (summary, rounds) = run_crc32_map_reduce_on_gpu(backend, &bytes, chunk_size);

            assert!(
                rounds > 0,
                "Fix: CUDA CRC32 map-reduce test must exercise at least one GPU reduction round."
            );
            assert_eq!(summary.len, bytes.len() as u64);
            assert_eq!(summary.crc, crc32(&bytes));
        },
    );
}

#[test]
fn cuda_crc32_map_reduce_generated_live_matrix_matches_direct_crc() {
    with_live_backend(
        "cuda_crc32_map_reduce_generated_live_matrix_matches_direct_crc",
        |backend| {
            let cases = [(0u32, 0usize, 64u32), (1, 1, 64), (2, 160, 31)];
            let mut multi_round_cases = 0usize;

            for (seed, len, chunk_size) in cases {
                let bytes = generated_crc32_bytes(seed, len);
                let chunk_size = NonZeroU32::new(chunk_size)
                    .expect("Fix: generated chunk sizes must be non-zero.");
                let (summary, rounds) = run_crc32_map_reduce_on_gpu(backend, &bytes, chunk_size);

                if rounds > 1 {
                    multi_round_cases += 1;
                }
                assert_eq!(summary.len, bytes.len() as u64, "seed {seed}");
                assert_eq!(summary.crc, crc32(&bytes), "seed {seed}");
            }

            assert!(
                multi_round_cases >= 1,
                "Fix: generated CUDA CRC32 matrix must exercise multiple GPU reduction rounds."
            );
        },
    );
}

#[test]
fn cuda_crc32_chunk_empty_input_stays_reducible() {
    with_live_backend("cuda_crc32_chunk_empty_input_stays_reducible", |backend| {
        let chunk_size = NonZeroU32::new(32).expect("Fix: chunk size must be non-zero.");
        let plan = crc32_map_reduce_plan(0, chunk_size)
            .expect("Fix: empty CUDA CRC32 map-reduce plan should be representable.");

        let chunks = run_crc32_chunks(backend, b"", chunk_size, plan.steps[0]);
        let reduced = reduce_crc32_chunks(&chunks);

        assert_eq!(chunks.len(), 1);
        assert_eq!(reduced, Crc32Chunk { len: 0, crc: 0 });
    });
}

fn generated_crc32_bytes(seed: u32, len: usize) -> Vec<u8> {
    let mut state = u64::from(seed) ^ 0xa409_3822_299f_31d0;
    let mut bytes = Vec::with_capacity(len);
    for index in 0..len {
        state ^= state << 7;
        state ^= state >> 9;
        state = state.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        bytes.push((state.rotate_left((index % 63) as u32) & 0xFF) as u8);
    }
    bytes
}

// ---------------------------------------------------------------------
// whitespace_classify_word
// ---------------------------------------------------------------------

fn run_whitespace_classify(backend: &CudaBackend, words: &[u32]) -> Vec<u32> {
    let n = words.len() as u32;
    let program = whitespace_classify_word(n);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(words), vec![0u8; n as usize * 4]];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(whitespace_classify_word_dispatch_grid(n));
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(n as usize);
    out
}

fn pack_bytes(bytes: &[u8]) -> Vec<u32> {
    let mut padded = bytes.to_vec();
    while padded.len() % 4 != 0 {
        padded.push(0);
    }
    padded
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn cuda_whitespace_classify_all_spaces() {
    with_live_backend("cuda_whitespace_classify_all_spaces", |backend| {
        let bytes = b"    ";
        let words = pack_bytes(bytes);
        let cpu = reference_whitespace_classify_word(&words);
        let gpu = run_whitespace_classify(backend, &words);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu[0], 0xF); // all 4 byte lanes whitespace.
    });
}

#[test]
fn cuda_whitespace_classify_mixed_text() {
    with_live_backend("cuda_whitespace_classify_mixed_text", |backend| {
        // "ab c\nx\ty "  -  8 bytes: 2 words.
        let bytes = b"ab c\nx\ty ";
        let words = pack_bytes(bytes);
        let cpu = reference_whitespace_classify_word(&words);
        let gpu = run_whitespace_classify(backend, &words);
        assert_eq!(gpu, cpu);
    });
}

#[test]
fn cuda_whitespace_classify_no_whitespace() {
    with_live_backend("cuda_whitespace_classify_no_whitespace", |backend| {
        let bytes = b"abcd";
        let words = pack_bytes(bytes);
        let cpu = reference_whitespace_classify_word(&words);
        let gpu = run_whitespace_classify(backend, &words);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu[0], 0);
    });
}

#[test]
fn cuda_whitespace_classify_generated_multi_block_corpus() {
    with_live_backend(
        "cuda_whitespace_classify_generated_multi_block_corpus",
        |backend| {
            let bytes = (0..4100u32)
                .map(|index| match index % 37 {
                    0 => b' ',
                    5 => b'\t',
                    17 => b'\n',
                    23 => b'\r',
                    _ => index.wrapping_mul(19).wrapping_add(0x41) as u8,
                })
                .collect::<Vec<_>>();
            let words = pack_bytes(&bytes);
            let cpu = reference_whitespace_classify_word(&words);
            let gpu = run_whitespace_classify(backend, &words);

            assert_eq!(words.len(), 1025);
            assert_eq!(gpu, cpu);
            assert!(
                gpu.iter().any(|mask| *mask == 0),
                "Fix: generated whitespace corpus must include non-whitespace-only words."
            );
            assert!(
                gpu.iter().any(|mask| *mask != 0),
                "Fix: generated whitespace corpus must include whitespace lanes."
            );
        },
    );
}

// ---------------------------------------------------------------------
// hypervector_xor_bind
// ---------------------------------------------------------------------

fn run_xor_bind(backend: &CudaBackend, a: &[u32], b: &[u32]) -> Vec<u32> {
    assert_eq!(a.len(), b.len());
    let dim_words = a.len() as u32;
    let program = hypervector_xor_bind("a", "b", "out", dim_words);
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(a),
        u32_bytes(b),
        vec![0u8; dim_words as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((dim_words + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(dim_words as usize);
    out
}

#[test]
fn cuda_hypervector_xor_bind_basic() {
    with_live_backend("cuda_hypervector_xor_bind_basic", |backend| {
        let a = vec![0xAAAA_AAAAu32, 0u32, 0xFFFF_FFFFu32];
        let b = vec![0x5555_5555u32, 0xAAAA_AAAAu32, 0xFFFF_FFFFu32];
        let cpu: Vec<u32> = a.iter().zip(b.iter()).map(|(x, y)| x ^ y).collect();
        let gpu = run_xor_bind(backend, &a, &b);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu[0], 0xFFFF_FFFF);
        assert_eq!(gpu[1], 0xAAAA_AAAA);
        assert_eq!(gpu[2], 0); // self-XOR is zero.
    });
}

#[test]
fn cuda_hypervector_xor_bind_zero_lhs_returns_rhs() {
    with_live_backend(
        "cuda_hypervector_xor_bind_zero_lhs_returns_rhs",
        |backend| {
            let a = vec![0u32; 4];
            let b = vec![0xDEAD_BEEFu32, 0xCAFE_BABE, 0xFEEDu32, 0x1234_5678];
            let cpu: Vec<u32> = a.iter().zip(b.iter()).map(|(x, y)| x ^ y).collect();
            let gpu = run_xor_bind(backend, &a, &b);
            assert_eq!(gpu, cpu);
            assert_eq!(gpu, b);
        },
    );
}
