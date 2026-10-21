//! Effects-handler scheduler enforcement.

use super::*;
use crate::lower::effects::compute_program_effects;

#[test]
fn effect_handler_enforcement_disabled_by_default_keeps_effect_adding_rewrites() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(BarrierAddingPass {
        metadata: PassMetadata::new("effect_add_default_off", &[], &[]),
        allowed: ProgramEffects::empty(),
    })]);
    assert!(
        !scheduler.effect_handler_enforcement(),
        "effect-handler enforcement must default to OFF for compatibility"
    );

    let report = scheduler
        .run(trivial_program())
        .expect("Fix: scheduler must converge");

    assert!(
        compute_program_effects(&report).contains(ProgramEffects::BARRIER),
        "with the gate disabled, an effect-adding rewrite still lands"
    );
}

#[test]
fn effect_handler_enforcement_reverts_undeclared_new_effects() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(BarrierAddingPass {
        metadata: PassMetadata::new("effect_add_forbidden", &[], &[]),
        allowed: ProgramEffects::empty(),
    })])
    .with_effect_handler_enforcement(true);

    let pre = trivial_program();
    let pre_effects = compute_program_effects(&pre);
    let post = scheduler
        .run(pre)
        .expect("Fix: effect-handler revert must converge");

    assert_eq!(
        compute_program_effects(&post),
        pre_effects,
        "undeclared effect additions must be reverted, not silently compiled"
    );
}

#[test]
fn effect_handler_enforcement_metrics_reflect_post_revert_state() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(BarrierAddingPass {
        metadata: PassMetadata::new("effect_add_metric_check", &[], &[]),
        allowed: ProgramEffects::empty(),
    })])
    .with_effect_handler_enforcement(true);

    let report = scheduler
        .run_with_metrics(trivial_program())
        .expect("Fix: metrics run must converge");
    assert_eq!(report.passes.len(), 1);
    let metric = &report.passes[0];

    assert!(metric.ran, "effect-adding pass must have run");
    assert!(!metric.changed, "reverted effect additions must not land");
    assert_eq!(metric.decision, PassRunDecision::EffectReverted);
    assert_eq!(metric.effect_bits_before, metric.effect_bits_after);
    assert_eq!(metric.refusal_kind, None);
}

#[test]
fn effect_handler_enforcement_allows_declared_effect_additions() {
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(BarrierAddingPass {
        metadata: PassMetadata::new("effect_add_declared", &[], &[]),
        allowed: ProgramEffects::BARRIER,
    })])
    .with_effect_handler_enforcement(true);

    let report = scheduler
        .run_with_metrics(trivial_program())
        .expect("Fix: declared effect additions must converge");
    let metric = report
        .passes
        .iter()
        .find(|metric| metric.ran)
        .expect("Fix: declared effect addition should produce one ran metric row");

    assert!(metric.changed, "declared barrier addition should land");
    assert_eq!(metric.decision, PassRunDecision::Changed);
    assert_ne!(metric.effect_bits_before, metric.effect_bits_after);
}
