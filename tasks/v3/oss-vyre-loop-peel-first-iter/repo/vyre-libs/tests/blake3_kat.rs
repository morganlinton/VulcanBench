//! BLAKE3 compression-function KAT vectors.
//!
//! Asserts that `vyre-libs::crypto::blake3_compress` produces exactly
//! the output the upstream `blake3` crate's `compress_in_place` produces
//! on the same (chaining, block, counter, block_len, flags) tuple.
//!
//! Source of truth: the BLAKE3 reference implementation at
//! https://github.com/BLAKE3-team/BLAKE3/blob/master/reference_impl/reference_impl.rs
//! and the `blake3` crate which exposes the same permutation.
//!
//! Any change to `blake3_compress`'s IR that diverges from the spec
//! fails this test. No way to silently regress.

#![cfg(feature = "crypto-blake3")]
#![allow(deprecated)]
mod common;
use common::{decode_u32_words, u32_bytes};
use vyre::ir::Program;
use vyre_libs::hash::blake3_compress;
use vyre_reference::value::Value;

/// BLAKE3 IV  -  matches `vyre-libs::crypto::blake3::IV` by spec.
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

/// Flag bits  -  CHUNK_START | CHUNK_END | ROOT for a 1-block input.
const CHUNK_START: u32 = 1;
const CHUNK_END: u32 = 2;
const ROOT: u32 = 8;

fn run_compress(
    program: &Program,
    cv_in: &[u32; 8],
    msg: &[u32; 16],
    params: &[u32; 4],
) -> [u32; 8] {
    let inputs = vec![
        Value::from(u32_bytes(cv_in)),
        Value::from(u32_bytes(msg)),
        Value::from(u32_bytes(params)),
        Value::from(vec![0u8; 8 * 4]),
    ];
    let outputs =
        vyre_reference::reference_eval(program, &inputs).expect("blake3_compress must execute");
    assert_eq!(outputs.len(), 1, "only cv_out buffer is ReadWrite");
    let words = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(words.len(), 8);
    let mut out = [0u32; 8];
    out.copy_from_slice(&words);
    out
}

