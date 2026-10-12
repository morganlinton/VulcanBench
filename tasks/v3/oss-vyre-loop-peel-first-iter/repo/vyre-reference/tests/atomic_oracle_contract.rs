//! External contracts for reference atomic-operation semantics.
//!
//! Atomic operations are parity-critical because GPU backends differ in
//! return-value conventions and memory-order behavior. The reference oracle
//! must define old-value return semantics and the exact stored value after
//! every supported operation.

use vyre::ir::AtomicOp;
use vyre_reference::atomics;

#[test]
fn supported_atomic_ops_return_old_value_and_exact_new_value() {
    let cases = [
        (AtomicOp::Add, 0xffff_fffe, None, 5, 0xffff_fffe, 3),
        (AtomicOp::Or, 0b1010, None, 0b0101, 0b1010, 0b1111),
        (AtomicOp::And, 0b1010, None, 0b0110, 0b1010, 0b0010),
        (AtomicOp::Xor, 0b1010, None, 0b0110, 0b1010, 0b1100),
        (AtomicOp::Min, 99, None, 7, 99, 7),
        (AtomicOp::Max, 99, None, 777, 99, 777),
        (AtomicOp::Exchange, 123, None, 456, 123, 456),
        (AtomicOp::LruUpdate, 40, None, 41, 40, 41),
        (AtomicOp::LruUpdate, 40, None, 39, 40, 40),
    ];

    for (op, old, expected, value, returned_old, stored_new) in cases {
        assert_eq!(
            atomics::apply(op, old, expected, value).expect("supported atomic op must execute"),
            (returned_old, stored_new),
            "{op:?} must return the pre-operation value and store the exact oracle value"
        );
    }
}

#[test]
fn compare_exchange_updates_only_on_expected_match() {
    assert_eq!(
        atomics::apply(AtomicOp::CompareExchange, 0xaaaa, Some(0xaaaa), 0xbbbb)
            .expect("matching compare-exchange must execute"),
        (0xaaaa, 0xbbbb)
    );
    assert_eq!(
        atomics::apply(AtomicOp::CompareExchange, 0xaaaa, Some(0xcccc), 0xbbbb)
            .expect("nonmatching compare-exchange must execute"),
        (0xaaaa, 0xaaaa)
    );
}

#[test]
fn compare_exchange_without_expected_is_rejected_with_actionable_error() {
    let error = atomics::apply(AtomicOp::CompareExchange, 1, None, 2)
        .expect_err("compare-exchange without expected value must be rejected");
    let message = error.to_string();
    assert!(
        message.contains("compare-exchange") && message.contains("expected"),
        "compare-exchange diagnostic must explain the missing expected value, got: {message}"
    );
}

#[test]
fn unsupported_atomic_ops_are_rejected_instead_of_faked() {
    for op in [AtomicOp::CompareExchangeWeak, AtomicOp::FetchNand] {
        let error = atomics::apply(op, 1, Some(1), 2)
            .expect_err("unsupported atomic op must not silently emulate fake semantics");
        let message = error.to_string();
        assert!(
            message.contains("unsupported atomic op") && message.contains("sequential semantics"),
            "unsupported op diagnostic must require explicit semantics, got: {message}"
        );
    }
}
