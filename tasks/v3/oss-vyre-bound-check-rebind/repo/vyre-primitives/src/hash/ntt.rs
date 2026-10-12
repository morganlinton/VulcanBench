//! Number-Theoretic Transform  -  FFT in a finite field.
//!
//! NTT replaces FFT's complex roots of unity with primitive roots of
//! a finite field GF(p). With prime `p ≡ 1 (mod 2N)`, an N-th root
//! exists in GF(p) and the radix-2 Cooley-Tukey FFT structure carries
//! over verbatim  -  but with EXACT integer arithmetic. No
//! floating-point error.
//!
//! Substrate for:
//! - **FHE schemes** (BFV, BGV, CKKS)  -  polynomial multiplication
//!   modulo `x^N + 1`,
//! - **zk-SNARKs** (PLONK, Plonky2, STARK)  -  polynomial commitment
//!   schemes,
//! - **Reed-Solomon codes**  -  error correction over GF(p),
//! - **Lattice-based crypto**  -  Kyber, Dilithium core operations.
//!
//! As FHE & zk become production primitives (Apple PCC, Worldcoin,
//! attested-compute markets), whoever ships the GPU NTT primitive
//! controls the substrate. Today: nobody has it as a Tier-2.5
//! reusable primitive. Vyre will.
//!
//! # Why this primitive is dual-use
//!
//! | Composition role | Use |
//! |---|---|
//! | homomorphic encryption | BFV / CKKS polynomial multiply |
//! | zero-knowledge proving | PLONK polynomial commitment |
//! | lattice cryptography | Kyber NTT-friendly multiply |
//! | stable polynomial math | exact-integer polynomial multiply without FFT precision loss |
//!
//! The primitive is domain-neutral: higher-level cryptographic or numeric
//! compositions supply scheme policy while this module owns finite-field
//! transform mechanics.
//!
//! # Choice of prime
//!
//! Default to **Solinas prime `p = 0xFFFF_FFFF_0000_0001` = 2^64 - 2^32 + 1**
//! a.k.a. **Goldilocks**  -  chosen by Plonky2. Properties:
//! - 2^64 - 2^32 + 1, 64-bit wide,
//! - admits primitive root of order 2^32 → up to N=2^32 NTT lengths,
//! - reductions exploit `2^96 ≡ -1 (mod p)` for fast Barrett-free
//!   modular multiply.
//!
//! For 32-bit-only buffer constraints we use **NTT-friendly primes**
//! with `p < 2^31` so a single u32 holds residues; e.g.
//! `p = 998244353 = 119 · 2^23 + 1`, primitive root `g = 3`.
//!
//! This module's public contract is the **u32 32-bit prime** path. A
//! Goldilocks-field NTT is a separate op because it requires a native
//! 64-bit arithmetic schema rather than this module's u32 buffer ABI.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id for one Cooley-Tukey butterfly stage.
pub const OP_ID: &str = "vyre-primitives::hash::ntt_butterfly_stage";

/// 32-bit NTT-friendly prime: `998244353 = 119 · 2^23 + 1`.
/// Admits primitive roots of order up to `2^23`.
pub const PRIME_P: u32 = 998_244_353;

/// Primitive root of the multiplicative group of `Z/PRIME_P`. `3` is
/// generator of order `(p - 1) = 2^23 · 119`.
pub const GENERATOR_G: u32 = 3;

/// Maximum NTT length (`2^23`).
pub const MAX_LEN: u32 = 1 << 23;

const MONTGOMERY_R2: u32 = 932_051_910;
const MONTGOMERY_N_PRIME: u32 = 998_244_351;

/// Modular addition in `Z/p`.
#[inline]
#[must_use]
pub fn mod_add(a: u32, b: u32) -> u32 {
    let s = (a as u64) + (b as u64);
    (if s >= PRIME_P as u64 {
        s - PRIME_P as u64
    } else {
        s
    }) as u32
}

/// Modular subtraction in `Z/p`.
#[inline]
#[must_use]
pub fn mod_sub(a: u32, b: u32) -> u32 {
    if a >= b {
        a - b
    } else {
        PRIME_P - (b - a)
    }
}

/// Modular multiplication in `Z/p` via 64-bit wide intermediate.
#[inline]
#[must_use]
pub fn mod_mul(a: u32, b: u32) -> u32 {
    ((a as u64 * b as u64) % PRIME_P as u64) as u32
}

/// Modular exponentiation `base^exp mod p`.
#[must_use]
pub fn mod_pow(mut base: u32, mut exp: u32) -> u32 {
    let mut result: u32 = 1;
    base %= PRIME_P;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mod_mul(result, base);
        }
        exp >>= 1;
        base = mod_mul(base, base);
    }
    result
}

