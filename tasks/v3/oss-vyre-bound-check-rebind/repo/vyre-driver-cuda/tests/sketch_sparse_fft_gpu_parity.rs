//! Parity tests for hash::count_sketch_update + hash::sparse_fft_bin_hash.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_primitives::hash::sketch::{count_sketch_update, count_sketch_update_cpu};
use vyre_primitives::hash::sparse_fft::{sparse_fft_bin_hash, sparse_fft_bin_hash_cpu};

// ---------------------------------------------------------------------
// count_sketch_update
// ---------------------------------------------------------------------

fn run_count_sketch_update(
    backend: &CudaBackend,
    table_seed: &[u32],
    hashes: &[u32],
    signs_u32: &[u32],
    d: u32,
    w: u32,
) -> Vec<u32> {
    let program = count_sketch_update("table", "hashes", "signs", d, w);
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(table_seed),
        u32_bytes(hashes),
        u32_bytes(signs_u32),
    ];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((d + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    let mut out = bytes_u32(&outputs[0]);
    out.truncate((d * w) as usize);
    out
}

#[test]
fn cuda_count_sketch_update_positive_signs() {
    with_live_backend("cuda_count_sketch_update_positive_signs", |backend| {
        let d = 3u32;
        let w = 4u32;
        // Three rows, each writing +1 at a different column.
        let table_seed = vec![0u32; (d * w) as usize];
        let hashes = vec![0u32, 2, 3];
        let signs = vec![1u32, 1, 1];
        let mut cpu = table_seed.clone();
        let signs_i32: Vec<i32> = signs.iter().map(|s| *s as i32).collect();
        count_sketch_update_cpu(&mut cpu, &hashes, &signs_i32, d, w);
        let gpu = run_count_sketch_update(backend, &table_seed, &hashes, &signs, d, w);
        assert_eq!(gpu, cpu);
        let mut expected = vec![0u32; (d * w) as usize];
        expected[0] = 1;
        expected[w as usize + 2] = 1;
        expected[2 * w as usize + 3] = 1;
        assert_eq!(gpu, expected);
    });
}

#[test]
fn cuda_count_sketch_update_negative_sign_via_twos_complement() {
    with_live_backend(
        "cuda_count_sketch_update_negative_sign_via_twos_complement",
        |backend| {
            let d = 1u32;
            let w = 4u32;
            let mut table_seed = vec![10u32, 20, 30, 40];
            let hashes = vec![1u32];
            // sign = -1 encoded as 0xFFFF_FFFF.
            let signs = vec![0xFFFF_FFFFu32];
            let signs_i32 = vec![-1i32];
            let mut cpu = table_seed.clone();
            count_sketch_update_cpu(&mut cpu, &hashes, &signs_i32, d, w);
            let gpu = run_count_sketch_update(backend, &table_seed, &hashes, &signs, d, w);
            assert_eq!(gpu, cpu);
            table_seed[1] = 19;
            assert_eq!(gpu, table_seed);
        },
    );
}

#[test]
fn cuda_count_sketch_update_accumulates_existing_table() {
    with_live_backend(
        "cuda_count_sketch_update_accumulates_existing_table",
        |backend| {
            let d = 2u32;
            let w = 3u32;
            let table_seed = vec![5u32, 5, 5, 7, 7, 7];
            let hashes = vec![1u32, 2];
            let signs = vec![1u32, 1];
            let mut cpu = table_seed.clone();
            let signs_i32 = vec![1i32, 1];
            count_sketch_update_cpu(&mut cpu, &hashes, &signs_i32, d, w);
            let gpu = run_count_sketch_update(backend, &table_seed, &hashes, &signs, d, w);
            assert_eq!(gpu, cpu);
        },
    );
}

// ---------------------------------------------------------------------
// sparse_fft_bin_hash
// ---------------------------------------------------------------------

fn run_sparse_fft(backend: &CudaBackend, signal: &[u32], a: u32, c: u32, b: u32) -> Vec<u32> {
    let n = signal.len() as u32;
    let program = sparse_fft_bin_hash("signal", "bins", a, c, b, n);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(signal), vec![0u8; b as usize * 4]];
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 1, 1]);
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(b as usize);
    out
}

#[test]
fn cuda_sparse_fft_bin_hash_basic() {
    with_live_backend("cuda_sparse_fft_bin_hash_basic", |backend| {
        let signal = vec![10u32, 20, 30, 40, 50, 60, 70, 80];
        let a = 1u32;
        let c = 0u32;
        let b = 4u32;
        let cpu = sparse_fft_bin_hash_cpu(&signal, a, c, b);
        let gpu = run_sparse_fft(backend, &signal, a, c, b);
        assert_eq!(gpu, cpu);
    });
}

#[test]
fn cuda_sparse_fft_bin_hash_a3_c1() {
    with_live_backend("cuda_sparse_fft_bin_hash_a3_c1", |backend| {
        let signal = vec![1u32, 2, 3, 4, 5];
        let a = 3u32;
        let c = 1u32;
        let b = 5u32;
        let cpu = sparse_fft_bin_hash_cpu(&signal, a, c, b);
        let gpu = run_sparse_fft(backend, &signal, a, c, b);
        assert_eq!(gpu, cpu);
    });
}

#[test]
fn cuda_sparse_fft_bin_hash_zero_signal_returns_zero_bins() {
    with_live_backend(
        "cuda_sparse_fft_bin_hash_zero_signal_returns_zero_bins",
        |backend| {
            let signal = vec![0u32; 8];
            let cpu = sparse_fft_bin_hash_cpu(&signal, 7, 3, 4);
            let gpu = run_sparse_fft(backend, &signal, 7, 3, 4);
            assert_eq!(gpu, cpu);
            assert_eq!(gpu, vec![0u32; 4]);
        },
    );
}
