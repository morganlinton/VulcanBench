//! Generated property coverage for sequential atomic oracle semantics.

use proptest::prelude::*;
use vyre::ir::AtomicOp;
use vyre_reference::atomics;

proptest! {
    #[test]
    fn generated_atomic_binary_ops_return_old_and_expected_new_value(old in any::<u32>(), value in any::<u32>()) {
        let cases = [
            (AtomicOp::Add, old.wrapping_add(value)),
            (AtomicOp::Or, old | value),
            (AtomicOp::And, old & value),
            (AtomicOp::Xor, old ^ value),
            (AtomicOp::Min, old.min(value)),
            (AtomicOp::Max, old.max(value)),
            (AtomicOp::Exchange, value),
            (AtomicOp::LruUpdate, old.max(value)),
        ];

        for (op, expected_new) in cases {
            let (observed_old, observed_new) = atomics::apply(op, old, None, value)
                .expect("Fix: supported atomic op must have sequential reference semantics");
            prop_assert_eq!(observed_old, old);
            prop_assert_eq!(
                observed_new,
                expected_new,
                "Fix: atomic {:?} produced wrong new value",
                op
            );
        }
    }

    #[test]
    fn generated_compare_exchange_updates_only_on_expected_match(
        old in any::<u32>(),
        expected in any::<u32>(),
        value in any::<u32>(),
    ) {
        let (observed_old, observed_new) = atomics::apply(
            AtomicOp::CompareExchange,
            old,
            Some(expected),
            value,
        )
        .expect("Fix: compare-exchange with expected value must be supported");

        prop_assert_eq!(observed_old, old);
        prop_assert_eq!(observed_new, if old == expected { value } else { old });
    }

    #[test]
    fn generated_compare_exchange_without_expected_is_rejected(old in any::<u32>(), value in any::<u32>()) {
        let err = atomics::apply(AtomicOp::CompareExchange, old, None, value)
            .expect_err("Fix: compare-exchange without expected value must be rejected");
        prop_assert!(
            err.to_string().contains("expected value"),
            "Fix: compare-exchange rejection must explain the missing expected value, got {}",
            err
        );
    }
}
