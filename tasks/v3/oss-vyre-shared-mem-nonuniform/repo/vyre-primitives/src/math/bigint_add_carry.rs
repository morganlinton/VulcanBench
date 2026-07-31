//! `bigint_add_carry`  -  multi-limb big-integer addition with explicit
//! carry-out propagation, packed as one u32-limb per element.
//!
//! Op id: `vyre-primitives::math::bigint_add_carry`. Soundness: `Exact` over
//! `(a + b) mod 2^(32 * limb_count)` with the high carry-out emitted as a
//! separate scalar. The CPU reference at the bottom of this file is the
//! contract; the GPU `Program` matches it lane-for-lane.
//!
//! ## Why it matters
//!
//! Public-key crypto (RSA, ECDSA, post-quantum lattices), digital-signature
//! verification, and arbitrary-precision integer math all bottom out into
//! ripple-carry addition over 256-bit / 512-bit / 4096-bit operands. Doing
//! this on GPU naively serializes all carries through a single thread: each
//! limb depends on the carry from the limb below. This primitive ships the
//! foundation: a load-and-add wave that emits per-limb sums + per-limb
//! carry-out booleans. A carry-fix wave sweeps the per-limb carry stream
//! into a final answer.
//!
//! The output layout is the canonical "split-carry" form expected by every
//! known parallel bigint adder (Brent-Kung, Kogge-Stone, Sklansky). Once you
//! have `(sum_no_carry[i], carry[i])` you can finish in O(log n) prefix-scan
//! depth instead of O(n) ripple. This module emits the first half; the
//! prefix-scan finish is in `prefix_scan` (#5).
//!
//! ## Wire layout
//!
//! Inputs:
//!   - `a`  -  limb_count u32 limbs, little-endian (limb 0 = LSB).
//!   - `b`  -  limb_count u32 limbs, little-endian.
//!
//! Outputs:
//!   - `sum_partial`  -  limb_count u32 limbs: `(a[i] + b[i]) mod 2^32`.
//!   - `carry_partial`  -  limb_count u32 limbs (each is 0 or 1): the
//!     carry-out of `a[i] + b[i]`. Bit `i` of the final carry-resolved sum
//!     comes from `sum_partial[i] + carry_in[i]` where `carry_in[i]` is
//!     the prefix-or-style fold of `carry_partial[0..i]` adjusted for
//!     "carry-generate" from the partial sum overflowing.
//!
//! This module is the load-and-half-add primitive used by the parallel-prefix
//! carry resolver.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id for region-chain audits and bench attribution.
pub const OP_ID: &str = "vyre-primitives::math::bigint_add_carry";

/// Canonical binding indices.
pub const BINDING_A_IN: u32 = 0;
/// `b` operand binding.
pub const BINDING_B_IN: u32 = 1;
/// `sum_partial` output binding.
pub const BINDING_SUM_PARTIAL_OUT: u32 = 2;
/// `carry_partial` output binding (one u32 per limb, value is 0 or 1).
pub const BINDING_CARRY_PARTIAL_OUT: u32 = 3;

/// One lane per bigint limb in the split add-carry pass.
pub const BIGINT_ADD_CARRY_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid that covers every bigint limb lane.
#[must_use]
pub const fn bigint_add_carry_dispatch_grid(limb_count: u32) -> [u32; 3] {
    let lanes_per_block = BIGINT_ADD_CARRY_WORKGROUP_SIZE[0];
    let full_blocks = limb_count / lanes_per_block;
    let tail_block = if limb_count % lanes_per_block == 0 {
        0
    } else {
        1
    };
    let blocks = full_blocks + tail_block;
    [if blocks == 0 { 1 } else { blocks }, 1, 1]
}

/// Bigint CPU-reference error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BigIntAddCarryError {
    /// Input operands had different limb counts.
    LimbCountMismatch {
        /// `a` operand length.
        a_len: usize,
        /// `b` operand length.
        b_len: usize,
    },
    /// Split carry arrays had different limb counts.
    SplitCarryLengthMismatch {
        /// `sum_partial` length.
        sum_len: usize,
        /// `carry_partial` length.
        carry_len: usize,
    },
    /// Caller-owned storage could not be reserved.
    AllocationFailed {
        /// Operation that was reserving storage.
        operation: &'static str,
        /// Allocator or capacity diagnostic.
        message: String,
    },
}

