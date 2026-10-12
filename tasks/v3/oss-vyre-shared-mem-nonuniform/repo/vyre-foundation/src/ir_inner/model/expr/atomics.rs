use crate::ir_inner::model::expr::{Expr, Ident};
use crate::ir_inner::model::types::AtomicOp;
use crate::memory_model::MemoryOrdering;

impl Expr {
    /// Atomic-add builder: `buffer[index] = buffer[index].wrapping_add(value)`.
    #[must_use]
    pub fn atomic_add(buffer: &str, index: Expr, value: Expr) -> Expr {
        Self::atomic_add_ordered(buffer, index, value, MemoryOrdering::default())
    }

    /// Atomic-add builder with explicit memory ordering.
    #[must_use]
    pub fn atomic_add_ordered(
        buffer: &str,
        index: Expr,
        value: Expr,
        ordering: MemoryOrdering,
    ) -> Expr {
        atomic(buffer, AtomicOp::Add, index, None, value, ordering)
    }

    /// Atomic bitwise OR builder.
    #[must_use]
    pub fn atomic_or(buffer: &str, index: Expr, value: Expr) -> Expr {
        atomic(
            buffer,
            AtomicOp::Or,
            index,
            None,
            value,
            MemoryOrdering::default(),
        )
    }

    /// Atomic bitwise AND builder.
    #[must_use]
    pub fn atomic_and(buffer: &str, index: Expr, value: Expr) -> Expr {
        atomic(
            buffer,
            AtomicOp::And,
            index,
            None,
            value,
            MemoryOrdering::default(),
        )
    }

    /// Atomic bitwise XOR builder.
    #[must_use]
    pub fn atomic_xor(buffer: &str, index: Expr, value: Expr) -> Expr {
        atomic(
            buffer,
            AtomicOp::Xor,
            index,
            None,
            value,
            MemoryOrdering::default(),
        )
    }

    /// Atomic unsigned-min builder.
    #[must_use]
    pub fn atomic_min(buffer: &str, index: Expr, value: Expr) -> Expr {
        atomic(
            buffer,
            AtomicOp::Min,
            index,
            None,
            value,
            MemoryOrdering::default(),
        )
    }

    /// Atomic unsigned-max builder.
    #[must_use]
    pub fn atomic_max(buffer: &str, index: Expr, value: Expr) -> Expr {
        atomic(
            buffer,
            AtomicOp::Max,
            index,
            None,
            value,
            MemoryOrdering::default(),
        )
    }

    /// Atomic exchange builder: swap `buffer[index]` with `value`.
    #[must_use]
    pub fn atomic_exchange(buffer: &str, index: Expr, value: Expr) -> Expr {
        atomic(
            buffer,
            AtomicOp::Exchange,
            index,
            None,
            value,
            MemoryOrdering::default(),
        )
    }

    /// Atomic compare-exchange builder.
    ///
    /// Writes `new_value` into `buffer[index]` iff the current value equals
    /// `expected`; returns the previous value in either case.
    #[must_use]
    pub fn atomic_compare_exchange(
        buffer: &str,
        index: Expr,
        expected: Expr,
        new_value: Expr,
    ) -> Expr {
        Self::atomic_compare_exchange_ordered(
            buffer,
            index,
            expected,
            new_value,
            MemoryOrdering::default(),
        )
    }

    /// Atomic compare-exchange builder with explicit memory ordering.
    #[must_use]
    pub fn atomic_compare_exchange_ordered(
        buffer: &str,
        index: Expr,
        expected: Expr,
        new_value: Expr,
        ordering: MemoryOrdering,
    ) -> Expr {
        atomic(
            buffer,
            AtomicOp::CompareExchange,
            index,
            Some(expected),
            new_value,
            ordering,
        )
    }
}

fn atomic(
    buffer: &str,
    op: AtomicOp,
    index: Expr,
    expected: Option<Expr>,
    value: Expr,
    ordering: MemoryOrdering,
) -> Expr {
    Expr::Atomic {
        op,
        buffer: Ident::from(buffer),
        index: Box::new(index),
        expected: expected.map(Box::new),
        value: Box::new(value),
        ordering,
    }
}
