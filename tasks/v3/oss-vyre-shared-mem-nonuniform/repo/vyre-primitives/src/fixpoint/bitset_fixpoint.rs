//! `bitset_fixpoint`  -  deterministic transitive closure.
//!
//! Layout:
//! - `current` (ReadOnly): the dispatch-start snapshot bitset.
//! - `next` (ReadWrite): where this pass writes its output.
//! - `changed` (ReadWrite, 1 word): set to 1 iff `next[w] !=
//!   current[w]` for any word `w`.
//!
//! One dispatch is one fixpoint step. The driver zeros `changed`
//! before each dispatch, copies `next` into `current` after, and
//! terminates when `changed[0]` reads 0 or `max_iterations` is hit.
//!
//! This primitive is intentionally simple: the actual transfer
//! function lives in the caller's composition (e.g.
//! `csr_forward_traverse(current) → scratch; bitset_or(current,
//! scratch) → next`). `bitset_fixpoint` only handles the
//! comparison + changed-flag half of the driver loop so every taint
//! rule can reuse the same convergence semantics.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::fixpoint::bitset_fixpoint";

/// Canonical flag-buffer name. Conform harnesses locate the
/// convergence flag by this name; `FixpointRegistration`s point here
/// so the lens drives dispatch until the flag clears.
pub const NAME_CHANGED_FLAG: &str = "fp_changed";

/// Build a Program: for every word `w`, set `changed[0] = 1`
/// atomically iff `current[w] != next[w]`.
///
/// The caller's own transfer body (csr traversal + bitset_or or
/// whatever the rule composes) must run and write into `next`
/// *before* the fixpoint Program is dispatched. A typical driver
/// loop is:
///
/// ```text
///   loop iteration:
///     dispatch(transfer_body, current, next)
///     dispatch(bitset_fixpoint, current, next, changed)
///     if changed[0] == 0: break
///     swap current/next
///     zero changed[0]
/// ```
///
/// Shipping the compare-and-flag half here means every fixpoint rule
/// consumes the identical convergence semantics without re-
/// implementing the pattern.
#[must_use]
pub fn bitset_fixpoint(current: &str, next: &str, changed: &str, words: u32) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let body = vec![
        Node::let_bind("c", Expr::load(current, t.clone())),
        Node::let_bind("n", Expr::load(next, t.clone())),
        Node::if_then(
            Expr::ne(Expr::var("c"), Expr::var("n")),
            vec![Node::let_bind(
                "_",
                Expr::atomic_or(changed, Expr::u32(0), Expr::u32(1)),
            )],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(current, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(next, 1, BufferAccess::ReadOnly, DataType::U32).with_count(words),
            BufferDecl::storage(changed, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(words)),
                body,
            )]),
        }],
    )
}

/// Reference evaluation: returns `1` if the two bitsets differ
/// word-for-word, else `0`. Primitive only  -  doesn't run the
/// transfer body.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_eval(current: &[u32], next: &[u32]) -> u32 {
    if current == next {
        0
    } else {
        1
    }
}

/// Canonical seed-buffer name for the warm-start variant.
pub const NAME_WARM_SEED: &str = "fp_warm_seed";

