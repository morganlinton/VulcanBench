//! Contract tests for the counterexample minimizer.
//!
//! Invariants: monotonic shrinking, termination, and convergence.

use vyre_conform_generate::CounterexampleMinimizer;

#[test]
fn shrink_is_monotonic_in_value() {
    // For the u32 shrinker, "monotonic" means the minimized value is
    // always <= the original failing witness.
    let original = 1_000_000u32;
    let min = CounterexampleMinimizer::shrink_u32(original, |v| v >= 42);
    assert!(
        min <= original,
        "shrunk value {min} must be <= original {original}"
    );
}

#[test]
fn shrink_is_monotonic_for_all_fail() {
    // When every value fails, the shrinker bottoms out at 0.
    let original = 500u32;
    let min = CounterexampleMinimizer::shrink_u32(original, |_| true);
    assert!(
        min <= original,
        "shrunk value {min} must be <= original {original}"
    );
    assert_eq!(min, 0);
}

#[test]
fn shrink_terminates_on_large_input() {
    // O(log n) termination: even for u32::MAX the shrinker must finish
    // instantly (the binary search loop executes at most 32 iterations).
    let start = std::time::Instant::now();
    let min = CounterexampleMinimizer::shrink_u32(u32::MAX, |v| v >= 1);
    let elapsed = start.elapsed();
    assert_eq!(min, 1);
    assert!(
        elapsed.as_millis() < 100,
        "shrinker must terminate in < 100 ms, took {:?}",
        elapsed
    );
}

#[test]
fn shrink_terminates_at_boundary() {
    let start = std::time::Instant::now();
    let min = CounterexampleMinimizer::shrink_u32(1_000_000, |v| v >= 999_999);
    let elapsed = start.elapsed();
    assert_eq!(min, 999_999);
    assert!(
        elapsed.as_millis() < 100,
        "shrinker must terminate in < 100 ms, took {:?}",
        elapsed
    );
}

#[test]
fn shrink_converges_when_minimal() {
    // Once the counterexample cannot be shrunk further, repeated calls
    // with the same minimized value must return the same value.
    let predicate = |v: u32| v == 7;
    let first = CounterexampleMinimizer::shrink_u32(7, predicate);
    assert_eq!(first, 7);

    // Shrinking the already-minimal value again must stay at 7.
    let second = CounterexampleMinimizer::shrink_u32(first, predicate);
    assert_eq!(
        second, 7,
        "convergence failed: second shrink changed the value"
    );
}

#[test]
fn shrink_converges_after_multi_step_reduction() {
    // Start large, shrink to boundary, then verify idempotence.
    let predicate = |v: u32| v >= 100;
    let reduced = CounterexampleMinimizer::shrink_u32(1_000_000, predicate);
    assert_eq!(reduced, 100);

    let again = CounterexampleMinimizer::shrink_u32(reduced, predicate);
    assert_eq!(again, 100, "convergence failed after multi-step reduction");
}

#[test]
fn shrink_converges_to_zero_for_universal_predicate() {
    // When every value satisfies the predicate, the minimizer bottoms
    // out at 0 and stays there.
    let predicate = |_| true;
    let reduced = CounterexampleMinimizer::shrink_u32(10_000, predicate);
    assert_eq!(reduced, 0);

    let again = CounterexampleMinimizer::shrink_u32(reduced, predicate);
    assert_eq!(again, 0, "convergence failed at zero boundary");
}
