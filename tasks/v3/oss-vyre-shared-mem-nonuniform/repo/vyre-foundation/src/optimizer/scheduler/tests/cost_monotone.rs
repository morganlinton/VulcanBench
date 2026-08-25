//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn cost_monotone_disabled_by_default_keeps_cost_up_rewrites() {
    // The TestPass with `changes: true` appends a Node::barrier to the entry,
    // which increases `node_count` by 1  -  a strict cost-up rewrite that the
    // monotone-down gate must catch when enabled. With the gate OFF (default),
    // the scheduler keeps the rewrite for backwards compatibility.
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(TestPass {
        metadata: PassMetadata::new("cost_up_default_off", &[], &[]),
        changes: true,
    })]);
    assert!(
        !scheduler.cost_monotone_enforcement(),
        "cost-monotone enforcement must default to OFF  -  flipping the default would change the \
         optimizer's observable behavior on every consumer that constructs PassScheduler::default()"
    );

    let pre = trivial_program();
    let pre_nodes = pre.stats().node_count;
    let report = scheduler.run(pre).expect("Fix: scheduler must converge");
    assert!(
        report.stats().node_count > pre_nodes,
        "with the gate disabled, the cost-up rewrite must land  -  got post_nodes={} pre_nodes={}",
        report.stats().node_count,
        pre_nodes
    );
}

#[test]
fn cost_monotone_enabled_reverts_cost_up_rewrites() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(TestPass {
        metadata: PassMetadata::new("cost_up_with_gate", &[], &[]),
        changes: true,
    })])
    .with_cost_monotone_enforcement(true);
    assert!(scheduler.cost_monotone_enforcement());

    let pre = trivial_program();
    let pre_nodes = pre.stats().node_count;
    let report = scheduler
        .run(pre.clone())
        .expect("Fix: scheduler must converge even when the gate reverts a cost-up rewrite");
    assert_eq!(
        report.stats().node_count,
        pre_nodes,
        "the gate must revert any pass that increases node_count without an explicit refusal  -  \
         observed post_nodes={} pre_nodes={}",
        report.stats().node_count,
        pre_nodes
    );
}

#[test]
fn cost_monotone_enabled_keeps_monotone_down_rewrites() {
    // A no-op pass returns `PassResult::unchanged`  -  node_count is identical
    // pre/post, so the gate must accept it. This proves the gate doesn't
    // over-revert (i.e. it doesn't mistake an unchanged result for a violation).
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(TestPass {
        metadata: PassMetadata::new("noop_with_gate", &[], &[]),
        changes: false,
    })])
    .with_cost_monotone_enforcement(true);

    let pre = trivial_program();
    let pre_nodes = pre.stats().node_count;
    let report = scheduler.run(pre).expect("Fix: scheduler must converge");
    assert_eq!(
        report.stats().node_count,
        pre_nodes,
        "the gate must NOT mutate Programs that the pass left unchanged"
    );
}

#[test]
fn cost_monotone_enabled_metrics_reflect_post_revert_state() {
    // When the gate reverts a rewrite, the per-pass metric's `changed` flag
    // must be false (no change ACTUALLY landed) and `nodes_after` must equal
    // `nodes_before`. Without this contract, downstream attribution would
    // record a phantom change.
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(TestPass {
        metadata: PassMetadata::new("cost_up_metric_check", &[], &[]),
        changes: true,
    })])
    .with_cost_monotone_enforcement(true);

    let report = scheduler
        .run_with_metrics(trivial_program())
        .expect("Fix: metrics run must converge");
    assert_eq!(report.passes.len(), 1);
    let metric = &report.passes[0];
    assert!(
        metric.ran,
        "the pass must have actually been called by the scheduler"
    );
    assert!(
        !metric.changed,
        "after gate-revert, the metric's `changed` flag must reflect that no change landed; \
         got changed={}",
        metric.changed
    );
    assert_eq!(metric.decision, PassRunDecision::CostReverted);
    assert_eq!(metric.refusal_kind, None);
    assert_eq!(
        metric.nodes_after, metric.nodes_before,
        "after gate-revert, nodes_before must equal nodes_after  -  the metric describes the \
         post-gate Program shape, not the rejected rewrite"
    );
}

#[test]
fn cost_monotone_enabled_honors_explicit_try_transform_refusal() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(RefusingPass {
        metadata: PassMetadata::new("explicit_cost_refusal", &[], &[]),
    })])
    .with_cost_monotone_enforcement(true);

    let pre = trivial_program();
    let pre_nodes = pre.stats().node_count;
    let report = scheduler
        .run(pre)
        .expect("Fix: explicit cost refusal should be treated as a no-op rewrite");

    assert_eq!(
        report.stats().node_count,
        pre_nodes,
        "explicit try_transform refusal must preserve the pre-pass Program"
    );
}

#[test]
fn cost_monotone_metrics_honor_explicit_try_transform_refusal() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(RefusingPass {
        metadata: PassMetadata::new("explicit_cost_refusal_metrics", &[], &[]),
    })])
    .with_cost_monotone_enforcement(true);

    let report = scheduler
        .run_with_metrics(trivial_program())
        .expect("Fix: explicit cost refusal should converge in metrics mode");
    assert_eq!(report.passes.len(), 1);
    assert!(report.passes[0].ran);
    assert!(
        !report.passes[0].changed,
        "explicit refusal must not be recorded as a landed rewrite"
    );
    assert_eq!(report.passes[0].decision, PassRunDecision::Refused);
    assert_eq!(report.passes[0].refusal_kind, Some("cost_increase"));
    assert_eq!(
        report.passes[0].nodes_after, report.passes[0].nodes_before,
        "metrics must describe the preserved pre-refusal Program"
    );
}