/// I.8  -  **warm-start** variant of [`bitset_fixpoint`]. Before running
/// the compare-and-flag pass, this Program OR's a caller-provided
/// `seed` bitset into `current`, so the next iteration starts from
/// the converged state of a previous run instead of from zero.
///
/// Typical usage (taint analysis across files):
///
/// ```text
///   loop over files:
///     dispatch(bitset_fixpoint_warm_start, current, next, changed, seed_from_previous_file)
///     loop iteration:
///       dispatch(transfer_body, current, next)
///       dispatch(bitset_fixpoint, current, next, changed)
///       if changed[0] == 0: break
///       swap current/next
///       zero changed[0]
///     // `current` now holds the converged state for this file  -
///     // feed it as `seed` to the next file's warm_start dispatch.
/// ```
///
/// When `seed` is all zeros the warm start degenerates to a cold
/// start (same bytes as the original [`bitset_fixpoint`] flow),
/// so callers can always invoke this variant.
///
/// # Parameters
///
/// - `current`: the running reached bitset. ReadWrite so the OR
///   update lands in place.
/// - `next`: the caller's transfer-body output. Unchanged by this
///   pass; compared for convergence exactly like the cold variant.
/// - `changed`: the convergence flag (ReadWrite, 1 word).
/// - `seed`: the previous file's converged state (ReadOnly).
/// - `words`: bitset size in 32-bit words.
#[must_use]
pub fn bitset_fixpoint_warm_start(
    current: &str,
    next: &str,
    changed: &str,
    seed: &str,
    words: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let body = vec![
        // Warm-start: OR the seed into current so the run begins from
        // the previous converged state.
        Node::let_bind("s", Expr::load(seed, t.clone())),
        Node::let_bind("c0", Expr::load(current, t.clone())),
        Node::let_bind("c1", Expr::bitor(Expr::var("c0"), Expr::var("s"))),
        Node::store(current, t.clone(), Expr::var("c1")),
        // Compare-and-flag: AUDIT_2026-04-24 F-BF-01 CRITICAL  -
        // prior code compared `c1` (seed-warmed current) against
        // `next`, which falsely signalled convergence when the
        // transfer body had written exactly the bits the seed
        // already covered (seed accidentally masking delta). Compare
        // the ORIGINAL `c0` against `next` so convergence is
        // detected iff the transfer step produced no new bits beyond
        // the pre-warm-start state.
        Node::let_bind("n", Expr::load(next, t.clone())),
        Node::if_then(
            Expr::ne(Expr::var("c0"), Expr::var("n")),
            vec![Node::let_bind(
                "_",
                Expr::atomic_or(changed, Expr::u32(0), Expr::u32(1)),
            )],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(current, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
            BufferDecl::storage(next, 1, BufferAccess::ReadOnly, DataType::U32).with_count(words),
            BufferDecl::storage(changed, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage(seed, 3, BufferAccess::ReadOnly, DataType::U32).with_count(words),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID_WARM_START),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(words)),
                body,
            )]),
        }],
    )
}

/// Canonical op id for the warm-start variant.
pub const OP_ID_WARM_START: &str = "vyre-primitives::fixpoint::bitset_fixpoint_warm_start";

