//! Vector Symbolic Architecture (VSA) primitives  -  bind + bundle on
//! high-dimensional binary hypervectors.
//!
//! VSAs (Plate 1995, Kanerva 2009) compute over 10K-dim ±1 / 0/1
//! hypervectors using two operations: *binding* (associates two
//! vectors into a key-value pair) and *bundling* (superposes a set of
//! vectors into a single representative). Recent ML work (Schlegel
//! 2022, Hersche 2023) shows VSA + transformers > transformers alone
//! on systematic-generalization benchmarks.
//!
//! This file ships the **binary spatter code** (BSC) variant: each
//! hypervector is a u32 bitset, binding is bitwise XOR, bundling is
//! per-bit majority vote. Already GPU-trivial; the gravity gap is
//! that no one has packaged it as a Tier-2.5 primitive.
//!
//! # Why this primitive is dual-use
//!
//! | Composition role | Use |
//! |---|---|
//! | retrieval | structured key-value lookup |
//! | symbolic reasoning | compositional symbol algebra |
//! | program fingerprints | bind op-kind, buffer signature, and region shape into one hypervector so semantically-equivalent regions can share cache entries even when byte-equal hashing misses |
//!
//! # Operations
//!
//! - `hypervector_xor_bind(a, b, out, dim_words)`  -  bitwise XOR.
//!   Each output word is `a[i] ^ b[i]`. XOR is its own inverse, so
//!   `xor_bind(xor_bind(a, b), b) == a` (unbinding by re-binding with
//!   the same key).
//! - `hypervector_majority_bundle(stacked, out, dim_words, k)`  -
//!   per-bit majority over `k` stacked hypervectors. For each bit
//!   position, output bit = 1 iff > k/2 input bits are 1. Ties
//!   (k even, exactly k/2) round to 0 (callers typically use odd k).

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id for the binding primitive.
pub const BIND_OP_ID: &str = "vyre-primitives::hash::hypervector_xor_bind";
/// Canonical op id for the bundling primitive.
pub const BUNDLE_OP_ID: &str = "vyre-primitives::hash::hypervector_majority_bundle";

/// Standard BSC hypervector dimensionality (in bits). 10240 bits =
/// 320 u32 words. Plate / Kanerva established that dimensions in the
/// 10K range give negligible chance-binding noise for practical
/// vocabularies up to ~10⁶ items.
pub const STANDARD_DIM_BITS: u32 = 10240;
/// Standard hypervector size in u32 words.
pub const STANDARD_DIM_WORDS: u32 = STANDARD_DIM_BITS / 32;

