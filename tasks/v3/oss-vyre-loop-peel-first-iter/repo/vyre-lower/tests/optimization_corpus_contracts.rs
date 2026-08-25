//! Test: optimization corpus contracts.
use std::collections::BTreeSet;

use vyre_lower::optimization_corpus::{
    generate_release_corpus, manifest_for, RELEASE_MIN_OPTIMIZATION_CASES,
};

#[test]
fn release_optimization_corpus_has_4096_plus_cases() {
    let cases = generate_release_corpus();
    assert!(
        RELEASE_MIN_OPTIMIZATION_CASES >= 4_096,
        "Fix: release optimization corpus floor is {RELEASE_MIN_OPTIMIZATION_CASES}; release requires at least 4096."
    );
    assert!(
        cases.len() >= RELEASE_MIN_OPTIMIZATION_CASES,
        "Fix: release optimization corpus generated {} cases, but release requires at least {}.",
        cases.len(),
        RELEASE_MIN_OPTIMIZATION_CASES
    );
}

#[test]
fn release_optimization_corpus_ids_are_unique() {
    let cases = generate_release_corpus();
    let mut ids = BTreeSet::new();
    for case in &cases {
        assert!(
            ids.insert(case.id.as_str()),
            "Fix: duplicate optimization corpus case id `{}`.",
            case.id
        );
    }
}

#[test]
fn release_optimization_corpus_has_multiple_families() {
    let cases = generate_release_corpus();
    let manifest = manifest_for(&cases);
    assert!(
        manifest.families.len() >= 14,
        "Fix: release optimization corpus must span at least 14 release optimization families; got {}.",
        manifest.families.len()
    );
    for family in &manifest.families {
        assert!(
            family.cases >= 128,
            "Fix: optimization family `{}` has only {} cases; release families must be matrixed, not token examples.",
            family.family,
            family.cases
        );
    }
}

#[test]
fn release_optimization_corpus_uses_weir_dataflow_family_contracts() {
    let cases = generate_release_corpus();
    let manifest = manifest_for(&cases);
    let family_names = manifest
        .families
        .iter()
        .map(|family| family.family.as_str())
        .collect::<BTreeSet<_>>();
    for required in [
        "weir-dataflow-dse",
        "weir-dataflow-loop-fusion",
        "weir-dataflow-loop-fission",
        "weir-dataflow-licm",
    ] {
        assert!(
            family_names.contains(required),
            "Fix: release optimization corpus must publish required Weir-aware family `{required}`."
        );
    }
    assert_eq!(
        manifest.dataflow_analysis_cases, 1024,
        "Fix: release optimization corpus must expose every Weir-aware optimization fixture through dataflow_analysis_cases."
    );
    assert_eq!(
        manifest.dataflow_analysis_optimized_cases, 1024,
        "Fix: every Weir-aware release optimization fixture must fire under dataflow facts."
    );
}

#[test]
fn release_optimization_corpus_descriptors_verify() {
    let cases = generate_release_corpus();
    let manifest = manifest_for(&cases);
    assert_eq!(
        manifest.verified_cases,
        cases.len(),
        "Fix: every generated corpus case must verify and optimize through the production rewrite entry point."
    );
    assert!(
        manifest.blockers.is_empty(),
        "Fix: generated corpus validation reported blockers: {:?}",
        manifest.blockers
    );
    for case in &cases {
        let errors = vyre_lower::verify(&case.descriptor);
        assert!(
            errors.is_ok(),
            "Fix: generated optimization corpus case `{}` produced an invalid KernelDescriptor: {:?}",
            case.id,
            errors
        );
    }
}