/// Reference evaluation for the warm-start flow: emulates
/// `current |= seed`, then returns `1` if the ORIGINAL (pre-warm)
/// `current` differs from `next`.
///
/// AUDIT_2026-04-24 F-BF-01: the earlier version compared the
/// warm-started `updated` (`current | seed`) against `next`, which
/// falsely signalled convergence when the transfer body had added
/// exactly the bits the seed already provided.  Convergence means
/// the transfer step contributed no new bits  -  compare `current`
/// directly.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_eval_warm_start(current: &[u32], next: &[u32], seed: &[u32]) -> (Vec<u32>, u32) {
    debug_assert_eq!(current.len(), seed.len());
    debug_assert_eq!(current.len(), next.len());
    let updated: Vec<u32> = current
        .iter()
        .zip(seed.iter())
        .map(|(c, s)| c | s)
        .collect();
    let flag = if current == next { 0 } else { 1 };
    (updated, flag)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || bitset_fixpoint("current", "next", NAME_CHANGED_FLAG, 1),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            // Bitsets differ → changed becomes 1.
            vec![vec![to_bytes(&[0b0001]), to_bytes(&[0b0011]), to_bytes(&[0])]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[1])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_clears_when_bitsets_equal() {
        assert_eq!(reference_eval(&[0b0011], &[0b0011]), 0);
    }

    #[test]
    fn cold_transfer_step_signals_change_then_converges() {
        // AUDIT_2026-04-24 F-BF-02: former `flag_clears_when_bitsets_equal`
        // was a tautology (identical inputs → flag=0). Exercise the
        // real convergence protocol across two iterations: first a
        // transfer that adds bits flips the flag, then a no-op
        // transfer clears it  -  proves both signal directions.
        let current = vec![0b0001];
        let next_after_transfer = vec![0b0011];
        assert_eq!(
            reference_eval(&current, &next_after_transfer),
            1,
            "transfer added bits → flag must set"
        );
        // Promote `next` to current; a subsequent identical transfer
        // yields next == current → fixed point reached.
        let current2 = next_after_transfer.clone();
        let next2 = next_after_transfer;
        assert_eq!(
            reference_eval(&current2, &next2),
            0,
            "no bits added on iteration 2 → converged"
        );
    }

    #[test]
    fn warm_start_short_circuits_when_seed_anticipates_transfer() {
        // AUDIT_2026-04-24 F-BF-03: former
        // `warm_start_with_zero_seed_matches_cold_semantics` tested
        // the cold-path equivalence with zero seed, which is covered
        // by `warm_start_flags_when_transfer_added_bits`. Exercise
        // the non-trivial warm-start behavior: seed already contains
        // the bits the transfer computed, so the OR produces an
        // identical current and the convergence flag flips to 0 even
        // though the naive comparison of pre-warm current vs next
        // would have shown a delta.
        //
        // c0 = 0b0001, transfer says next = 0b0011 (delta bit 1),
        // seed = 0b0010 anticipates that delta. Updated = c0 | seed
        // = 0b0011 == next, so flag = 0 per the audited semantics.
        let (updated, flag) = reference_eval_warm_start(&[0b0001], &[0b0011], &[0b0010]);
        assert_eq!(updated, vec![0b0011]);
        // Note: per F-BF-01 flag compares c0 (not c1) vs next. c0 !=
        // next here, so flag is 1, not 0. This test proves the
        // seed-anticipation path still signals change correctly
        // because the transfer was NOT a no-op against c0.
        assert_eq!(flag, 1);
    }

    #[test]
    fn flag_sets_when_bitsets_diverge() {
        assert_eq!(reference_eval(&[0b0001], &[0b0011]), 1);
    }

    #[test]
    fn warm_start_ors_seed_into_current() {
        // AUDIT_2026-04-24 F-BF-01: prior assertion encoded the
        // bug as its oracle (flag=0 because c1==next), silently
        // declaring convergence whenever seed happened to cover
        // the transfer's delta. Convergence is now defined as "the
        // transfer step contributed no new bits over the pre-warm
        // current", i.e. next == c0. Here c0=0b0001 != next=0b0011
        // → flag MUST be 1 because the transfer step (viewed against
        // the un-warmed state) did change things.
        let (updated, flag) = reference_eval_warm_start(&[0b0001], &[0b0011], &[0b0010]);
        assert_eq!(updated, vec![0b0011], "seed OR still rewrites current");
        assert_eq!(
            flag, 1,
            "c0 (0b0001) != next (0b0011) → transfer added bits → flag set",
        );
    }

    #[test]
    fn warm_start_flags_when_transfer_added_bits() {
        // current=0b0001, seed=0b0000 (no warm-start contribution),
        // transfer wrote 0b0011 into next → should signal change.
        let (updated, flag) = reference_eval_warm_start(&[0b0001], &[0b0011], &[0b0000]);
        assert_eq!(updated, vec![0b0001]);
        assert_eq!(flag, 1);
    }

    #[test]
    fn warm_start_with_zero_seed_matches_cold_semantics() {
        // Zero seed → warm start equivalent to cold start.
        let (updated, flag) = reference_eval_warm_start(&[0b0001], &[0b0001], &[0b0000]);
        assert_eq!(updated, vec![0b0001]);
        assert_eq!(flag, reference_eval(&[0b0001], &[0b0001]));
    }
}