/// Emit `out`w` = a`w` ^ b`w`` for each of `dim_words` lanes.
#[must_use]
pub fn hypervector_xor_bind(a: &str, b: &str, out: &str, dim_words: u32) -> Program {
    if dim_words == 0 {
        return crate::invalid_output_program(
            BIND_OP_ID,
            out,
            DataType::U32,
            "Fix: hypervector_xor_bind requires dim_words > 0, got 0.".to_string(),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(dim_words)),
        vec![Node::store(
            out,
            t.clone(),
            Expr::bitxor(Expr::load(a, t.clone()), Expr::load(b, t)),
        )],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(a, 0, BufferAccess::ReadOnly, DataType::U32).with_count(dim_words),
            BufferDecl::storage(b, 1, BufferAccess::ReadOnly, DataType::U32).with_count(dim_words),
            BufferDecl::storage(out, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(dim_words),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(BIND_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Emit per-bit majority vote over `k` hypervectors stacked row-major
/// in `stacked` (size `k * dim_words`).
///
/// For each output word lane `w` and each bit position `bit` in 0..32:
///   count = popcount of (stacked[i*dim_words + w] >> bit & 1) for i in 0..k
///   out`w` bit `bit` = 1 iff count > k/2
#[must_use]
pub fn hypervector_majority_bundle(stacked: &str, out: &str, dim_words: u32, k: u32) -> Program {
    if dim_words == 0 {
        return crate::invalid_output_program(
            BUNDLE_OP_ID,
            out,
            DataType::U32,
            "Fix: hypervector_majority_bundle requires dim_words > 0, got 0.".to_string(),
        );
    }
    if k == 0 {
        return crate::invalid_output_program(
            BUNDLE_OP_ID,
            out,
            DataType::U32,
            "Fix: hypervector_majority_bundle requires k > 0, got 0.".to_string(),
        );
    }
    let Some(stacked_words) = k.checked_mul(dim_words) else {
        return crate::invalid_output_program(
            BUNDLE_OP_ID,
            out,
            DataType::U32,
            format!(
                "Fix: hypervector_majority_bundle k*dim_words overflows stacked input count for k={k}, dim_words={dim_words}; shard the bundle before GPU dispatch."
            ),
        );
    };

    let t = Expr::InvocationId { axis: 0 };
    let threshold = k / 2; // ties (count == threshold) round to 0.

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(dim_words)),
        vec![
            Node::let_bind("acc", Expr::u32(0)),
            Node::loop_for(
                "bit",
                Expr::u32(0),
                Expr::u32(32),
                vec![
                    Node::let_bind("count", Expr::u32(0)),
                    Node::loop_for(
                        "ii",
                        Expr::u32(0),
                        Expr::u32(k),
                        vec![
                            Node::let_bind("_unused_assign", Expr::u32(0)),
                            Node::assign(
                                "count",
                                Expr::add(
                                    Expr::var("count"),
                                    Expr::bitand(
                                        Expr::shr(
                                            Expr::load(
                                                stacked,
                                                Expr::add(
                                                    Expr::mul(
                                                        Expr::var("ii"),
                                                        Expr::u32(dim_words),
                                                    ),
                                                    t.clone(),
                                                ),
                                            ),
                                            Expr::var("bit"),
                                        ),
                                        Expr::u32(1),
                                    ),
                                ),
                            ),
                        ],
                    ),
                    Node::if_then(
                        Expr::gt(Expr::var("count"), Expr::u32(threshold)),
                        vec![Node::assign(
                            "acc",
                            Expr::bitor(
                                Expr::var("acc"),
                                Expr::shl(Expr::u32(1), Expr::var("bit")),
                            ),
                        )],
                    ),
                ],
            ),
            Node::store(out, t, Expr::var("acc")),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(stacked, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(stacked_words),
            BufferDecl::storage(out, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(dim_words),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(BUNDLE_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

// ---- CPU references ----

/// CPU reference for [`hypervector_xor_bind`].
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn xor_bind_cpu(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    match try_xor_bind_cpu_into(a, b, &mut out) {
        Ok(()) => out,
        // A parity oracle that returns empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use the try_ variant.
        Err(error) => panic!("vyre-primitives hypervector XOR bind CPU reference failed: {error}"),
    }
}

/// CPU reference for [`hypervector_xor_bind`] using a caller-owned buffer.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn xor_bind_cpu_into(a: &[u32], b: &[u32], out: &mut Vec<u32>) {
    if let Err(error) = try_xor_bind_cpu_into(a, b, out) {
        panic!("vyre-primitives hypervector XOR bind CPU reference failed: {error}");
    }
}

/// Fallible CPU reference for [`hypervector_xor_bind`] using a caller-owned buffer.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_xor_bind_cpu_into(a: &[u32], b: &[u32], out: &mut Vec<u32>) -> Result<(), String> {
    let dim_words = a.len().min(b.len());
    if dim_words > out.capacity() {
        out.try_reserve_exact(dim_words - out.capacity())
            .map_err(|err| {
                format!("hypervector XOR bind could not reserve {dim_words} output words: {err}")
            })?;
    }
    out.clear();
    out.extend(a.iter().zip(b.iter()).take(dim_words).map(|(&x, &y)| x ^ y));
    Ok(())
}

/// CPU reference for [`hypervector_majority_bundle`].
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn majority_bundle_cpu(hvs: &[Vec<u32>]) -> Vec<u32> {
    let mut out = Vec::new();
    match try_majority_bundle_cpu_into(hvs, &mut out) {
        Ok(()) => out,
        // A parity oracle that returns empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use the try_ variant.
        Err(error) => {
            panic!("vyre-primitives hypervector majority bundle CPU reference failed: {error}")
        }
    }
}

/// CPU reference for [`hypervector_majority_bundle`] using a caller-owned buffer.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn majority_bundle_cpu_into(hvs: &[Vec<u32>], out: &mut Vec<u32>) {
    if let Err(error) = try_majority_bundle_cpu_into(hvs, out) {
        panic!("vyre-primitives hypervector majority bundle CPU reference failed: {error}");
    }
}

/// Fallible CPU reference for [`hypervector_majority_bundle`] using a caller-owned buffer.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_majority_bundle_cpu_into(hvs: &[Vec<u32>], out: &mut Vec<u32>) -> Result<(), String> {
    let Some(dim_words) = hvs.iter().map(Vec::len).min() else {
        out.clear();
        return Ok(());
    };
    if dim_words == 0 {
        out.clear();
        return Ok(());
    }
    let k = hvs.len();
    let threshold = k / 2;

    if dim_words > out.capacity() {
        out.try_reserve_exact(dim_words - out.capacity())
            .map_err(|err| {
                format!(
                    "hypervector majority bundle could not reserve {dim_words} output words: {err}"
                )
            })?;
    }
    out.clear();
    out.resize(dim_words, 0);
    for w in 0..dim_words {
        for bit in 0..32 {
            let mut count = 0;
            for hv in hvs {
                count += (hv[w] >> bit) & 1;
            }
            if count as usize > threshold {
                out[w] |= 1 << bit;
            }
        }
    }
    Ok(())
}

/// Cosine-style similarity over BSC hypervectors: 1 - 2 · hamming(a, b) /
/// dim_bits. Returns f32 in roughly [-1, 1] (perfect match = 1.0, anti-
/// correlation = -1.0, random = 0.0).
#[must_use]
pub fn hamming_similarity(a: &[u32], b: &[u32]) -> f32 {
    let dim_words = a.len().min(b.len());
    if dim_words == 0 {
        return 1.0;
    }
    let dim_bits = (dim_words * 32) as f32;
    let hamming: u32 = a
        .iter()
        .zip(b.iter())
        .take(dim_words)
        .map(|(&x, &y)| (x ^ y).count_ones())
        .sum();
    1.0 - 2.0 * (hamming as f32) / dim_bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_bind_self_cancels() {
        // bind(bind(a, b), b) == a  -  XOR is self-inverse.
        let a = vec![0xDEAD_BEEFu32, 0x0BAD_F00D];
        let b = vec![0x1234_5678, 0x90AB_CDEF];
        let bound = xor_bind_cpu(&a, &b);
        let unbound = xor_bind_cpu(&bound, &b);
        assert_eq!(unbound, a);
    }

    #[test]
    fn xor_bind_zero_is_identity() {
        let a = vec![0x1234, 0x5678, 0xABCD];
        let zero = vec![0u32; a.len()];
        assert_eq!(xor_bind_cpu(&a, &zero), vec![0x1234, 0x5678, 0xABCD]);
    }

    #[test]
    fn xor_bind_cpu_into_reuses_output() {
        let a = vec![0x1234, 0x5678, 0xABCD];
        let b = vec![0xFFFF, 0x0000, 0x1111];
        let mut out = Vec::with_capacity(8);
        let ptr = out.as_ptr();
        xor_bind_cpu_into(&a, &b, &mut out);
        assert_eq!(out, vec![0xEDCB, 0x5678, 0xBADC]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn try_xor_bind_cpu_into_clears_stale_tail_without_reallocating() {
        let a = vec![0x1234, 0x5678, 0xABCD];
        let b = vec![0xFFFF];
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&[u32::MAX; 8]);
        let ptr = out.as_ptr();

        try_xor_bind_cpu_into(&a, &b, &mut out).unwrap();

        assert_eq!(out, vec![0xEDCB]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn xor_bind_wrappers_match_fallible_reference() {
        let a = vec![0x1234, 0x5678, 0xABCD];
        let b = vec![0xFFFF, 0, 0x1111];
        let mut compat = Vec::with_capacity(8);
        let mut fallible = Vec::with_capacity(8);

        xor_bind_cpu_into(&a, &b, &mut compat);
        try_xor_bind_cpu_into(&a, &b, &mut fallible)
            .expect("Fix: small hypervector XOR bind CPU reference must reserve");

        assert_eq!(xor_bind_cpu(&a, &b), fallible);
        assert_eq!(compat, fallible);
    }

    #[test]
    fn xor_bind_cpu_truncates_mismatched_inputs() {
        let a = vec![0x1234, 0x5678, 0xABCD];
        let b = vec![0xFFFF];
        assert_eq!(xor_bind_cpu(&a, &b), vec![0xEDCB]);
    }

    #[test]
    fn majority_bundle_three_vectors() {
        // Bit 0 set in 2/3 → output bit 0 = 1.
        // Bit 1 set in 1/3 → output bit 1 = 0.
        // Bit 2 set in 0/3 → output bit 2 = 0.
        let hvs = vec![vec![0b001], vec![0b001], vec![0b010]];
        let out = majority_bundle_cpu(&hvs);
        assert_eq!(out, vec![0b001]);
    }

    #[test]
    fn majority_bundle_unanimous() {
        let hvs = vec![vec![0xFF], vec![0xFF], vec![0xFF]];
        let out = majority_bundle_cpu(&hvs);
        assert_eq!(out, vec![0xFF]);
    }

    #[test]
    fn majority_bundle_cpu_into_reuses_output() {
        let hvs = vec![vec![0b001], vec![0b001], vec![0b010]];
        let mut out = Vec::with_capacity(8);
        let ptr = out.as_ptr();
        majority_bundle_cpu_into(&hvs, &mut out);
        assert_eq!(out, vec![0b001]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn try_majority_bundle_cpu_into_clears_stale_tail_without_reallocating() {
        let hvs = vec![vec![0b001], vec![0b001], vec![0b010]];
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&[u32::MAX; 8]);
        let ptr = out.as_ptr();

        try_majority_bundle_cpu_into(&hvs, &mut out).unwrap();

        assert_eq!(out, vec![0b001]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn majority_bundle_wrappers_match_fallible_reference() {
        let hvs = vec![vec![0b001], vec![0b001], vec![0b010]];
        let mut compat = Vec::with_capacity(8);
        let mut fallible = Vec::with_capacity(8);

        majority_bundle_cpu_into(&hvs, &mut compat);
        try_majority_bundle_cpu_into(&hvs, &mut fallible)
            .expect("Fix: small hypervector majority bundle CPU reference must reserve");

        assert_eq!(majority_bundle_cpu(&hvs), fallible);
        assert_eq!(compat, fallible);
    }

    #[test]
    fn majority_bundle_tie_rounds_to_zero() {
        // 2 vectors, bit 0 set in 1: count=1, threshold=k/2=1, count > threshold is false
        let hvs = vec![vec![0b1], vec![0b0]];
        let out = majority_bundle_cpu(&hvs);
        assert_eq!(out, vec![0b0]);
    }

    #[test]
    fn majority_bundle_cpu_handles_empty_and_mismatched_inputs() {
        let empty: Vec<Vec<u32>> = Vec::new();
        assert!(majority_bundle_cpu(&empty).is_empty());

        let hvs = vec![vec![0b001, 0b111], vec![0b001]];
        assert_eq!(majority_bundle_cpu(&hvs), vec![0b001]);
    }

    #[test]
    fn hamming_similarity_self_is_one() {
        let a = vec![0xDEAD_BEEFu32; 8];
        assert!((hamming_similarity(&a, &a) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hamming_similarity_complement_is_minus_one() {
        let a = vec![0xFFFF_FFFFu32; 4];
        let b = vec![0x0000_0000u32; 4];
        assert!((hamming_similarity(&a, &b) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn hamming_similarity_handles_empty_and_mismatched_inputs() {
        assert_eq!(hamming_similarity(&[], &[]), 1.0);
        let a = vec![0xFFFF_FFFFu32, 0];
        let b = vec![0];
        assert!((hamming_similarity(&a, &b) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn ir_program_xor_bind_buffer_layout() {
        let p = hypervector_xor_bind("a", "b", "out", 64);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["a", "b", "out"]);
        for buf in p.buffers.iter() {
            assert_eq!(buf.count(), 64);
        }
    }

    #[test]
    fn ir_program_xor_bind_zero_dim_is_trap() {
        let p = hypervector_xor_bind("a", "b", "out", 0);
        assert_eq!(p.buffers.len(), 1);
        assert_eq!(p.buffers[0].name(), "out");
    }

    #[test]
    fn ir_program_bundle_buffer_layout() {
        let p = hypervector_majority_bundle("stack", "out", 8, 5);
        assert_eq!(p.buffers[0].count(), 5 * 8);
        assert_eq!(p.buffers[1].count(), 8);
    }

    #[test]
    fn bundle_overflow_lowers_to_trap_not_host_panic() {
        let p = hypervector_majority_bundle("stack", "out", u32::MAX, 2);
        assert!(p.stats().trap());
        assert_eq!(p.buffers[0].name(), "out");
    }

    #[test]
    fn production_wrappers_have_no_raw_panic_path() {
        let production = include_str!("hypervector.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: hypervector.rs must contain production section");

        // No LAZY panics (no fix hint); an explicit panic!() fail-loud IS the
        // blessed Law-10 fix for an infallible parity wrapper.
        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: hypervector production wrappers must not use bare .unwrap()/.expect() — use an explicit panic!() with the error."
        );
        // No SILENT fallback: returning empty on failure masks a parity divergence (Law 10/6).
        assert!(
            !production.contains(concat!("eprintln", "!(\"vyre-primitives hypervector")),
            "Fix: hypervector CPU oracle must not log-and-return empty on error — fail loud via panic!() so callers use the try_ variant."
        );
        assert!(
            production.contains("panic!("),
            "Fix: hypervector CPU oracle must panic!() when it cannot compute the reference, never return an empty vec."
        );
    }

    #[test]
    fn standard_dim_constants() {
        assert_eq!(STANDARD_DIM_BITS, STANDARD_DIM_WORDS * 32);
        const _: () = assert!(STANDARD_DIM_BITS >= 8192);
    }
}
