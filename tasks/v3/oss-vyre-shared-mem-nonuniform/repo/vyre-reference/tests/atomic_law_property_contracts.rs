//! Generated algebraic-law coverage for atomic oracle operations.

use proptest::prelude::*;
use vyre_reference::atomics;

proptest! {
    #[test]
    fn generated_bitwise_atomic_updates_match_commutative_bitwise_laws(left in any::<u32>(), right in any::<u32>()) {
        prop_assert_eq!(atomics::atomic_or(left, right).1, atomics::atomic_or(right, left).1);
        prop_assert_eq!(atomics::atomic_and(left, right).1, atomics::atomic_and(right, left).1);
        prop_assert_eq!(atomics::atomic_xor(left, right).1, atomics::atomic_xor(right, left).1);
    }

    #[test]
    fn generated_idempotent_atomic_updates_leave_value_stable(value in any::<u32>()) {
        prop_assert_eq!(atomics::atomic_or(value, value).1, value);
        prop_assert_eq!(atomics::atomic_and(value, value).1, value);
        prop_assert_eq!(atomics::atomic_min(value, value).1, value);
        prop_assert_eq!(atomics::atomic_max(value, value).1, value);
        prop_assert_eq!(atomics::atomic_lru_update(value, value).1, value);
    }

    #[test]
    fn generated_atomic_identity_updates_preserve_value(value in any::<u32>()) {
        prop_assert_eq!(atomics::atomic_add(value, 0).1, value);
        prop_assert_eq!(atomics::atomic_or(value, 0).1, value);
        prop_assert_eq!(atomics::atomic_xor(value, 0).1, value);
        prop_assert_eq!(atomics::atomic_and(value, u32::MAX).1, value);
    }
}