/// Build the IR `Program` that emits `(sum_partial, carry_partial)` for
/// a multi-limb big-integer addition.
///
/// One thread per limb. Each thread:
///   1. Loads `a[gid]` and `b[gid]`.
///   2. Computes `sum = a + b` mod 2^32 and `carry = if sum < a { 1 } else { 0 }`
///      (canonical "carry from unsigned overflow" check).
///   3. Stores both into the output buffers at index `gid`.
///
/// `limb_count` must be > 0; the workgroup size is fixed at 256 lanes.
#[must_use]
pub fn bigint_add_carry(limb_count: u32) -> Program {
    if limb_count == 0 {
        return crate::invalid_output_program(
            OP_ID,
            "sum_partial",
            DataType::U32,
            "Fix: bigint_add_carry requires limb_count > 0, got 0.".to_string(),
        );
    }

    let body = vec![
        Node::let_bind("limb_idx", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(Expr::var("limb_idx"), Expr::u32(limb_count)),
            vec![
                Node::let_bind("a_limb", Expr::load("a", Expr::var("limb_idx"))),
                Node::let_bind("b_limb", Expr::load("b", Expr::var("limb_idx"))),
                // sum (mod 2^32). Hardware u32 add already wraps.
                Node::let_bind("sum", Expr::add(Expr::var("a_limb"), Expr::var("b_limb"))),
                // carry = (sum < a_limb) ? 1 : 0    -  the canonical
                // detect-unsigned-overflow check.
                Node::let_bind(
                    "carry_bool",
                    Expr::lt(Expr::var("sum"), Expr::var("a_limb")),
                ),
                Node::let_bind(
                    "carry",
                    Expr::select(Expr::var("carry_bool"), Expr::u32(1), Expr::u32(0)),
                ),
                Node::store("sum_partial", Expr::var("limb_idx"), Expr::var("sum")),
                Node::store("carry_partial", Expr::var("limb_idx"), Expr::var("carry")),
            ],
        ),
    ];

    let buffers = vec![
        BufferDecl::storage("a", BINDING_A_IN, BufferAccess::ReadOnly, DataType::U32)
            .with_count(limb_count),
        BufferDecl::storage("b", BINDING_B_IN, BufferAccess::ReadOnly, DataType::U32)
            .with_count(limb_count),
        BufferDecl::storage(
            "sum_partial",
            BINDING_SUM_PARTIAL_OUT,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(limb_count),
        BufferDecl::storage(
            "carry_partial",
            BINDING_CARRY_PARTIAL_OUT,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(limb_count),
    ];

    let entry = vec![Node::Region {
        generator: Ident::from(OP_ID),
        source_region: None,
        body: Arc::new(body),
    }];
    Program::wrapped(buffers, BIGINT_ADD_CARRY_WORKGROUP_SIZE, entry)
}

/// CPU reference. Returns `(sum_partial, carry_partial)` matching the
/// GPU `Program` lane-for-lane.
///
/// Each limb is added with the per-limb carry computed in isolation
/// (no carry chaining). The downstream prefix-scan resolves the chain.
///
/// # Errors
///
/// Returns [`BigIntAddCarryError::LimbCountMismatch`] when operands have
/// different limb counts.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn bigint_add_carry_cpu(
    a: &[u32],
    b: &[u32],
) -> Result<(Vec<u32>, Vec<u32>), BigIntAddCarryError> {
    let mut sum_partial = Vec::with_capacity(a.len());
    let mut carry_partial = Vec::with_capacity(a.len());
    bigint_add_carry_cpu_into(a, b, &mut sum_partial, &mut carry_partial)?;
    Ok((sum_partial, carry_partial))
}

/// CPU reference into caller-owned output buffers.
///
/// Clears `sum_partial` and `carry_partial`, then reuses their capacity.
///
/// # Errors
///
/// Returns [`BigIntAddCarryError::LimbCountMismatch`] when operands have
/// different limb counts.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn bigint_add_carry_cpu_into(
    a: &[u32],
    b: &[u32],
    sum_partial: &mut Vec<u32>,
    carry_partial: &mut Vec<u32>,
) -> Result<(), BigIntAddCarryError> {
    if a.len() != b.len() {
        return Err(BigIntAddCarryError::LimbCountMismatch {
            a_len: a.len(),
            b_len: b.len(),
        });
    }
    reserve_bigint_output(sum_partial, a.len(), "sum_partial")?;
    reserve_bigint_output(carry_partial, a.len(), "carry_partial")?;
    sum_partial.clear();
    carry_partial.clear();
    for (a_limb, b_limb) in a.iter().zip(b.iter()) {
        let (sum, overflow) = a_limb.overflowing_add(*b_limb);
        sum_partial.push(sum);
        carry_partial.push(u32::from(overflow));
    }
    Ok(())
}

