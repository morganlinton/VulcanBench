//! Execution-planning contract tests.

use vyre_foundation::execution_plan::{
    plan, plan_with_options, AccuracyStrategy, AutotuneStrategy, DispatchStrategy, FusionStrategy,
    InnovationTrack, LayoutStrategy, PlanError, PolicyRoute, ProvenanceStrategy, ReadbackStrategy,
    SchedulingPolicy,
};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::validate::{BackendCapabilities, ValidationOptions};

fn ranged_output_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)
            .with_count(1024)
            .with_output_byte_range(4..12)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
    )
}

#[test]
fn plan_tracks_readback_minimization() {
    let plan = plan(&ranged_output_program()).expect("canonical ranged output program must plan");
    assert_eq!(plan.memory.visible_readback_bytes, 8);
    assert_eq!(plan.memory.avoided_readback_bytes, 4088);
    assert!(plan.track_active(InnovationTrack::ReadbackMinimization));
}

#[test]
fn plan_marks_subgroup_program_accuracy_sensitive() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind(
            "x",
            Expr::subgroup_add(Expr::u32(1)),
        )],
    );
    let options = ValidationOptions::default().with_backend_capabilities(BackendCapabilities {
        supports_subgroup_ops: true,
        ..BackendCapabilities::default()
    });
    let plan = plan_with_options(&program, options).expect("subgroup-capable backend must plan");
    assert!(plan.required_capabilities.subgroup_ops);
    assert!(plan.track_active(InnovationTrack::DifferentialAccuracy));
}

#[test]
fn plan_rejects_subgroup_program_without_capability_context() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind(
            "x",
            Expr::subgroup_add(Expr::u32(1)),
        )],
    );
    let err = plan(&program).expect_err("subgroup program needs backend capability context");
    assert!(
        err.to_string().contains("subgroup-ops support"),
        "capability-sensitive rejection must name subgroup support, got {err}"
    );
}

#[test]
fn plan_marks_wrapped_program_fusion_candidate() {
    let plan = plan(&ranged_output_program()).expect("canonical ranged output program must plan");
    assert!(plan.fusion.batch_fusion_candidate);
    assert!(plan.track_active(InnovationTrack::WholeProgramFusion));
    assert!(plan.provenance.top_level_region_wrapped);
}

#[test]
fn strategy_encodes_all_seven_tracks_for_small_trimmed_program() {
    let plan = plan(&ranged_output_program()).expect("canonical ranged output program must plan");
    assert_eq!(plan.strategy.fusion, FusionStrategy::Candidate);
    assert_eq!(plan.strategy.dispatch, DispatchStrategy::PersistentRuntime);
    assert_eq!(plan.strategy.accuracy, AccuracyStrategy::Direct);
    assert_eq!(plan.strategy.autotune, AutotuneStrategy::DeclaredShape);
    assert_eq!(plan.strategy.provenance, ProvenanceStrategy::GpuTrace);
    assert_eq!(plan.strategy.layout, LayoutStrategy::Static);
    assert_eq!(
        plan.strategy.readback,
        ReadbackStrategy::Trimmed {
            visible_bytes: 8,
            avoided_bytes: 4088,
        }
    );
}

#[test]
fn strategy_marks_large_program_for_persistent_runtime_and_autotune() {
    let body: Vec<Node> = (0..65)
        .map(|idx| Node::store("out", Expr::u32(idx), Expr::u32(idx)))
        .collect();
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(128)],
        [128, 1, 1],
        body,
    );
    let plan = plan(&program).expect("large static program must plan");
    assert_eq!(plan.strategy.dispatch, DispatchStrategy::PersistentRuntime);
    assert_eq!(plan.strategy.autotune, AutotuneStrategy::MeasureVariants);
}

#[test]
fn shared_policy_owns_strategy_and_route_boundaries() {
    let policy = SchedulingPolicy::standard();
    assert!(policy.use_persistent_runtime(64));
    assert!(policy.use_persistent_runtime(65));
    assert!(!policy.recommend_autotune(64));
    assert!(policy.recommend_autotune(65));
    assert_eq!(
        policy.route(64, (1 << 16) - 1),
        PolicyRoute::PersistentMegakernel
    );
    assert_eq!(policy.route(64, 1 << 16), PolicyRoute::PersistentMegakernel);
    assert_eq!(
        policy.route(1025, 1 << 16),
        PolicyRoute::PersistentMegakernel
    );
}

#[test]
fn runtime_sized_storage_buffers_remain_dynamic_layout() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
    );
    let plan = plan(&program).expect("runtime-sized input storage must be wire-roundtrippable");
    assert_eq!(plan.strategy.layout, LayoutStrategy::Dynamic);
    assert_eq!(plan.memory.dynamic_buffers, 1);
}

#[test]
fn zero_count_output_is_rejected_before_strategy() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
    );
    let err = plan(&program).expect_err("dynamic output must not plan without a concrete size");
    assert!(
        matches!(err, PlanError::NonCanonicalProgram { .. }),
        "invalid Program must surface as PlanError::NonCanonicalProgram"
    );
    assert!(
        err.to_string().contains("canonical execution plan"),
        "Fix: planning errors must explain canonicality, got {err}"
    );
}

#[test]
fn inverted_output_byte_range_is_rejected_with_named_error() {
    let inverted = std::ops::Range { start: 12, end: 4 };
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)
            .with_count(1024)
            .with_output_byte_range(inverted)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
    );
    let err = plan(&program).expect_err("inverted range must fail planning");
    assert!(
        matches!(err, PlanError::InvalidOutputRange { ref name, start: 12, end: 4, .. } if name == "out"),
        "expected InvalidOutputRange for inverted range, got {err:?}"
    );
    let msg = err.to_string();
    assert!(msg.contains("out"), "error must name the buffer: {msg}");
    assert!(msg.contains("12"), "error must name the start: {msg}");
    assert!(msg.contains("4"), "error must name the end: {msg}");
}

#[test]
fn output_byte_range_past_end_is_rejected_with_named_error() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)
            .with_count(4)
            .with_output_byte_range(0..64)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
    );
    let err = plan(&program).expect_err("range past end must fail planning");
    assert!(
        matches!(err, PlanError::InvalidOutputRange { ref name, start: 0, end: 64, .. } if name == "out"),
        "expected InvalidOutputRange for range past end, got {err:?}"
    );
}

#[test]
#[allow(deprecated)]
fn unwrapped_deprecated_constructor_rejected_by_plan() {
    let program = Program::new(vec![], [1, 1, 1], vec![Node::Return]);
    let err = plan(&program).expect_err("unwrapped program must not plan");
    assert!(
        matches!(err, PlanError::NonCanonicalProgram { .. }),
        "expected NonCanonicalProgram for unwrapped entry, got {err:?}"
    );
    let msg = err.to_string();
    assert!(msg.contains("Fix:"), "error must be actionable: {msg}");
}
