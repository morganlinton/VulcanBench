use super::*;
use std::collections::{BTreeSet, HashSet};

#[test]
fn catalog_entries_have_owner_invariant_and_benchmark_contracts() {
    let catalog = optimization_catalog().expect("Fix: optimizer catalog must build");
    assert!(
        catalog.iter().any(|e| e.name == "const_fold"),
        "Fix: optimizer catalog must include const_fold."
    );
    for entry in &catalog {
        assert!(!entry.name.is_empty(), "catalog pass name must be stable");
        assert!(!entry.owner.is_empty(), "catalog pass owner must be set");
        assert!(
            !entry.invariant.is_empty(),
            "catalog pass invariant must be set"
        );
        assert!(
            !entry.benchmark.is_empty(),
            "catalog pass benchmark contract must be set"
        );
    }
}

#[test]
fn catalog_names_are_unique() {
    let catalog = optimization_catalog().expect("Fix: optimizer catalog must build");
    let mut seen = BTreeSet::new();
    for entry in catalog {
        assert!(
            seen.insert(entry.name),
            "Fix: duplicate optimizer pass name `{}` makes profile reports ambiguous.",
            entry.name
        );
    }
}

#[test]
fn release_catalog_exposes_100_plus_discoverable_optimizations() {
    const MIN_RELEASE_OPTIMIZATIONS: usize = 100;

    let catalog = optimization_catalog().expect("Fix: optimizer catalog must build");
    assert!(
            catalog.len() >= MIN_RELEASE_OPTIMIZATIONS,
            "Fix: optimizer catalog exposes only {} optimizations; release requires at least {MIN_RELEASE_OPTIMIZATIONS} discoverable optimizations with owner, phase, invariant, and benchmark contracts.",
            catalog.len()
        );

    let phases = catalog
        .iter()
        .map(|entry| entry.phase)
        .collect::<HashSet<_>>();
    let benchmarks = catalog
        .iter()
        .map(|entry| entry.benchmark)
        .collect::<BTreeSet<_>>();

    assert!(
            phases.len() >= 5,
            "Fix: optimizer catalog must span at least five scheduler phases so contributors can find the correct insertion point."
        );
    assert!(
            benchmarks.len() >= 5,
            "Fix: optimizer catalog must map optimizations to benchmark families, not one vague performance bucket."
        );
}

#[test]
fn catalog_names_plan_required_structural_optimization_families() {
    let catalog = optimization_catalog().expect("Fix: optimizer catalog must build");
    let names = catalog
        .iter()
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    for required in [
        "megakernel.allocation_reuse",
        "megakernel.launch_fusion",
        "dataflow.layout_normalization",
        "dataflow.frontier_density_switch",
        "dataflow.bitset_compression",
        "megakernel.readback_minimization",
    ] {
        assert!(
                names.contains(required),
                "Fix: optimizer catalog is missing required structural optimization family `{required}`."
            );
    }
}

#[test]
fn catalog_distinguishes_runnable_passes_from_supplemental_rules() {
    let catalog = optimization_catalog().expect("Fix: optimizer catalog must build");
    let executable_count = catalog
        .iter()
        .filter(|entry| entry.kind == OptimizationCatalogEntryKind::ExecutablePass)
        .count();
    let supplemental_count = catalog
        .iter()
        .filter(|entry| entry.kind == OptimizationCatalogEntryKind::SupplementalRule)
        .count();
    let registered_count = registered_pass_registrations()
        .expect("Fix: registered optimizer passes must schedule")
        .len();

    assert_eq!(
            executable_count, registered_count,
            "Fix: executable optimizer catalog entries must correspond exactly to runnable registered passes."
        );
    assert!(
        supplemental_count > 0,
        "Fix: supplemental rule entries should remain visible, but not counted as runnable passes."
    );
}

#[test]
fn release_catalog_entries_are_fully_classified() {
    let catalog = optimization_catalog_for_profile(OptimizerProfile::Release)
        .expect("Fix: release optimizer catalog must build");
    assert!(
        !catalog.is_empty(),
        "Fix: release optimizer profile must expose classified passes."
    );
    for entry in catalog {
        assert_ne!(
            entry.phase,
            PassPhase::Unclassified,
            "Fix: release pass `{}` must declare a concrete phase.",
            entry.name
        );
        assert_ne!(
            entry.boundary_class,
            PassBoundaryClass::Unknown,
            "Fix: release pass `{}` must declare an API boundary class.",
            entry.name
        );
        assert_ne!(
            entry.cost_model_family,
            CostModelFamily::Unknown,
            "Fix: release pass `{}` must declare a benchmark/cost model family.",
            entry.name
        );
        assert!(
                entry.preserves_abi,
                "Fix: release pass `{}` must preserve public buffer ABI or move behind an explicit opt-in profile.",
                entry.name
            );
        assert!(
                entry.requires_caps.is_empty(),
                "Fix: release pass `{}` must not require backend/runtime caps; use a backend-aware profile instead.",
                entry.name
            );
    }
}