/// Resolve carry chain in O(n) ripple form. Used by the CPU reference
/// to validate that the (sum_partial, carry_partial) split form composes
/// correctly into the final big-integer sum + final carry-out.
///
/// Returns `(final_sum, final_carry_out)`. `final_carry_out` is 0 or 1.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn resolve_carry_chain_cpu(
    sum_partial: &[u32],
    carry_partial: &[u32],
) -> Result<(Vec<u32>, u32), BigIntAddCarryError> {
    let mut final_sum = Vec::with_capacity(sum_partial.len());
    let final_carry = resolve_carry_chain_cpu_into(sum_partial, carry_partial, &mut final_sum)?;
    Ok((final_sum, final_carry))
}

/// Resolve carry chain into caller-owned output storage.
///
/// Clears `final_sum`, then reuses its capacity. Returns final carry-out.
///
/// # Errors
///
/// Returns [`BigIntAddCarryError::SplitCarryLengthMismatch`] when the split
/// carry buffers have different limb counts.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn resolve_carry_chain_cpu_into(
    sum_partial: &[u32],
    carry_partial: &[u32],
    final_sum: &mut Vec<u32>,
) -> Result<u32, BigIntAddCarryError> {
    if sum_partial.len() != carry_partial.len() {
        return Err(BigIntAddCarryError::SplitCarryLengthMismatch {
            sum_len: sum_partial.len(),
            carry_len: carry_partial.len(),
        });
    }
    reserve_bigint_output(final_sum, sum_partial.len(), "final_sum")?;
    final_sum.clear();
    let mut carry_in: u32 = 0;
    for (sum, carry) in sum_partial.iter().zip(carry_partial.iter()) {
        let (with_in, overflow_from_in) = sum.overflowing_add(carry_in);
        final_sum.push(with_in);
        // Total carry-out of this limb = original carry_partial OR
        // carry-from-adding-the-incoming-carry. They cannot both fire
        // at the same time unless sum was exactly 0xFFFF_FFFF (in which
        // case the original add did NOT overflow; only the +1 did).
        carry_in = *carry | u32::from(overflow_from_in);
    }
    Ok(carry_in)
}

