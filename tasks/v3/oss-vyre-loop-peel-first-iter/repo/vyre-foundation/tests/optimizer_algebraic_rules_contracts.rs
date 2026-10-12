//! Integration contracts for optimizer algebraic rule helpers.

use std::collections::BTreeSet;

use vyre_foundation::ir::BinOp;
use vyre_foundation::optimizer::algebraic_rules::{
    arithmetic_rewrite_proof_contracts, binop_identity_replacement,
    strength_reduce_power_of_two_shift, IdentityReplacement, ScalarLiteral,
    REWRITE_ID_CANONICALIZE_ADD_COMMUTATIVE, REWRITE_ID_CANONICALIZE_MUL_COMMUTATIVE,
    REWRITE_ID_CONST_FOLD_ADD_LITERALS, REWRITE_ID_CONST_FOLD_MUL_LITERALS,
    REWRITE_ID_IDENTITY_ELIM_ADD_ZERO, REWRITE_ID_IDENTITY_ELIM_MUL_ONE,
    REWRITE_ID_IDENTITY_ELIM_MUL_ZERO, REWRITE_ID_STRENGTH_REDUCE_MUL_POW2_EIGHT,
    REWRITE_ID_STRENGTH_REDUCE_MUL_POW2_FOUR, REWRITE_ID_STRENGTH_REDUCE_MUL_POW2_TWO,
};

#[test]
fn identity_rules_cover_bool_and_integer_absorbers() {
    assert_eq!(
        binop_identity_replacement(BinOp::And, false, None, Some(ScalarLiteral::Bool(true))),
        Some(IdentityReplacement::Left)
    );
    assert_eq!(
        binop_identity_replacement(BinOp::Or, false, Some(ScalarLiteral::Bool(true)), None),
        Some(IdentityReplacement::Left)
    );
    assert_eq!(
        binop_identity_replacement(
            BinOp::BitAnd,
            false,
            None,
            Some(ScalarLiteral::U32(u32::MAX)),
        ),
        Some(IdentityReplacement::Left)
    );
    assert_eq!(
        binop_identity_replacement(BinOp::Mul, false, None, Some(ScalarLiteral::U32(0))),
        Some(IdentityReplacement::Right)
    );
}

#[test]
fn strength_reduce_power_of_two_excludes_one_and_zero() {
    assert_eq!(strength_reduce_power_of_two_shift(0), None);
    assert_eq!(strength_reduce_power_of_two_shift(1), None);
    assert_eq!(strength_reduce_power_of_two_shift(8), Some(3));
}

#[test]
fn arithmetic_rewrite_proof_registration_names_every_qfbv_rewrite_once() {
    let contracts = arithmetic_rewrite_proof_contracts();
    let ids = contracts
        .iter()
        .map(|contract| contract.rewrite_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ids.len(),
        contracts.len(),
        "arithmetic rewrite proof ids must be unique"
    );
    assert_eq!(
        ids,
        BTreeSet::from([
            REWRITE_ID_IDENTITY_ELIM_ADD_ZERO,
            REWRITE_ID_IDENTITY_ELIM_MUL_ONE,
            REWRITE_ID_IDENTITY_ELIM_MUL_ZERO,
            REWRITE_ID_STRENGTH_REDUCE_MUL_POW2_TWO,
            REWRITE_ID_STRENGTH_REDUCE_MUL_POW2_FOUR,
            REWRITE_ID_STRENGTH_REDUCE_MUL_POW2_EIGHT,
            REWRITE_ID_CONST_FOLD_ADD_LITERALS,
            REWRITE_ID_CONST_FOLD_MUL_LITERALS,
            REWRITE_ID_CANONICALIZE_ADD_COMMUTATIVE,
            REWRITE_ID_CANONICALIZE_MUL_COMMUTATIVE,
        ]),
        "registered arithmetic proof ids must cover the solver-backed rewrite families"
    );
    assert!(
        contracts
            .iter()
            .all(|contract| !contract.family.is_empty() && !contract.rewrite_id.is_empty()),
        "arithmetic proof registrations must keep both family and rewrite id"
    );
}