fn mod_add_expr(left: Expr, right: Expr) -> Expr {
    let sum = Expr::add(left, right);
    Expr::select(
        Expr::ge(sum.clone(), Expr::u32(PRIME_P)),
        Expr::sub(sum.clone(), Expr::u32(PRIME_P)),
        sum,
    )
}

fn mod_sub_expr(left: Expr, right: Expr) -> Expr {
    Expr::select(
        Expr::ge(left.clone(), right.clone()),
        Expr::sub(left.clone(), right.clone()),
        Expr::sub(Expr::add(left, Expr::u32(PRIME_P)), right),
    )
}

fn montgomery_reduce_product_expr(left: Expr, right: Expr) -> Expr {
    let lo = Expr::mul(left.clone(), right.clone());
    let hi = Expr::mulhi(left, right);
    let m = Expr::mul(lo.clone(), Expr::u32(MONTGOMERY_N_PRIME));
    let mp_lo = Expr::mul(m.clone(), Expr::u32(PRIME_P));
    let mp_hi = Expr::mulhi(m, Expr::u32(PRIME_P));
    let low_sum = Expr::add(lo.clone(), mp_lo);
    let carry = Expr::select(Expr::lt(low_sum, lo), Expr::u32(1), Expr::u32(0));
    let reduced = Expr::add(Expr::add(hi, mp_hi), carry);
    Expr::select(
        Expr::ge(reduced.clone(), Expr::u32(PRIME_P)),
        Expr::sub(reduced.clone(), Expr::u32(PRIME_P)),
        reduced,
    )
}

fn mod_mul_expr(left: Expr, right: Expr) -> Expr {
    let left_mont = montgomery_reduce_product_expr(left, Expr::u32(MONTGOMERY_R2));
    let right_mont = montgomery_reduce_product_expr(right, Expr::u32(MONTGOMERY_R2));
    let product_mont = montgomery_reduce_product_expr(left_mont, right_mont);
    montgomery_reduce_product_expr(product_mont, Expr::u32(1))
}

/// CPU reference: in-place forward NTT of length `n` (power of two,
/// `n ≤ MAX_LEN`). Iterative Cooley-Tukey (decimation-in-time) with
/// bit-reversal permutation up front.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn ntt_forward_cpu(a: &mut [u32]) {
    let n = a.len() as u32;
    if !n.is_power_of_two() || n > MAX_LEN {
        a.fill(0);
        return;
    }

    // Bit-reversal permutation
    bit_reverse(a);

    let mut len = 2u32;
    while len <= n {
        // Primitive `len`-th root of unity in Z/p
        let w_n = mod_pow(GENERATOR_G, (PRIME_P - 1) / len);
        let half = len / 2;
        let mut i = 0;
        while i < n as usize {
            let mut w: u32 = 1;
            for j in 0..half as usize {
                let u = a[i + j];
                let v = mod_mul(a[i + j + half as usize], w);
                a[i + j] = mod_add(u, v);
                a[i + j + half as usize] = mod_sub(u, v);
                w = mod_mul(w, w_n);
            }
            i += len as usize;
        }
        len <<= 1;
    }
}

/// CPU reference: in-place inverse NTT.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn ntt_inverse_cpu(a: &mut [u32]) {
    let n = a.len() as u32;
    if !n.is_power_of_two() || n > MAX_LEN {
        a.fill(0);
        return;
    }

    bit_reverse(a);

    let mut len = 2u32;
    while len <= n {
        // Inverse primitive root
        let w_n_inv = mod_pow(mod_pow(GENERATOR_G, (PRIME_P - 1) / len), PRIME_P - 2);
        let half = len / 2;
        let mut i = 0;
        while i < n as usize {
            let mut w: u32 = 1;
            for j in 0..half as usize {
                let u = a[i + j];
                let v = mod_mul(a[i + j + half as usize], w);
                a[i + j] = mod_add(u, v);
                a[i + j + half as usize] = mod_sub(u, v);
                w = mod_mul(w, w_n_inv);
            }
            i += len as usize;
        }
        len <<= 1;
    }

    // Final scale by 1/n
    let n_inv = mod_pow(n, PRIME_P - 2);
    for x in a.iter_mut() {
        *x = mod_mul(*x, n_inv);
    }
}

/// Bit-reversal permutation of `a` (in place). Helper for both
/// forward and inverse NTT.
pub fn bit_reverse<T: Copy>(a: &mut [T]) {
    let n = a.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            a.swap(i, j);
        }
    }
}

