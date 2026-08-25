//! Optimizer framework tests.

use super::*;
use crate::ir::{BufferDecl, DataType, Expr, Node, Program};

fn trivial_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("input", Expr::u32(0)),
        )],
    )
}

const _: () = assert!(PassAnalysis::RUN.should_run);
const _: () = assert!(!PassAnalysis::SKIP.should_run);

#[test]
fn pass_result_unchanged_reports_no_change() {
    let result = PassResult::unchanged(trivial_program());
    assert!(!result.changed);
}

#[test]
fn pass_result_from_programs_identical() {
    let p = trivial_program();
    let result = PassResult::from_programs(&p, p.clone());
    assert!(!result.changed);
}

#[test]
fn pass_metadata_construction() {
    let meta = PassMetadata::new("test_pass", &["dead_buffer_elim"], &["fusion"]);
    assert_eq!(meta.name, "test_pass");
    assert_eq!(meta.requires.len(), 1);
    assert_eq!(meta.invalidates.len(), 1);
}

#[test]
fn optimizer_error_max_iterations_display() {
    let err = OptimizerError::MaxIterations {
        max_iterations: 100,
        last_pass: "const_fold",
    };
    let msg = err.to_string();
    assert!(msg.contains("100"));
    assert!(msg.contains("const_fold"));
}

#[test]
fn optimizer_error_unsatisfied_requirement_display() {
    let err = OptimizerError::UnsatisfiedRequirement {
        pass: "fusion",
        missing: "dead_buffer_elim",
    };
    let msg = err.to_string();
    assert!(msg.contains("fusion"));
    assert!(msg.contains("dead_buffer_elim"));
}

#[test]
fn fingerprint_is_deterministic() {
    let p = trivial_program();
    assert_eq!(fingerprint_program(&p), fingerprint_program(&p));
}

#[test]
fn fingerprint_different_programs_differ() {
    let p1 = trivial_program();
    let p2 = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    );
    assert_ne!(fingerprint_program(&p1), fingerprint_program(&p2));
}

#[test]
fn requirements_satisfied_empty_requires() {
    let meta = PassMetadata::new("trivial", &[], &[]);
    let available = FxHashSet::default();
    assert!(requirements_satisfied(meta, &available));
}

#[test]
fn requirements_satisfied_missing_dep() {
    let meta = PassMetadata::new("needs_stuff", &["missing"], &[]);
    let available = FxHashSet::default();
    assert!(!requirements_satisfied(meta, &available));
}

#[test]
fn refusal_reason_kind_tags_are_stable() {
    let cost = RefusalReason::CostIncrease {
        delta: 17,
        detail: "fusion would add 12 atomic ops",
    };
    assert_eq!(cost.kind(), "cost_increase");

    let effect = RefusalReason::EffectLatticeViolation {
        producer: "vyre-libs::dataflow::reaching",
        consumer: "vyre-primitives::reduce::scan",
        suggested_fix: "insert MemoryOrdering::GridSync between arms",
    };
    assert_eq!(effect.kind(), "effect_lattice_violation");

    let wire = RefusalReason::WireContractViolation {
        detail: "op_id drift detected: vyre-primitives::math::add became vyre::add",
    };
    assert_eq!(wire.kind(), "wire_contract_violation");

    let other = RefusalReason::Other {
        detail: "user-provided refusal",
    };
    assert_eq!(other.kind(), "other");
}

#[test]
fn refusal_reason_display_includes_payload() {
    let cost = RefusalReason::CostIncrease {
        delta: 42,
        detail: "extra atomics",
    };
    let msg = cost.to_string();
    assert!(msg.contains("cost_increase"));
    assert!(msg.contains("42"));
    assert!(msg.contains("extra atomics"));

    let effect = RefusalReason::EffectLatticeViolation {
        producer: "p",
        consumer: "c",
        suggested_fix: "barrier",
    };
    let msg = effect.to_string();
    assert!(msg.contains("p"));
    assert!(msg.contains("c"));
    assert!(msg.contains("barrier"));
}

#[test]
fn try_transform_default_delegates_to_transform_for_every_builtin() {
    let p = trivial_program();
    let passes = registered_passes()
        .expect("Fix: registered_passes should succeed; restore this invariant before continuing.");
    for pass in passes {
        let result = pass.try_transform(p.clone());
        let _optimized = result.unwrap_or_else(|e| {
            panic!(
                "built-in pass `{}` unexpectedly returned a refusal: {e:?}",
                pass.metadata().name
            );
        });
    }
}

#[test]
fn preserves_default_is_empty_for_every_builtin() {
    let passes = registered_passes()
        .expect("Fix: registered_passes should succeed; restore this invariant before continuing.");
    for pass in passes {
        assert!(
            pass.preserves().is_empty(),
            "built-in pass `{}` declared a preserves[] entry but the scheduler doesn't \
             yet honor it; either wire the scheduler or remove the declaration",
            pass.metadata().name
        );
    }
}

#[test]
fn registered_passes_includes_builtins() {
    let passes = registered_passes()
        .expect("Fix: registered_passes should succeed; restore this invariant before continuing.");
    assert!(passes.len() >= 19, "at least 19 builtin passes");
    let names: Vec<_> = passes.iter().map(|p| p.metadata().name).collect();
    assert!(names.contains(&"autotune"));
    assert!(names.contains(&"buffer_decl_sort"));
    assert!(names.contains(&"canonicalize"));
    assert!(names.contains(&"const_fold"));
    assert!(names.contains(&"loop_redundant_bound_check_elide"));
    assert!(names.contains(&"loop_trip_zero_eliminate"));
    assert!(names.contains(&"if_constant_branch_eliminate"));
    assert!(names.contains(&"empty_block_collapse"));
    assert!(names.contains(&"noop_assign_eliminate"));
    assert!(names.contains(&"region_promote_singleton_block"));
    assert!(names.contains(&"decode_scan_fuse"));
    assert!(names.contains(&"loop_unroll"));
    assert!(names.contains(&"vectorization"));
    assert!(names.contains(&"dead_buffer_elim"));
}
