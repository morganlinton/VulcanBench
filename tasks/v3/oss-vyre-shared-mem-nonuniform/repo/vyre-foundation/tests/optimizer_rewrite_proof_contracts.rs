//! Integration contracts for optimizer rewrite proof SMT generation.

use vyre_foundation::optimizer::rewrite_proof::{ProofExpr, ProofSort, RewriteProofObligation};

#[test]
fn emits_add_zero_equivalence_obligation() {
    let x = ProofExpr::var("x", ProofSort::BitVec(32));
    let proof = RewriteProofObligation::equivalence(
        "u32.add_zero_rhs",
        [],
        ProofExpr::bvadd(x.clone(), ProofExpr::bv(0, 32)),
        x,
    );
    let smt = proof.to_smt2();

    assert!(smt.contains("(set-logic QF_BV)"));
    assert!(smt.contains("(declare-fun x () (_ BitVec 32))"));
    assert!(smt.contains("(assert (not (= (bvadd x (_ bv0 32)) x)))"));
    assert!(smt.ends_with("(check-sat)\n"));
}

#[test]
fn declarations_are_deterministic() {
    let z = ProofExpr::var("z", ProofSort::BitVec(32));
    let a = ProofExpr::var("a", ProofSort::BitVec(32));
    let proof = RewriteProofObligation::equivalence(
        "deterministic",
        [],
        ProofExpr::bvadd(z, ProofExpr::bv(0, 32)),
        a,
    );
    let smt = proof.to_smt2();
    let a_pos = smt.find("(declare-fun a").unwrap();
    let z_pos = smt.find("(declare-fun z").unwrap();

    assert!(a_pos < z_pos);
}

#[test]
fn preconditions_are_asserted_before_negated_equivalence() {
    let x = ProofExpr::var("x", ProofSort::BitVec(32));
    let y = ProofExpr::var("y", ProofSort::BitVec(32));
    let pre = ProofExpr::eq(y.clone(), ProofExpr::bv(3, 32));
    let proof = RewriteProofObligation::equivalence(
        "with_pre",
        [pre],
        ProofExpr::bvadd(x.clone(), y),
        ProofExpr::bvadd(x, ProofExpr::bv(3, 32)),
    );
    let smt = proof.to_smt2();
    let pre_pos = smt.find("(assert (= y (_ bv3 32)))").unwrap();
    let proof_pos = smt.find("(assert (not").unwrap();

    assert!(pre_pos < proof_pos);
}

#[test]
fn escaped_symbols_are_valid_smt_identifiers() {
    let x = ProofExpr::var("loop index", ProofSort::BitVec(32));
    let proof = RewriteProofObligation::equivalence("escape", [], x, ProofExpr::bv(0, 32));
    let smt = proof.to_smt2();

    assert!(smt.contains("(declare-fun |loop index| () (_ BitVec 32))"));
}

#[test]
fn invalid_sort_combinations_are_rejected_before_smt_emission() {
    let bool_expr = ProofExpr::bool(true);
    let bv_expr = ProofExpr::bv(1, 32);

    let invalid_bvadd = std::panic::catch_unwind(|| {
        let _ = ProofExpr::bvadd(bool_expr.clone(), bv_expr.clone());
    });
    assert_eq!(
        invalid_bvadd.is_err(),
        true,
        "bit-vector operators must reject boolean operands"
    );

    let invalid_equivalence = std::panic::catch_unwind(|| {
        let _ = RewriteProofObligation::equivalence("bad_sort", [], bool_expr, bv_expr);
    });
    assert_eq!(
        invalid_equivalence.is_err(),
        true,
        "rewrite obligations must reject mismatched before/after sorts"
    );
}
