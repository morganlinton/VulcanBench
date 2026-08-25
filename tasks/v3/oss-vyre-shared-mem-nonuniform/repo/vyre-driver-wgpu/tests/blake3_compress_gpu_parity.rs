//! BLAKE3 compression-function parity against the upstream `blake3` crate on
//! the LIVE GPU.
//!
//! `blake3_kat.rs` proves `vyre-libs::hash::blake3_compress` bit-matches the
//! official `blake3` crate — but ONLY through `reference_eval` (the CPU oracle).
//! The program emits 224 `RotateRight` IR nodes, and naga has no native rotate:
//! op_dispatch lowers `RotateRight` as the SYNTHETIC
//! `(x << (s & 31)) | (x >> ((32 - (s & 31)) & 31))` — exactly the computed,
//! multi-step lowering the naga signed-`Modulo` bug proved can be silently wrong
//! on real silicon (a source read is not proof). If the GPU rotate diverged,
//! every BLAKE3 hash computed on the GPU would be silently wrong and NO test
//! would catch it, because the KAT never dispatches to a backend.
//!
//! This runs the real shipped BLAKE3 single-block compression on the 5090 and
//! asserts the 8-word chaining output bit-matches `blake3::hash` — a real-
//! workload end-to-end check, the strongest possible verification of the
//! rotate/xor/add lowering chain under load.

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::hash::blake3_compress;

/// BLAKE3 IV (matches `vyre-libs::hash::blake3` by spec).
const IV: [u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];

const CHUNK_START: u32 = 1;
const CHUNK_END: u32 = 2;
const ROOT: u32 = 8;
const ROOT_FLAGS: u32 = CHUNK_START | CHUNK_END | ROOT;

fn u32_le_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// Block a byte string (<= 64 bytes) as 16 little-endian u32 words, zero-padded.
fn block_words_from_bytes(bytes: &[u8]) -> [u32; 16] {
    assert!(bytes.len() <= 64, "one BLAKE3 block holds 64 bytes");
    let mut padded = [0u8; 64];
    padded[..bytes.len()].copy_from_slice(bytes);
    let mut out = [0u32; 16];
    for (i, chunk) in padded.chunks_exact(4).enumerate() {
        out[i] = u32::from_le_bytes(chunk.try_into().unwrap());
    }
    out
}

/// Reference: first 32 bytes (8 words) of `blake3::hash(input)`. For a single
/// chunk (<= 64 bytes, root flags, counter 0) this is exactly the compression
/// function's chaining output — the same identity `blake3_kat.rs` relies on.
fn reference_hash_words(input: &[u8]) -> [u32; 8] {
    let hash = ::blake3::hash(input);
    let bytes = hash.as_bytes();
    let mut out = [0u32; 8];
    for (i, chunk) in bytes.chunks_exact(4).take(8).enumerate() {
        out[i] = u32::from_le_bytes(chunk.try_into().unwrap());
    }
    out
}

/// Dispatch the real `blake3_compress` program on the GPU for a single block.
fn gpu_compress(backend: &WgpuBackend, input: &[u8]) -> [u32; 8] {
    let program = blake3_compress("cv_in", "msg", "params", "cv_out");
    let msg = block_words_from_bytes(input);
    let params: [u32; 4] = [0, 0, input.len() as u32, ROOT_FLAGS];

    let cv_in = u32_le_bytes(&IV);
    let msg_b = u32_le_bytes(&msg);
    let params_b = u32_le_bytes(&params);
    let cv_out_init = u32_le_bytes(&[0u32; 8]);

    // Buffers in binding order: 0=cv_in(RO), 1=msg(RO), 2=params(RO),
    // 3=cv_out(ReadWrite/output). The readback returns the output buffer(s),
    // so outputs[0] is cv_out — the same shape `reference_eval` returns.
    let outputs = backend
        .dispatch_borrowed(
            &program,
            &[
                cv_in.as_slice(),
                msg_b.as_slice(),
                params_b.as_slice(),
                cv_out_init.as_slice(),
            ],
            &DispatchConfig::default(),
        )
        .expect("Fix: WGPU must dispatch the BLAKE3 compression program.");
    assert_eq!(
        outputs.len(),
        1,
        "BLAKE3 compress exposes exactly one ReadWrite output (cv_out); got {} buffers",
        outputs.len()
    );
    let words: Vec<u32> = outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    assert_eq!(words.len(), 8, "cv_out must be 8 u32 words");
    let mut out = [0u32; 8];
    out.copy_from_slice(&words);
    out
}

fn run_case(input: &[u8], label: &str) {
    let backend = WgpuBackend::acquire()
        .expect("Fix: BLAKE3 GPU parity requires a live GPU backend.");
    let gpu = gpu_compress(&backend, input);
    let expected = reference_hash_words(input);
    assert_ne!(expected, [0u32; 8], "blake3 reference produced all zeros");
    assert_eq!(
        gpu, expected,
        "GPU BLAKE3 compression of {label} diverged from the `blake3` crate — the \
         synthetic RotateRight lowering miscompiles on hardware.\n  \
         gpu      = {gpu:08x?}\n  expected = {expected:08x?}"
    );
}

#[test]
fn blake3_empty_block_matches_crate_on_gpu() {
    run_case(b"", "the empty input");
}

#[test]
fn blake3_abc_matches_crate_on_gpu() {
    run_case(b"abc", "\"abc\"");
}

#[test]
fn blake3_full_64_byte_block_matches_crate_on_gpu() {
    let input: [u8; 64] = std::array::from_fn(|i| (i as u8).wrapping_mul(7));
    run_case(&input, "a full 64-byte block");
}

#[test]
fn blake3_gpu_is_deterministic_across_runs() {
    let backend = WgpuBackend::acquire()
        .expect("Fix: BLAKE3 GPU parity requires a live GPU backend.");
    let a = gpu_compress(&backend, b"abc");
    let b = gpu_compress(&backend, b"abc");
    assert_eq!(a, b, "BLAKE3 GPU compression of 'abc' must be deterministic");
}
