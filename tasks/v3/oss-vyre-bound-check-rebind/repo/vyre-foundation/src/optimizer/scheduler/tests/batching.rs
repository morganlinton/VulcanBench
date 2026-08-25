//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn scheduler_uses_batch_apply_for_many_rewrite_candidates() {
    let batch_calls = Arc::new(AtomicUsize::new(0));
    let transform_calls = Arc::new(AtomicUsize::new(0));
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(BatchingPass {
        batch_calls: Arc::clone(&batch_calls),
        transform_calls: Arc::clone(&transform_calls),
        threshold: 1,
    })]);

    let optimized = scheduler
        .run(repeated_store_program(6))
        .expect("Fix: planar batched rewrite pass must converge");

    assert_eq!(
        batch_calls.load(Ordering::Relaxed),
        2,
        "six adjacent candidates with k=2 on one row must land in two disjoint waves"
    );
    assert_eq!(
        transform_calls.load(Ordering::Relaxed),
        0,
        "scheduler must call ProgramPass::batch_apply, not the sequential transform fallback"
    );
    assert!(
        all_stores_have_value(optimized.entry(), 43),
        "every candidate must be rewritten across the planned waves"
    );
}

#[test]
fn batch_apply_uses_sequential_fallback_under_threshold() {
    let batch_calls = Arc::new(AtomicUsize::new(0));
    let transform_calls = Arc::new(AtomicUsize::new(0));
    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(BatchingPass {
        batch_calls: Arc::clone(&batch_calls),
        transform_calls: Arc::clone(&transform_calls),
        threshold: 8,
    })]);

    let optimized = scheduler
        .run(repeated_store_program(3))
        .expect("Fix: sequential fallback rewrite pass must converge");

    assert_eq!(batch_calls.load(Ordering::Relaxed), 0);
    assert_eq!(transform_calls.load(Ordering::Relaxed), 1);
    assert!(
        all_stores_have_value(optimized.entry(), 43),
        "sequential fallback must preserve the same rewrite semantics"
    );
}