/// Reference oracle using the upstream `blake3` crate. Hashes a single
/// 64-byte block (or shorter) and returns the 8-word output of the
/// compression function on IV + that block with the given flags.
fn reference_compress(
    cv: &[u32; 8],
    block_words: &[u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [u32; 8] {
    assert_eq!(cv, &IV, "single-block KAT oracle expects the BLAKE3 IV");
    assert_eq!(counter, 0, "single-block KAT oracle expects counter 0");
    assert!(
        block_len <= 64,
        "single-block KAT oracle only accepts one 64-byte block"
    );
    assert_eq!(
        flags,
        CHUNK_START | CHUNK_END | ROOT,
        "single-block KAT oracle expects root chunk flags"
    );
    let mut block = [0u8; 64];
    for (word_index, word) in block_words.iter().enumerate() {
        let start = word_index * 4;
        block[start..start + 4].copy_from_slice(&word.to_le_bytes());
    }
    let hash = ::blake3::hash(&block[..block_len as usize]);
    let bytes = hash.as_bytes();
    let mut out = [0u32; 8];
    for (i, chunk) in bytes.chunks_exact(4).take(8).enumerate() {
        out[i] = u32::from_le_bytes(chunk.try_into().unwrap());
    }
    out
}

/// Block a byte string as 16 little-endian u32 words, zero-padding to 64 bytes.
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

/// Expected output for an empty input: first 32 bytes of blake3::hash(b"").
/// blake3 crate produces deterministic bytes; we compute at runtime.
fn expected_empty_hash_prefix() -> [u32; 8] {
    let hash = ::blake3::hash(b"");
    let bytes = hash.as_bytes();
    let mut out = [0u32; 8];
    for (i, chunk) in bytes.chunks_exact(4).take(8).enumerate() {
        out[i] = u32::from_le_bytes(chunk.try_into().unwrap());
    }
    out
}

#[test]
fn kat_empty_input_matches_blake3_crate() {
    // Empty input: one block, block_len = 0, flags = CHUNK_START | CHUNK_END | ROOT.
    // With `BinOp::RotateRight` wired (P1.4), the reference interpreter
    // runs the full 7-round permutation and must bit-match the official
    // `blake3` crate's output for the empty input.
    let program = blake3_compress("cv_in", "msg", "params", "cv_out");
    let msg = block_words_from_bytes(b"");
    let params = [0, 0, 0, CHUNK_START | CHUNK_END | ROOT];
    let got = run_compress(&program, &IV, &msg, &params);
    let expected = reference_compress(&IV, &msg, 0, 0, CHUNK_START | CHUNK_END | ROOT);
    assert_eq!(expected, expected_empty_hash_prefix());
    assert_eq!(
        got, expected,
        "BLAKE3 KAT empty input: first 32 bytes must match blake3 crate\n  got      = {got:08x?}\n  expected = {expected:08x?}"
    );
}

#[test]
fn kat_abc_matches_blake3_crate() {
    // BLAKE3 hash of "abc": one block, block_len = 3, root flags.
    // Compression-function output = first 32 bytes of blake3::hash("abc").
    let program = blake3_compress("cv_in", "msg", "params", "cv_out");
    let msg = block_words_from_bytes(b"abc");
    let params = [0, 0, 3, CHUNK_START | CHUNK_END | ROOT];
    let got = run_compress(&program, &IV, &msg, &params);

    let expected = reference_compress(&IV, &msg, 0, 3, CHUNK_START | CHUNK_END | ROOT);
    assert_eq!(got, expected, "BLAKE3 KAT `abc` must match blake3 crate");
}

#[test]
fn kat_64_byte_block_matches_blake3_crate() {
    // Exactly-64-byte input: one block, block_len = 64, root flags.
    let input: [u8; 64] = std::array::from_fn(|i| (i as u8).wrapping_mul(7));
    let program = blake3_compress("cv_in", "msg", "params", "cv_out");
    let msg = block_words_from_bytes(&input);
    let params = [0, 0, 64, CHUNK_START | CHUNK_END | ROOT];
    let got = run_compress(&program, &IV, &msg, &params);

    let expected = reference_compress(&IV, &msg, 0, 64, CHUNK_START | CHUNK_END | ROOT);
    assert_eq!(
        got, expected,
        "BLAKE3 KAT 64-byte block must match blake3 crate"
    );
}

#[test]
fn kat_deterministic_across_runs() {
    // Regression guard: whatever the IR produces, two runs on the
    // same input must agree. This catches any hidden nondeterminism
    // (uninitialized reads, racey state, etc.) that would otherwise
    // go undetected until the bit-exact KAT gate lands with P1.4.
    let program = blake3_compress("cv_in", "msg", "params", "cv_out");
    let msg = block_words_from_bytes(b"abc");
    let params = [0, 0, 3, CHUNK_START | CHUNK_END | ROOT];
    let a = run_compress(&program, &IV, &msg, &params);
    let b = run_compress(&program, &IV, &msg, &params);
    assert_eq!(a, b, "blake3_compress on 'abc' must be deterministic");
}

#[test]
fn kat_structural_shape_rotate_right_intrinsic() {
    // Every G-quartet in the BLAKE3 compression emits 4 rotate_right
    // calls for n ∈ {16, 12, 8, 7}. With `BinOp::RotateRight` as a
    // first-class IR node (P1.4), the count is 4 × 8 × 7 = 224 direct
    // rotate nodes. Regression to the (shift-or) idiom, or to bare
    // shifts, fails this test.
    use vyre::ir::{BinOp, Expr, Node};

    fn count_rotate(nodes: &[Node]) -> usize {
        let mut count = 0;
        for node in nodes {
            match node {
                Node::Block(children) => count += count_rotate(children),
                Node::Loop { body, .. } => count += count_rotate(body),
                Node::If {
                    then, otherwise, ..
                } => {
                    count += count_rotate(then) + count_rotate(otherwise);
                }
                Node::Region { body, .. } => count += count_rotate(body),
                Node::Let { value, .. } | Node::Assign { value, .. } => {
                    count += expr_rotate(value);
                }
                Node::Store { value, .. } => count += expr_rotate(value),
                _ => {}
            }
        }
        count
    }
    fn expr_rotate(e: &Expr) -> usize {
        match e {
            Expr::BinOp {
                op: BinOp::RotateRight,
                left,
                right,
            } => 1 + expr_rotate(left) + expr_rotate(right),
            Expr::BinOp { left, right, .. } => expr_rotate(left) + expr_rotate(right),
            Expr::UnOp { operand, .. } => expr_rotate(operand),
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => expr_rotate(cond) + expr_rotate(true_val) + expr_rotate(false_val),
            _ => 0,
        }
    }

    let program = blake3_compress("cv_in", "msg", "params", "cv_out");
    let count = count_rotate(program.entry());
    assert_eq!(count, 224, "BLAKE3 must emit 224 RotateRight IR nodes");
}

#[test]
fn reference_compress_oracle_is_not_zero_stub() {
    let msg = block_words_from_bytes(b"abc");
    let expected = reference_compress(&IV, &msg, 0, 3, CHUNK_START | CHUNK_END | ROOT);
    assert_ne!(expected, [0u32; 8], "BLAKE3 oracle must compute real words");
}
