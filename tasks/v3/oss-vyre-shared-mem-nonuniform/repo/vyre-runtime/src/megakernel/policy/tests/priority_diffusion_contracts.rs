use super::*;

#[test]
fn priority_accounting_reports_structured_drain_before_overflow() {
    let accounting = PriorityRequeueAccounting {
        requeue_count: u64::MAX - 8,
        aged_promotions: 3,
        max_priority_age: 64,
    };
    let recommendation = accounting.drain_recommendation();

    assert!(recommendation.should_drain);
    assert_eq!(
        recommendation.reason,
        PriorityDrainReason::RequeueCounterNearLimit
    );
    assert_eq!(recommendation.requeue_count, u64::MAX - 8);
    assert_eq!(recommendation.aged_promotions, 3);
    assert_eq!(recommendation.max_priority_age, 64);
    assert_eq!(recommendation.requeue_counter_headroom, 8);
    assert_eq!(
        recommendation.aged_promotion_counter_headroom,
        u64::MAX - 3
    );
    assert_eq!(recommendation.fix, PRIORITY_COUNTER_DRAIN_FIX);
}

#[test]
fn priority_accounting_reports_no_drain_for_empty_counters() {
    let recommendation = PriorityRequeueAccounting::default().drain_recommendation();

    assert!(!recommendation.should_drain);
    assert_eq!(recommendation.reason, PriorityDrainReason::None);
    assert_eq!(recommendation.requeue_count, 0);
    assert_eq!(recommendation.aged_promotions, 0);
    assert_eq!(recommendation.max_priority_age, 0);
    assert_eq!(recommendation.requeue_counter_headroom, u64::MAX);
    assert_eq!(recommendation.aged_promotion_counter_headroom, u64::MAX);
    assert_eq!(recommendation.fix, PRIORITY_COUNTER_DRAIN_FIX);
}

#[test]
fn diffuse_priority_mismatched_restrictions_preserve_input_shape() {
    let input = [3.0, 1.0, 2.0];
    let restrictions = [1.0, 0.5];
    let mut out = Vec::with_capacity(input.len());
    let mut scratch = Vec::with_capacity(input.len());

    diffuse_priority_across_siblings_into(&input, &restrictions, 0.5, 4, &mut out, &mut scratch);

    assert_eq!(out, input);
    assert!(scratch.is_empty());
    assert_eq!(out.capacity(), input.len());
}

#[test]
fn diffuse_priority_reuses_exact_scratch_capacity() {
    let input = [4.0, 2.0, 1.0];
    let restrictions = [1.0, 1.0, 1.0];
    let mut out = Vec::with_capacity(input.len());
    let mut scratch = Vec::with_capacity(input.len());
    let out_ptr = out.as_ptr();
    let scratch_ptr = scratch.as_ptr();

    diffuse_priority_across_siblings_into(&input, &restrictions, 0.25, 2, &mut out, &mut scratch);

    assert_eq!(out.len(), input.len());
    assert_eq!(scratch.len(), input.len());
    assert_eq!(out.capacity(), input.len());
    assert_eq!(scratch.capacity(), input.len());
    assert_eq!(out.as_ptr(), out_ptr);
    assert_eq!(scratch.as_ptr(), scratch_ptr);
}
