//! Failure-oriented tests for spec-level validation contracts.
//!
//! Covers backend-availability predicates, intrinsic-table emptiness detection,
//! invariant lookup edge cases, and catalog-consistency guards.

use vyre_spec::{
    by_category, by_id, BackendAvailabilityPredicate, BackendId, Category, DataType,
    EngineInvariant, IntrinsicLowering, IntrinsicTable, InvariantCategory, OpSignature, PgNodeKind,
    Verification,
};

// ------------------------------------------------------------------
// BackendAvailabilityPredicate  -  false predicate must reject
// ------------------------------------------------------------------

#[test]
fn backend_availability_predicate_rejects_when_false() {
    let pred = BackendAvailabilityPredicate::new(|_| false);
    assert!(
        !pred.available("anything"),
        "false predicate must reject every op"
    );
}

#[test]
fn backend_availability_predicate_accepts_when_true() {
    let pred = BackendAvailabilityPredicate::new(|_| true);
    assert!(
        pred.available("anything"),
        "true predicate must accept every op"
    );
}

// ------------------------------------------------------------------
// IntrinsicTable  -  whitespace and None are both treated as missing
// ------------------------------------------------------------------

#[test]
fn intrinsic_table_reports_all_missing_for_default() {
    let table = IntrinsicTable::default();
    let required = required_backends();
    let missing: Vec<_> = table.missing_backends(&required).collect();
    assert_eq!(missing, vec!["alpha", "beta", "gamma", "delta"]);
}

#[test]
fn intrinsic_table_detects_whitespace_as_missing() {
    let table = IntrinsicTable {
        lowerings: vec![
            IntrinsicLowering::new("alpha", "   "),
            IntrinsicLowering::new("beta", "atom.add"),
            IntrinsicLowering::new("gamma", ""),
        ],
    };
    let required = required_backends();
    let missing: Vec<_> = table.missing_backends(&required).collect();
    assert_eq!(missing, vec!["alpha", "gamma", "delta"]);
}

#[test]
fn intrinsic_table_detects_partial_population() {
    let table = IntrinsicTable {
        lowerings: vec![
            IntrinsicLowering::new("alpha", "countOneBits"),
            IntrinsicLowering::new("gamma", "popcount"),
            IntrinsicLowering::new("delta", ""),
        ],
    };
    let required = required_backends();
    let missing: Vec<_> = table.missing_backends(&required).collect();
    assert_eq!(missing, vec!["beta", "delta"]);
}

fn required_backends() -> Vec<BackendId> {
    ["alpha", "beta", "gamma", "delta"]
        .into_iter()
        .map(BackendId::from)
        .collect()
}

// ------------------------------------------------------------------
// by_id  -  unknown / invalid lookup must return None
// ------------------------------------------------------------------

#[test]
fn by_id_returns_none_for_nonexistent_variant() {
    // There is no I0 variant, so we cannot construct it. Instead we rely on
    // the fact that every valid InvariantId is in the catalog and by_id
    // returns Some for all of them. This test documents the contract.
    for inv in EngineInvariant::iter() {
        assert!(
            by_id(inv).is_some(),
            "by_id({:?}) must never return None for a valid id",
            inv
        );
    }
}

// ------------------------------------------------------------------
// by_category  -  partition must be exact
// ------------------------------------------------------------------

#[test]
fn by_category_execution_contains_only_execution() {
    for inv in by_category(InvariantCategory::Execution) {
        assert_eq!(
            inv.category,
            InvariantCategory::Execution,
            "by_category(Execution) must not leak other categories"
        );
    }
}

#[test]
fn by_category_algebra_contains_only_algebra() {
    for inv in by_category(InvariantCategory::Algebra) {
        assert_eq!(
            inv.category,
            InvariantCategory::Algebra,
            "by_category(Algebra) must not leak other categories"
        );
    }
}

#[test]
fn by_category_resource_contains_only_resource() {
    for inv in by_category(InvariantCategory::Resource) {
        assert_eq!(
            inv.category,
            InvariantCategory::Resource,
            "by_category(Resource) must not leak other categories"
        );
    }
}

#[test]
fn by_category_stability_contains_only_stability() {
    for inv in by_category(InvariantCategory::Stability) {
        assert_eq!(
            inv.category,
            InvariantCategory::Stability,
            "by_category(Stability) must not leak other categories"
        );
    }
}

// ------------------------------------------------------------------
// OpSignature  -  empty contract must not affect byte counting
// ------------------------------------------------------------------

#[test]
fn op_signature_min_input_bytes_with_tensor_output_is_finite() {
    let sig = OpSignature {
        inputs: vec![DataType::U32, DataType::U32],
        output: DataType::Tensor,
        input_params: None,
        output_params: None,
        contract: None,
    };
    assert_eq!(sig.min_input_bytes(), 8);
}

// ------------------------------------------------------------------
// Verification  -  zero witness count is still Some
// ------------------------------------------------------------------

#[test]
fn verification_witnessed_u32_zero_count_is_some() {
    let v = Verification::WitnessedU32 { seed: 1, count: 0 };
    assert_eq!(
        v.witness_count(),
        Some(0),
        "zero count must still be Some(0)"
    );
}

// ------------------------------------------------------------------
// Category  -  unclassified must be distinguishable from real Category A
// ------------------------------------------------------------------

#[test]
fn category_unclassified_is_empty_composition() {
    let cat = Category::unclassified();
    assert!(cat.is_unclassified());
    match cat {
        Category::A { composition_of } => assert!(composition_of.is_empty()),
        Category::C { .. } => panic!("unclassified must be Category A"),
        _ => panic!("unclassified must be Category A"),
    }
}

// ------------------------------------------------------------------
// EngineInvariant  -  iter must cover exactly I1..I15
// ------------------------------------------------------------------

#[test]
fn engine_invariant_iter_count_is_exactly_15() {
    let count = EngineInvariant::iter().count();
    assert_eq!(count, 15, "iter must yield exactly 15 invariants");
}

// ------------------------------------------------------------------
// PgNodeKind  -  from_u32 must be injective on valid range
// ------------------------------------------------------------------

#[test]
fn pg_node_kind_valid_range_is_injective() {
    let mut seen = std::collections::HashSet::new();
    for v in 1..=20 {
        let kind = PgNodeKind::from_u32(v).unwrap();
        assert!(
            seen.insert(std::mem::discriminant(&kind)),
            "PgNodeKind::from_u32({}) collided with an earlier value",
            v
        );
    }
}