/// Emit one Cooley-Tukey butterfly stage as a Program. Multi-stage
/// NTT dispatches log₂(n) of these in sequence.
///
/// Inputs:
/// - `data`: length-`n` u32 buffer (residues in `[0, p)`).
/// - `twiddles`: length-`n/2` u32 buffer of stage-`stage_log` twiddle
///   factors `w_n^k` for `k ∈ 0..n/2`. Caller pre-computes (host).
///
/// One butterfly per pair of lanes:
///   `(a, b) → (a + w · b mod p, a - w · b mod p)`
///
/// `stage_log` is the log₂ of the current butterfly distance  -  used by
/// the lane to index the correct twiddle.
#[must_use]
pub fn ntt_butterfly_stage(data: &str, twiddles: &str, n: u32, stage_log: u32) -> Program {
    if !n.is_power_of_two() {
        return crate::invalid_output_program(
            OP_ID,
            data,
            DataType::U32,
            format!("Fix: ntt_butterfly_stage requires power-of-two n, got {n}."),
        );
    }
    if n > MAX_LEN {
        return crate::invalid_output_program(
            OP_ID,
            data,
            DataType::U32,
            format!("Fix: ntt_butterfly_stage requires n <= MAX_LEN={MAX_LEN}, got {n}."),
        );
    }
    if stage_log >= 32 {
        return crate::invalid_output_program(
            OP_ID,
            data,
            DataType::U32,
            format!("Fix: ntt_butterfly_stage requires stage_log < 32, got {stage_log}."),
        );
    }

    let half = n / 2;
    let butterfly_distance = 1u32 << stage_log;
    if butterfly_distance == 0 || butterfly_distance > half {
        return crate::invalid_output_program(
            OP_ID,
            data,
            DataType::U32,
            format!(
                "Fix: ntt_butterfly_stage stage_log={stage_log} exceeds n={n} butterfly range."
            ),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    // For lane t in [0, half): compute pair (i, i + butterfly_distance)
    // with i = (t / butterfly_distance) * 2 * butterfly_distance + (t % butterfly_distance)
    // This places butterflies at distance butterfly_distance apart.
    let pair_lo = Expr::add(
        Expr::mul(
            Expr::div(t.clone(), Expr::u32(butterfly_distance)),
            Expr::u32(2 * butterfly_distance),
        ),
        Expr::rem(t.clone(), Expr::u32(butterfly_distance)),
    );
    let pair_hi = Expr::add(pair_lo.clone(), Expr::u32(butterfly_distance));

    // Twiddle index: t % butterfly_distance (the position within the
    // half-pair indexes the stage-local twiddle).
    let twiddle_idx = Expr::rem(t.clone(), Expr::u32(butterfly_distance));

    // u = a[lo], v = (a[hi] * w) mod p, write a[lo] = (u + v) mod p,
    // a[hi] = (u - v) mod p. Multiplication uses Montgomery reduction
    // built from u32 low/high products, so the GPU IR stays byte-identical
    // to the CPU reference without requiring native u64 arithmetic.
    //
    // For the first stage, butterfly_distance == 1 and the only valid
    // stage-local twiddle is w^0 == 1. Specialize that case to `v = hi`
    // instead of emitting the full Montgomery product. This removes a huge
    // expression tree from catalog-scale proof fixtures and avoids WGPU
    // spending release-gate time compiling arithmetic that is provably dead
    // for the stage-0 butterfly.
    let v_expr = if butterfly_distance == 1 {
        Expr::var("hi")
    } else {
        mod_mul_expr(Expr::var("hi"), Expr::var("w"))
    };
    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(half)),
        vec![
            Node::let_bind("u", Expr::load(data, pair_lo.clone())),
            Node::let_bind("hi", Expr::load(data, pair_hi.clone())),
            Node::let_bind("w", Expr::load(twiddles, twiddle_idx)),
            Node::let_bind("v", v_expr),
            Node::store(data, pair_lo, mod_add_expr(Expr::var("u"), Expr::var("v"))),
            Node::store(data, pair_hi, mod_sub_expr(Expr::var("u"), Expr::var("v"))),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(data, 0, BufferAccess::ReadWrite, DataType::U32).with_count(n),
            BufferDecl::storage(twiddles, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(half),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || ntt_butterfly_stage("data", "twiddles", 4, 0),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[1, 2, 3, 4]),
                to_bytes(&[1, mod_pow(GENERATOR_G, (PRIME_P - 1) / 4)]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[3, PRIME_P - 1, 7, PRIME_P - 1])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_ops_roundtrip() {
        // Add then sub recovers identity.
        let a = 12345u32;
        let b = 6789u32;
        assert_eq!(mod_sub(mod_add(a, b), b), a);
        // Mul then by inverse via Fermat's little theorem
        let c = 100u32;
        let c_inv = mod_pow(c, PRIME_P - 2);
        assert_eq!(mod_mul(c, c_inv), 1);
    }

    #[test]
    fn mod_add_wraps_correctly() {
        let near_p = PRIME_P - 1;
        // (p-1) + (p-1) = 2p - 2 → mod p = p - 2
        assert_eq!(mod_add(near_p, near_p), PRIME_P - 2);
    }

    #[test]
    fn mod_pow_zero_is_one() {
        assert_eq!(mod_pow(7, 0), 1);
    }

    #[test]
    fn mod_pow_one_returns_base() {
        assert_eq!(mod_pow(7, 1), 7);
    }

    #[test]
    fn primitive_root_has_correct_order() {
        // GENERATOR_G^(p-1) ≡ 1 (mod p) by Fermat.
        assert_eq!(mod_pow(GENERATOR_G, PRIME_P - 1), 1);
    }

    #[test]
    fn ntt_forward_then_inverse_recovers_input() {
        let mut a: Vec<u32> = (0..8).map(|i| (i * 7 + 3) % PRIME_P).collect();
        let original = a.clone();
        ntt_forward_cpu(&mut a);
        ntt_inverse_cpu(&mut a);
        assert_eq!(a, original);
    }

    #[test]
    fn ntt_forward_then_inverse_size_16() {
        let mut a: Vec<u32> = (0..16).map(|i| (i * 31 + 5) % PRIME_P).collect();
        let original = a.clone();
        ntt_forward_cpu(&mut a);
        ntt_inverse_cpu(&mut a);
        assert_eq!(a, original);
    }

    #[test]
    fn ntt_implements_polynomial_multiplication() {
        // (1 + 2x) * (3 + 4x) = 3 + 10x + 8x²
        // Encode in length-4 buffer (next power of 2 ≥ deg+1).
        let mut a = vec![1u32, 2, 0, 0];
        let mut b = vec![3u32, 4, 0, 0];
        ntt_forward_cpu(&mut a);
        ntt_forward_cpu(&mut b);
        let c: Vec<u32> = a
            .iter()
            .zip(b.iter())
            .map(|(&x, &y)| mod_mul(x, y))
            .collect();
        let mut c_mut = c;
        ntt_inverse_cpu(&mut c_mut);
        assert_eq!(c_mut[0], 3);
        assert_eq!(c_mut[1], 10);
        assert_eq!(c_mut[2], 8);
        assert_eq!(c_mut[3], 0);
    }

    #[test]
    fn bit_reverse_is_self_inverse() {
        let mut a: Vec<u32> = (0..16).collect();
        let original = a.clone();
        bit_reverse(&mut a);
        bit_reverse(&mut a);
        assert_eq!(a, original);
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = ntt_butterfly_stage("data", "tw", 16, 0);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["data", "tw"]);
        assert_eq!(p.buffers[0].count(), 16);
        assert_eq!(p.buffers[1].count(), 8);
    }

    #[test]
    fn stage_zero_butterfly_avoids_dead_montgomery_multiply_tree() {
        let p = ntt_butterfly_stage("data", "tw", 4, 0);
        let rendered = format!("{:?}", p.entry());
        assert!(
            !rendered.contains("MulHi"),
            "Fix: stage-0 NTT twiddle is 1, so release proof fixtures must not emit the giant Montgomery multiply expression tree that stalls WGPU compilation: {rendered}"
        );
    }

    #[test]
    fn ir_butterfly_stage_matches_exact_modular_reference() {
        use vyre_reference::value::Value;

        let n = 4;
        let root = mod_pow(GENERATOR_G, (PRIME_P - 1) / n);
        let input = [PRIME_P - 1, 2, 3, 4];
        let twiddles = [1, root];
        let program = ntt_butterfly_stage("data", "tw", n, 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(crate::wire::pack_u32_slice(&input)),
                Value::from(crate::wire::pack_u32_slice(&twiddles)),
            ],
        )
        .expect("Fix: NTT butterfly stage must execute in the reference interpreter.");
        let got = outputs[0]
            .to_bytes()
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();

        let v0 = mod_mul(input[2], twiddles[0]);
        let v1 = mod_mul(input[3], twiddles[1]);
        let expected = vec![
            mod_add(input[0], v0),
            mod_add(input[1], v1),
            mod_sub(input[0], v0),
            mod_sub(input[1], v1),
        ];
        assert_eq!(
            got, expected,
            "Fix: GPU IR must perform the same modular butterfly as the CPU reference."
        );
    }

    #[test]
    fn non_power_of_two_traps() {
        let p = ntt_butterfly_stage("d", "t", 7, 0);
        assert!(p.stats().trap());
    }
}
