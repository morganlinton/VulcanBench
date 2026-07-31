//! Regression tests for megakernel rule-catalog caller-owned scratch reuse.
#![cfg(feature = "megakernel-batch")]

use vyre_runtime::megakernel::rule_catalog::{
    accepted_rule_fingerprints_and_rejections_into, pack_rule_catalog_into, BatchRuleProgram,
    RuleCatalogPackingScratch,
};

#[test]
fn accepted_rule_fingerprints_into_reuses_caller_storage() {
    let rules = (0..8)
        .map(|rule_idx| BatchRuleProgram::new(rule_idx, vec![0; 256], vec![0], 1).unwrap())
        .collect::<Vec<_>>();
    let mut fingerprints = Vec::with_capacity(16);
    let mut occupied = Vec::with_capacity(16);
    let mut addressed = Vec::with_capacity(16);
    let mut rejections = Vec::with_capacity(16);
    let fingerprint_ptr = fingerprints.as_ptr();
    let occupied_ptr = occupied.as_ptr();
    let addressed_ptr = addressed.as_ptr();
    let rejection_ptr = rejections.as_ptr();

    accepted_rule_fingerprints_and_rejections_into(
        &rules,
        &mut fingerprints,
        &mut occupied,
        &mut addressed,
        &mut rejections,
    );

    assert!(rejections.is_empty());
    assert_eq!(fingerprints.len(), rules.len());
    assert_eq!(fingerprints.as_ptr(), fingerprint_ptr);
    assert_eq!(occupied.as_ptr(), occupied_ptr);
    assert_eq!(addressed.as_ptr(), addressed_ptr);
    assert_eq!(rejections.as_ptr(), rejection_ptr);
}

#[test]
fn pack_rule_catalog_into_reuses_caller_storage() {
    let rules = (0..8)
        .map(|rule_idx| BatchRuleProgram::new(rule_idx, vec![0; 256], vec![0], 1).unwrap())
        .collect::<Vec<_>>();
    let mut scratch = RuleCatalogPackingScratch::default();

    pack_rule_catalog_into(&rules, &mut scratch).expect("initial packing must succeed");
    let rule_meta_ptr = scratch.rule_meta.as_ptr();
    let transitions_ptr = scratch.transitions.as_ptr();
    let accept_ptr = scratch.accept.as_ptr();
    let rejections_ptr = scratch.rejected_rules.as_ptr();

    pack_rule_catalog_into(&rules, &mut scratch).expect("repeated packing must succeed");

    assert_eq!(scratch.rule_meta.len(), rules.len());
    assert_eq!(scratch.rule_meta.as_ptr(), rule_meta_ptr);
    assert_eq!(scratch.transitions.as_ptr(), transitions_ptr);
    assert_eq!(scratch.accept.as_ptr(), accept_ptr);
    assert_eq!(scratch.rejected_rules.as_ptr(), rejections_ptr);
    assert!(scratch.rejected_rules.is_empty());
}
