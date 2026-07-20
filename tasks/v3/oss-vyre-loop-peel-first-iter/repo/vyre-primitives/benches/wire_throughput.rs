#![allow(missing_docs)]
//! Criterion benches for the wire-format pack/unpack primitives.
//!
//! Locks the LE-host `bytemuck::cast_slice` fast path against a naive
//! `flat_map(to_le_bytes)` baseline at 1 KiB, 1 MiB and 100 MiB. The
//! win is bandwidth-bound on LE (the only path that ships), so the
//! ratio is a real regression gate: anyone re-introducing per-word
//! copies will see the bench plateau back down to scalar speeds.
//!
//! Run with:
//! ```text
//! cargo bench -p vyre-primitives --bench wire_throughput
//! ```
//!
//! On an x86_64 LE host the fast path saturates the L3 → DRAM bandwidth;
//! the naive path is ~10x slower at sizes that fit in L2 and stays
//! ~3-5x slower once you spill to DRAM (where the scalar path becomes
//! bandwidth-bound too but pays for an extra register-shuffle per
//! word).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use vyre_primitives::wire::{
    decode_u32_le_bytes_all, pack_u32_slice, pack_u32_slice_into, unpack_u32_slice_into,
};

fn naive_pack_u32(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn naive_unpack_u32(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn bench_pack(c: &mut Criterion) {
    let mut group = c.benchmark_group("wire/pack_u32");
    for &size_words in &[256usize, 256 * 1024, 25 * 1024 * 1024] {
        // 1 KiB, 1 MiB, 100 MiB
        let words: Vec<u32> = (0..size_words as u32)
            .map(|i| i.wrapping_mul(0x0101_0101))
            .collect();
        group.throughput(Throughput::Bytes((size_words * 4) as u64));
        group.bench_with_input(BenchmarkId::new("wire", size_words), &words, |b, words| {
            b.iter(|| pack_u32_slice(black_box(words)))
        });
        group.bench_with_input(
            BenchmarkId::new("naive_flat_map", size_words),
            &words,
            |b, words| b.iter(|| naive_pack_u32(black_box(words))),
        );
    }
    group.finish();
}

fn bench_pack_into(c: &mut Criterion) {
    // _into variant skips Vec allocation. Reuses caller storage.
    let mut group = c.benchmark_group("wire/pack_u32_into");
    let words: Vec<u32> = (0..262_144u32).collect();
    let mut buf: Vec<u8> = Vec::with_capacity(words.len() * 4);
    group.throughput(Throughput::Bytes((words.len() * 4) as u64));
    group.bench_function("wire_into", |b| {
        b.iter(|| pack_u32_slice_into(black_box(&words), &mut buf))
    });
    group.finish();
}

fn bench_unpack(c: &mut Criterion) {
    let mut group = c.benchmark_group("wire/unpack_u32");
    for &size_words in &[256usize, 256 * 1024, 25 * 1024 * 1024] {
        let words: Vec<u32> = (0..size_words as u32).collect();
        let bytes = pack_u32_slice(&words);
        group.throughput(Throughput::Bytes((size_words * 4) as u64));
        group.bench_with_input(BenchmarkId::new("wire", size_words), &bytes, |b, bytes| {
            let mut out = Vec::with_capacity(size_words);
            b.iter(|| {
                unpack_u32_slice_into(black_box(bytes), size_words, "bench", black_box(&mut out))
                    .unwrap()
            });
        });
        group.bench_with_input(
            BenchmarkId::new("wire_decode_all", size_words),
            &bytes,
            |b, bytes| b.iter(|| decode_u32_le_bytes_all(black_box(bytes))),
        );
        group.bench_with_input(
            BenchmarkId::new("naive_chunks_exact", size_words),
            &bytes,
            |b, bytes| b.iter(|| naive_unpack_u32(black_box(bytes))),
        );
    }
    group.finish();
}

criterion_group!(benches, bench_pack, bench_pack_into, bench_unpack);
criterion_main!(benches);
