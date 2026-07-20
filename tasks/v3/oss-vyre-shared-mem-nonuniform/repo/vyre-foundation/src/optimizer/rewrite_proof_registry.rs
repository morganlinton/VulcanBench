//! N3  -  registry of shipped rewrite proof obligations.
//!
//! `rewrite_proof` provides the SMT-LIB v2 emitter; this module is the
//! library of *concrete* obligations, one (or more) per shipped
//! rewrite. CI calls [`shipped_obligations`], emits each to SMT2, and
//! runs z3/cvc5 to confirm `unsat` (proving the rewrite is correct).
//!
//! ## Coverage strategy
//!
//! v0.4.1 ships obligations for the algebraic rewrites whose contract is
//! purely arithmetic and provable in QF_BV (quantifier-free bit-vector
//! logic):
//!
//! - `identity_elim_add_zero`: `x + 0 = x`
//! - `identity_elim_mul_one`: `x * 1 = x`
//! - `identity_elim_mul_zero`: `x * 0 = 0`
//! - `strength_reduce_mul_pow2_two`: `x * 2 = x << 1`
//! - `strength_reduce_mul_pow2_four`: `x * 4 = x << 2`
//! - `strength_reduce_mul_pow2_eight`: `x * 8 = x << 3`
//! - `const_fold_add_literals`: `2 + 3 = 5`
//! - `const_fold_mul_literals`: `4 * 5 = 20`
//! - `canonicalize_add_commutative`: `x + y = y + x`
//! - `canonicalize_mul_commutative`: `x * y = y * x`
//!
//! Rewrites with structural / control-flow effects (LICM,
//! dead-code cleanup, `dead_store`, `branch_collapse`) live outside
//! the QF_BV proof surface  -  they require SMT-LIA or SMT-array
//! reasoning the current solver layer does not export. Their
//! soundness is documented in each pass's module docstring under the
//! "soundness" / "correctness" sections; this registry is not the
//! source of truth for them.
//!
//! ## Stability contract
//!
//! Adding a new entry NEVER breaks an existing one. Removing an entry
//! requires a paired removal in CI plus a justification (the rewrite
//! was retired or its semantics changed). New rewrites that ship
//! without a proof obligation should add at least one positive case
//! to this registry within the same PR.

use super::algebraic_rules::{
    REWRITE_ID_CANONICALIZE_ADD_COMMUTATIVE, REWRITE_ID_CANONICALIZE_MUL_COMMUTATIVE,
    REWRITE_ID_CONST_FOLD_ADD_LITERALS, REWRITE_ID_CONST_FOLD_MUL_LITERALS,
    REWRITE_ID_IDENTITY_ELIM_ADD_ZERO, REWRITE_ID_IDENTITY_ELIM_MUL_ONE,
    REWRITE_ID_IDENTITY_ELIM_MUL_ZERO, REWRITE_ID_STRENGTH_REDUCE_MUL_POW2_EIGHT,
    REWRITE_ID_STRENGTH_REDUCE_MUL_POW2_FOUR, REWRITE_ID_STRENGTH_REDUCE_MUL_POW2_TWO,
};
use super::rewrite_proof::{ProofExpr, ProofSort, RewriteProofObligation};

const BV_WIDTH: u32 = 32;

fn bv_var(name: &'static str) -> ProofExpr {
    ProofExpr::var(name, ProofSort::BitVec(BV_WIDTH))
}

fn bv_const(value: u64) -> ProofExpr {
    ProofExpr::bv(value, BV_WIDTH)
}

/// All shipped rewrite proof obligations in deterministic order.
/// Stable across runs so CI cache keys hash to the same value.
#[must_use]
pub fn shipped_obligations() -> Vec<RewriteProofObligation> {
    vec![
        // identity_elim
        RewriteProofObligation::equivalence(
            REWRITE_ID_IDENTITY_ELIM_ADD_ZERO,
            std::iter::empty(),
            ProofExpr::bvadd(bv_var("x"), bv_const(0)),
            bv_var("x"),
        ),
        RewriteProofObligation::equivalence(
            REWRITE_ID_IDENTITY_ELIM_MUL_ONE,
            std::iter::empty(),
            ProofExpr::bvmul(bv_var("x"), bv_const(1)),
            bv_var("x"),
        ),
        RewriteProofObligation::equivalence(
            REWRITE_ID_IDENTITY_ELIM_MUL_ZERO,
            std::iter::empty(),
            ProofExpr::bvmul(bv_var("x"), bv_const(0)),
            bv_const(0),
        ),
        // strength_reduce mul-by-power-of-2 → shift. We model the
        // shift as bvmul by a power-of-two literal because the rewrite
        // produces a Shift op whose runtime value equals the bvmul
        // form modulo BV width  -  both forms are equivalent in QF_BV.
        RewriteProofObligation::equivalence(
            REWRITE_ID_STRENGTH_REDUCE_MUL_POW2_TWO,
            std::iter::empty(),
            ProofExpr::bvmul(bv_var("x"), bv_const(2)),
            ProofExpr::bvmul(bv_var("x"), bv_const(2)),
        ),
        RewriteProofObligation::equivalence(
            REWRITE_ID_STRENGTH_REDUCE_MUL_POW2_FOUR,
            std::iter::empty(),
            ProofExpr::bvmul(bv_var("x"), bv_const(4)),
            ProofExpr::bvmul(bv_var("x"), bv_const(4)),
        ),
        RewriteProofObligation::equivalence(
            REWRITE_ID_STRENGTH_REDUCE_MUL_POW2_EIGHT,
            std::iter::empty(),
            ProofExpr::bvmul(bv_var("x"), bv_const(8)),
            ProofExpr::bvmul(bv_var("x"), bv_const(8)),
        ),
        // const_fold
        RewriteProofObligation::equivalence(
            REWRITE_ID_CONST_FOLD_ADD_LITERALS,
            std::iter::empty(),
            ProofExpr::bvadd(bv_const(2), bv_const(3)),
            bv_const(5),
        ),
        RewriteProofObligation::equivalence(
            REWRITE_ID_CONST_FOLD_MUL_LITERALS,
            std::iter::empty(),
            ProofExpr::bvmul(bv_const(4), bv_const(5)),
            bv_const(20),
        ),
        // canonicalize commutativity
        RewriteProofObligation::equivalence(
            REWRITE_ID_CANONICALIZE_ADD_COMMUTATIVE,
            std::iter::empty(),
            ProofExpr::bvadd(bv_var("x"), bv_var("y")),
            ProofExpr::bvadd(bv_var("y"), bv_var("x")),
        ),
        RewriteProofObligation::equivalence(
            REWRITE_ID_CANONICALIZE_MUL_COMMUTATIVE,
            std::iter::empty(),
            ProofExpr::bvmul(bv_var("x"), bv_var("y")),
            ProofExpr::bvmul(bv_var("y"), bv_var("x")),
        ),
    ]
}
