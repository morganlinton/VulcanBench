//! Generated adversarial coverage for CRC-32 chunk map-reduce algebra.

use vyre_primitives::hash::crc32::{
    crc32, crc32_chunk, crc32_chunk_count, crc32_chunk_output_words, crc32_map_reduce_plan,
    crc32_pack_chunks_u32, crc32_pair_reduce_chunk_words, crc32_pair_reduce_chunks,
    crc32_pair_reduce_output_pairs, crc32_unpack_chunks_u32, Crc32Chunk, Crc32MapReduceStepKind,
};

#[test]
fn generated_crc32_map_reduce_matches_direct_crc_for_8192_cases() {
    let chunk_sizes = [1usize, 2, 3, 5, 7, 16, 31, 64, 127, 257];
    for seed in 0..8192u32 {
        let bytes = generated_bytes(seed);
        let chunk_size = chunk_sizes[seed as usize % chunk_sizes.len()];
        let chunks = bytes
            .chunks(chunk_size)
            .map(crc32_chunk)
            .collect::<Vec<_>>();
        let mut reduced = if chunks.is_empty() {
            vec![Crc32Chunk { len: 0, crc: 0 }]
        } else {
            chunks
        };
        let mut rounds = 0usize;
        while reduced.len() > 1 {
            reduced = crc32_pair_reduce_chunks(&reduced)
                .expect("Fix: generated CRC32 map-reduce lengths must not overflow.");
            rounds += 1;
        }

        assert_eq!(reduced[0].len, bytes.len() as u64, "seed {seed}");
        assert_eq!(reduced[0].crc, crc32(&bytes), "seed {seed}");
        if bytes.len() > chunk_size {
            assert!(
                rounds > 0,
                "Fix: generated multi-chunk CRC case must exercise pair reduction at seed {seed}."
            );
        }
    }
}

#[test]
fn generated_crc32_packed_abi_matches_chunk_reduction_for_8192_cases() {
    let chunk_sizes = [1usize, 4, 9, 32, 128, 511];
    for seed in 0..8192u32 {
        let bytes = generated_bytes(seed ^ 0x5A5A_1234);
        let chunk_size = chunk_sizes[seed as usize % chunk_sizes.len()];
        let chunks = bytes
            .chunks(chunk_size)
            .map(crc32_chunk)
            .collect::<Vec<_>>();
        let chunks = if chunks.is_empty() {
            vec![Crc32Chunk { len: 0, crc: 0 }]
        } else {
            chunks
        };
        let packed = crc32_pack_chunks_u32(&chunks)
            .expect("Fix: generated CRC32 chunks must fit packed ABI.");
        assert_eq!(
            crc32_unpack_chunks_u32(&packed),
            Some(chunks.clone()),
            "seed {seed}"
        );

        let reduced_chunks = crc32_pair_reduce_chunks(&chunks)
            .expect("Fix: generated CRC32 chunk reduction must not overflow.");
        let reduced_words = crc32_pair_reduce_chunk_words(&packed)
            .expect("Fix: generated CRC32 packed reduction must not overflow.");
        assert_eq!(
            crc32_unpack_chunks_u32(&reduced_words),
            Some(reduced_chunks),
            "seed {seed}"
        );
    }
}

#[test]
fn generated_crc32_map_reduce_plan_matches_round_shapes_for_8192_cases() {
    let chunk_sizes = [1u32, 2, 3, 5, 8, 13, 64, 255];
    for seed in 0..8192u32 {
        let input_len = seed.wrapping_mul(97).rotate_left(seed % 19) % 65_536;
        let chunk_size = std::num::NonZeroU32::new(chunk_sizes[seed as usize % chunk_sizes.len()])
            .expect("Fix: generated chunk sizes must be non-zero.");
        let plan = crc32_map_reduce_plan(input_len, chunk_size)
            .expect("Fix: generated CRC32 map-reduce plan must fit u32 shape accounting.");

        assert_eq!(plan.input_len, input_len);
        assert_eq!(plan.chunk_size, chunk_size);
        assert_eq!(
            plan.steps[0].kind,
            Crc32MapReduceStepKind::ChunkSummary,
            "seed {seed}"
        );
        assert_eq!(
            plan.steps[0].output_pairs,
            crc32_chunk_count(input_len, chunk_size.get()),
            "seed {seed}"
        );
        assert_eq!(
            plan.steps[0].output_words,
            crc32_chunk_output_words(input_len, chunk_size.get())
                .expect("Fix: generated chunk output shape must fit."),
            "seed {seed}"
        );

        let mut pairs = plan.steps[0].output_pairs;
        for step in plan.steps.iter().skip(1) {
            assert_eq!(step.kind, Crc32MapReduceStepKind::PairReduce, "seed {seed}");
            assert_eq!(step.input_items, pairs, "seed {seed}");
            assert_eq!(step.output_pairs, pairs.div_ceil(2), "seed {seed}");
            assert_eq!(step.input_words, pairs * 2, "seed {seed}");
            assert_eq!(step.output_words, step.output_pairs * 2, "seed {seed}");
            assert_eq!(step.grid, [step.output_pairs, 1, 1], "seed {seed}");
            pairs = step.output_pairs;
        }
        assert_eq!(pairs, 1, "seed {seed}");
    }
}

#[test]
fn generated_crc32_packed_output_shape_matches_pair_reduce_rounds() {
    for pair_count in 1u32..4096 {
        let output_pairs = crc32_pair_reduce_output_pairs(pair_count);
        assert_eq!(
            output_pairs,
            pair_count.div_ceil(2),
            "Fix: CRC32 pair-reduce output pair count must be ceil(pair_count / 2)."
        );
        assert_eq!(
            output_pairs.checked_mul(2),
            Some(pair_count.div_ceil(2) * 2),
            "Fix: packed CRC32 pair-reduce output shape must remain one `[crc,len]` buffer."
        );
    }

    for (n, chunk_size) in [(0, 64), (1, 64), (63, 64), (64, 64), (65, 64), (1025, 257)] {
        let chunk_count = crc32_chunk_count(n, chunk_size);
        assert_eq!(
            crc32_chunk_output_words(n, chunk_size),
            chunk_count.checked_mul(2),
            "Fix: CRC32 chunk output words must be exactly two words per summary."
        );
    }
}

fn generated_bytes(seed: u32) -> Vec<u8> {
    let len = ((seed.wrapping_mul(73) ^ seed.rotate_left(11)) % 2049) as usize;
    let mut state = u64::from(seed) ^ 0x243f_6a88_85a3_08d3;
    let mut bytes = Vec::with_capacity(len);
    for index in 0..len {
        state ^= state << 7;
        state ^= state >> 9;
        state = state.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        bytes.push((state.rotate_left((index % 63) as u32) & 0xFF) as u8);
    }
    bytes
}