#[cfg(any(test, feature = "cpu-parity"))]
fn reserve_bigint_output(
    out: &mut Vec<u32>,
    len: usize,
    operation: &'static str,
) -> Result<(), BigIntAddCarryError> {
    if len > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            len - out.len(),
            "bigint add-carry CPU oracle",
            operation,
        )
        .map_err(|message| BigIntAddCarryError::AllocationFailed { operation, message })?;
    }
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || bigint_add_carry(4),
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[1, u32::MAX, 5, u32::MAX]),
                crate::wire::pack_u32_slice(&[2, 1, u32::MAX, u32::MAX]),
                crate::wire::pack_u32_slice(&[0; 4]),
                crate::wire::pack_u32_slice(&[0; 4]),
            ]]
        }),
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[3, 0, 4, u32::MAX - 1]),
                crate::wire::pack_u32_slice(&[0, 1, 1, 1]),
            ]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_zero_plus_zero_returns_zero_with_no_carries() {
        let (sum, carry) =
            bigint_add_carry_cpu(&[0, 0, 0, 0], &[0, 0, 0, 0]).expect("Fix: matching limbs");
        assert_eq!(sum, vec![0, 0, 0, 0]);
        assert_eq!(carry, vec![0, 0, 0, 0]);
    }

    #[test]
    fn cpu_no_overflow_per_limb_keeps_carries_zero() {
        let a = [1u32, 2, 3, 4];
        let b = [10u32, 20, 30, 40];
        let (sum, carry) = bigint_add_carry_cpu(&a, &b).expect("Fix: matching limbs");
        assert_eq!(sum, vec![11, 22, 33, 44]);
        assert_eq!(carry, vec![0, 0, 0, 0]);
    }

    #[test]
    fn cpu_per_limb_overflow_emits_carry_bit() {
        // limb 0: 0xFFFF_FFFF + 1 = 0, carry 1.
        // limb 1: 0xFFFF_FFFF + 0 = 0xFFFF_FFFF, carry 0.
        let a = [0xFFFF_FFFFu32, 0xFFFF_FFFFu32];
        let b = [1u32, 0u32];
        let (sum, carry) = bigint_add_carry_cpu(&a, &b).expect("Fix: matching limbs");
        assert_eq!(sum, vec![0, 0xFFFF_FFFF]);
        assert_eq!(carry, vec![1, 0]);
    }

    #[test]
    fn cpu_max_plus_max_emits_per_limb_carry_and_truncated_sum() {
        let a = [0xFFFF_FFFFu32; 4];
        let b = [0xFFFF_FFFFu32; 4];
        let (sum, carry) = bigint_add_carry_cpu(&a, &b).expect("Fix: matching limbs");
        // 0xFFFF_FFFF + 0xFFFF_FFFF = 0x1_FFFF_FFFE (wraps to 0xFFFF_FFFE,
        // carry 1) for every limb.
        assert_eq!(sum, vec![0xFFFF_FFFEu32; 4]);
        assert_eq!(carry, vec![1u32; 4]);
    }

    #[test]
    fn resolve_carry_chain_propagates_single_carry_through_zeros() {
        // sum_partial = [0xFFFF_FFFF, 0, 0, 0], carry_partial = [1, 0, 0, 0].
        // After resolve: limb 0 stays 0xFFFF_FFFF; carry 1 propagates upward.
        let sum_partial = vec![0xFFFF_FFFFu32, 0, 0, 0];
        let carry_partial = vec![1u32, 0, 0, 0];
        let (final_sum, final_carry) = resolve_carry_chain_cpu(&sum_partial, &carry_partial)
            .expect("Fix: matching split limbs");
        // limb 0 has no carry-in → stays 0xFFFF_FFFF.
        assert_eq!(final_sum, vec![0xFFFF_FFFF, 1, 0, 0]);
        assert_eq!(
            final_carry, 0,
            "the carry from limb 0 propagates into limb 1, then dies"
        );
    }

    #[test]
    fn resolve_carry_chain_handles_chained_overflow() {
        // Adding 0x..FF + 0x..01 across all limbs ripples a carry the whole way.
        // a = [0xFFFF_FFFF, 0xFFFF_FFFF, 0xFFFF_FFFF, 0]
        // b = [0x0000_0001, 0x0000_0000, 0x0000_0000, 0]
        // Expected final sum = [0, 0, 0, 1], final carry-out = 0.
        let a = [0xFFFF_FFFFu32, 0xFFFF_FFFFu32, 0xFFFF_FFFFu32, 0];
        let b = [1u32, 0, 0, 0];
        let (sum_partial, carry_partial) =
            bigint_add_carry_cpu(&a, &b).expect("Fix: matching limbs");
        let (final_sum, final_carry) = resolve_carry_chain_cpu(&sum_partial, &carry_partial)
            .expect("Fix: matching split limbs");
        assert_eq!(final_sum, vec![0, 0, 0, 1]);
        assert_eq!(final_carry, 0);
    }

    #[test]
    fn resolve_carry_chain_emits_final_carry_out_at_top() {
        // Adding the max two-limb integer to itself  -  the final carry-out
        // must be 1 (the answer doesn't fit in 64 bits).
        let a = [0xFFFF_FFFFu32, 0xFFFF_FFFFu32];
        let b = [0xFFFF_FFFFu32, 0xFFFF_FFFFu32];
        let (sum_partial, carry_partial) =
            bigint_add_carry_cpu(&a, &b).expect("Fix: matching limbs");
        let (_final_sum, final_carry) = resolve_carry_chain_cpu(&sum_partial, &carry_partial)
            .expect("Fix: matching split limbs");
        assert_eq!(
            final_carry, 1,
            "max + max in 64 bits overflows into the 65th bit"
        );
    }

    #[test]
    fn resolve_carry_chain_handles_corner_carry_in_only() {
        // sum_partial = [0xFFFF_FFFF, 0xFFFF_FFFF], carry_partial = [1, 0].
        // limb 0 → 0xFFFF_FFFF (no carry-in), then carry from limb 0 = 1.
        // limb 1 → 0xFFFF_FFFF + 1 = 0, with overflow → next carry = 1.
        let sum_partial = vec![0xFFFF_FFFFu32, 0xFFFF_FFFFu32];
        let carry_partial = vec![1u32, 0];
        let (final_sum, final_carry) = resolve_carry_chain_cpu(&sum_partial, &carry_partial)
            .expect("Fix: matching split limbs");
        assert_eq!(final_sum, vec![0xFFFF_FFFF, 0]);
        assert_eq!(
            final_carry, 1,
            "carry propagated into limb 1 made it overflow"
        );
    }

    #[test]
    fn cpu_handles_8_limb_256_bit_operands() {
        // 256-bit RSA-shape operand. Verifies the primitive scales to the
        // sizes used by ECDSA / X25519.
        let a = [0x1234_5678u32; 8];
        let b = [0x8765_4321u32; 8];
        let (sum, carry) = bigint_add_carry_cpu(&a, &b).expect("Fix: matching limbs");
        // 0x1234_5678 + 0x8765_4321 = 0x9999_9999, no overflow.
        assert_eq!(sum, vec![0x9999_9999u32; 8]);
        assert_eq!(carry, vec![0u32; 8]);
    }

    #[test]
    fn cpu_handles_128_limb_4096_bit_operands() {
        // 4096-bit RSA modulus. Verifies the primitive scales to the
        // sizes used by RSA-4096.
        let a = vec![0x5555_5555u32; 128];
        let b = vec![0xAAAA_AAAAu32; 128];
        let (sum, carry) = bigint_add_carry_cpu(&a, &b).expect("Fix: matching limbs");
        // 0x5555_5555 + 0xAAAA_AAAA = 0xFFFF_FFFF, no overflow.
        assert_eq!(sum, vec![0xFFFF_FFFFu32; 128]);
        assert_eq!(carry, vec![0u32; 128]);
    }

    #[test]
    fn cpu_mismatched_limb_count_returns_error() {
        let a = vec![0u32; 4];
        let b = vec![0u32; 5];
        assert_eq!(
            bigint_add_carry_cpu(&a, &b),
            Err(BigIntAddCarryError::LimbCountMismatch { a_len: 4, b_len: 5 })
        );
    }

    #[test]
    fn cpu_into_reuses_output_capacity() {
        let a = [1u32, u32::MAX];
        let b = [2u32, 1];

        let mut sum = Vec::with_capacity(32);
        let mut carry = Vec::with_capacity(32);
        let sum_cap = sum.capacity();
        let carry_cap = carry.capacity();
        bigint_add_carry_cpu_into(&a, &b, &mut sum, &mut carry).expect("Fix: matching limbs");
        assert_eq!(sum, vec![3, 0]);
        assert_eq!(carry, vec![0, 1]);
        assert_eq!(sum.capacity(), sum_cap);
        assert_eq!(carry.capacity(), carry_cap);
    }

    #[test]
    fn cpu_into_truncates_stale_tail_without_reallocating() {
        let a = [1u32, u32::MAX];
        let b = [2u32, 1];
        let mut sum = Vec::with_capacity(8);
        let mut carry = Vec::with_capacity(8);
        sum.extend([99u32; 8]);
        carry.extend([99u32; 8]);
        let sum_ptr = sum.as_ptr();
        let carry_ptr = carry.as_ptr();

        bigint_add_carry_cpu_into(&a, &b, &mut sum, &mut carry).unwrap();

        assert_eq!(sum, vec![3, 0]);
        assert_eq!(carry, vec![0, 1]);
        assert_eq!(sum.as_ptr(), sum_ptr);
        assert_eq!(carry.as_ptr(), carry_ptr);
    }

    #[test]
    fn resolve_into_truncates_stale_tail_without_reallocating() {
        let mut out = Vec::with_capacity(8);
        out.extend([99u32; 8]);
        let ptr = out.as_ptr();

        let carry = resolve_carry_chain_cpu_into(&[u32::MAX, u32::MAX], &[1, 0], &mut out).unwrap();

        assert_eq!(out, vec![u32::MAX, 0]);
        assert_eq!(carry, 1);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn generated_split_and_resolve_matches_ripple_reference() {
        let mut state = 0xB16A_DDCA_u32;
        for case in 0..4096u32 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let len = match case {
                0 => 1,
                1 => 24,
                2 => 256,
                3 => 257,
                4 => 1025,
                _ => state % 4097 + 1,
            } as usize;
            let mut a = Vec::with_capacity(len);
            let mut b = Vec::with_capacity(len);
            for idx in 0..len {
                state = state.rotate_left(9) ^ (idx as u32).wrapping_mul(0x9E37_79B9);
                let left = match idx % 11 {
                    0 => u32::MAX,
                    1 => 0,
                    2 => 0x8000_0000,
                    _ => state,
                };
                let right = match idx % 13 {
                    0 => 1,
                    1 => u32::MAX,
                    2 => 0x8000_0000,
                    _ => state.rotate_right(7),
                };
                a.push(left);
                b.push(right);
            }
            let (sum_partial, carry_partial) = bigint_add_carry_cpu(&a, &b).unwrap();
            let (final_sum, final_carry) =
                resolve_carry_chain_cpu(&sum_partial, &carry_partial).unwrap();
            let mut expected = Vec::with_capacity(len);
            let mut carry = 0u64;
            for i in 0..len {
                let total = a[i] as u64 + b[i] as u64 + carry;
                expected.push(total as u32);
                carry = total >> 32;
            }

            assert_eq!(
                final_sum, expected,
                "generated bigint case {case} len={len}"
            );
            assert_eq!(
                final_carry, carry as u32,
                "generated bigint carry case {case} len={len}"
            );
        }
    }

    #[test]
    fn resolve_carry_chain_rejects_length_mismatch() {
        let mut out = Vec::new();
        assert_eq!(
            resolve_carry_chain_cpu_into(&[0, 1], &[0], &mut out),
            Err(BigIntAddCarryError::SplitCarryLengthMismatch {
                sum_len: 2,
                carry_len: 1,
            })
        );
    }

    #[test]
    fn build_program_returns_well_formed_program() {
        let program = bigint_add_carry(8);
        assert_eq!(
            program.buffers().len(),
            4,
            "a, b, sum_partial, carry_partial"
        );
        assert_eq!(program.workgroup_size(), BIGINT_ADD_CARRY_WORKGROUP_SIZE);
    }

    #[test]
    fn dispatch_grid_packs_limb_lanes_into_workgroups() {
        assert_eq!(bigint_add_carry_dispatch_grid(0), [1, 1, 1]);
        assert_eq!(bigint_add_carry_dispatch_grid(1), [1, 1, 1]);
        assert_eq!(bigint_add_carry_dispatch_grid(256), [1, 1, 1]);
        assert_eq!(bigint_add_carry_dispatch_grid(257), [2, 1, 1]);
        assert_eq!(bigint_add_carry_dispatch_grid(1025), [5, 1, 1]);
    }

    #[test]
    fn zero_limb_count_traps() {
        let program = bigint_add_carry(0);
        assert!(program.stats().trap());
    }

    #[test]
    fn build_program_is_deterministic_across_calls() {
        // Same input → same Program. This is the wire-content-hash
        // contract; if it ever fails, differential compilation breaks.
        let p1 = bigint_add_carry(16);
        let p2 = bigint_add_carry(16);
        assert_eq!(
            p1.buffers().len(),
            p2.buffers().len(),
            "two builds with identical inputs must produce identical buffer lists"
        );
        assert_eq!(p1.workgroup_size(), p2.workgroup_size());
    }

    #[test]
    fn op_id_is_canonical_and_stable() {
        // Op ids are wire-format-visible; changing them is a breaking change.
        assert_eq!(OP_ID, "vyre-primitives::math::bigint_add_carry");
    }

    #[test]
    fn binding_indices_are_canonical_and_stable() {
        // Bindings are wire-format-visible; changing them is a breaking change.
        assert_eq!(BINDING_A_IN, 0);
        assert_eq!(BINDING_B_IN, 1);
        assert_eq!(BINDING_SUM_PARTIAL_OUT, 2);
        assert_eq!(BINDING_CARRY_PARTIAL_OUT, 3);
    }
}
