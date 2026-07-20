//! Backend-neutral checked arithmetic and atomic accounting primitives.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::BackendError;

/// Add two `u64` values without wraparound.
pub fn checked_add_u64_value<E>(lhs: u64, rhs: u64, error: E) -> Result<u64, E> {
    lhs.checked_add(rhs).ok_or(error)
}

/// Add two `u64` values without wraparound, constructing the error lazily.
///
/// # Errors
///
/// Returns `E` from `error` when the addition would overflow.
pub fn checked_add_u64_lazy<E>(lhs: u64, rhs: u64, error: impl FnOnce() -> E) -> Result<u64, E> {
    lhs.checked_add(rhs).ok_or_else(error)
}

/// Multiply two `u64` values without wraparound.
pub fn checked_mul_u64_value<E>(lhs: u64, rhs: u64, error: E) -> Result<u64, E> {
    lhs.checked_mul(rhs).ok_or(error)
}

/// Multiply two `u64` values without wraparound, constructing the error lazily.
///
/// # Errors
///
/// Returns `E` from `error` when the multiplication would overflow.
pub fn checked_mul_u64_lazy<E>(lhs: u64, rhs: u64, error: impl FnOnce() -> E) -> Result<u64, E> {
    lhs.checked_mul(rhs).ok_or_else(error)
}

/// Subtract two `u64` values without underflow.
pub fn checked_sub_u64_value<E>(lhs: u64, rhs: u64, error: E) -> Result<u64, E> {
    lhs.checked_sub(rhs).ok_or(error)
}

/// Subtract two `u64` values without underflow, constructing the error lazily.
///
/// # Errors
///
/// Returns `E` from `error` when the subtraction would underflow.
pub fn checked_sub_u64_lazy<E>(lhs: u64, rhs: u64, error: impl FnOnce() -> E) -> Result<u64, E> {
    lhs.checked_sub(rhs).ok_or_else(error)
}

/// Subtract two `usize` values without underflow, constructing the error lazily.
///
/// # Errors
///
/// Returns `E` from `error` when the subtraction would underflow.
pub fn checked_sub_usize_lazy<E>(
    lhs: usize,
    rhs: usize,
    error: impl FnOnce() -> E,
) -> Result<usize, E> {
    lhs.checked_sub(rhs).ok_or_else(error)
}

/// Add two `usize` values without wraparound.
pub fn checked_add_usize_value<E>(lhs: usize, rhs: usize, error: E) -> Result<usize, E> {
    lhs.checked_add(rhs).ok_or(error)
}

/// Add two `usize` values without wraparound, constructing the error lazily.
///
/// # Errors
///
/// Returns `E` from `error` when the addition would overflow.
pub fn checked_add_usize_lazy<E>(
    lhs: usize,
    rhs: usize,
    error: impl FnOnce() -> E,
) -> Result<usize, E> {
    lhs.checked_add(rhs).ok_or_else(error)
}

/// Multiply two `usize` values without wraparound, constructing the error lazily.
///
/// # Errors
///
/// Returns `E` from `error` when the multiplication would overflow.
pub fn checked_mul_usize_lazy<E>(
    lhs: usize,
    rhs: usize,
    error: impl FnOnce() -> E,
) -> Result<usize, E> {
    lhs.checked_mul(rhs).ok_or_else(error)
}

/// Convert `usize` to `u64`, constructing the error lazily on overflow.
///
/// # Errors
///
/// Returns `E` from `error` when `value` cannot fit in `u64`.
pub fn checked_usize_to_u64_lazy<E>(value: usize, error: impl FnOnce() -> E) -> Result<u64, E> {
    u64::try_from(value).map_err(|_| error())
}

/// Validate a `usize` byte range and return its exclusive end.
///
/// # Errors
///
/// Returns `E` from `overflow_error` when `start + len` would overflow, or
/// `E` from `out_of_bounds_error` when the range end exceeds `limit`.
pub fn checked_usize_byte_range_end_lazy<E>(
    start: usize,
    len: usize,
    limit: usize,
    overflow_error: impl FnOnce() -> E,
    out_of_bounds_error: impl FnOnce(usize) -> E,
) -> Result<usize, E> {
    let end = start.checked_add(len).ok_or_else(overflow_error)?;
    if end > limit {
        return Err(out_of_bounds_error(end));
    }
    Ok(end)
}

/// Add a `usize` byte offset to a `u64` base pointer/counter without wraparound.
///
/// # Errors
///
/// Returns `E` from `conversion_error` when `offset` cannot be represented as
/// `u64`, or `E` from `overflow_error` when `base + offset` would overflow.
pub fn checked_add_u64_usize_offset_lazy<E>(
    base: u64,
    offset: usize,
    conversion_error: impl FnOnce() -> E,
    overflow_error: impl FnOnce() -> E,
) -> Result<u64, E> {
    let offset = u64::try_from(offset).map_err(|_| conversion_error())?;
    base.checked_add(offset).ok_or_else(overflow_error)
}

#[cfg(test)]
mod byte_range_accounting_tests {
    use std::cell::Cell;

    use super::{
        checked_add_u64_usize_offset_lazy, checked_mul_u32_value, checked_mul_u64_lazy,
        checked_sub_u64_lazy, checked_sub_usize_lazy, checked_usize_byte_range_end_lazy,
        checked_usize_to_u64_lazy,
    };

    #[test]
    fn checked_mul_u64_lazy_is_lazy_on_success() {
        let overflow_called = Cell::new(false);

        let value = checked_mul_u64_lazy(8, 4, || {
            overflow_called.set(true);
            "overflow"
        });

        assert_eq!(value, Ok(32));
        assert!(!overflow_called.get());
    }

    #[test]
    fn checked_mul_u64_lazy_reports_overflow() {
        let value = checked_mul_u64_lazy(u64::MAX, 2, || "overflow");

        assert_eq!(value, Err("overflow"));
    }

    #[test]
    fn checked_mul_u32_value_multiplies_without_wraparound() {
        let value = checked_mul_u32_value(128, 8, "overflow");

        assert_eq!(value, Ok(1024));
    }

    #[test]
    fn checked_mul_u32_value_reports_overflow() {
        let value = checked_mul_u32_value(u32::MAX, 2, "overflow");

        assert_eq!(value, Err("overflow"));
    }

    #[test]
    fn checked_sub_u64_lazy_reports_underflow() {
        let value = checked_sub_u64_lazy(1, 2, || "underflow");

        assert_eq!(value, Err("underflow"));
    }

    #[test]
    fn checked_sub_usize_lazy_reports_underflow() {
        let value = checked_sub_usize_lazy(4, 8, || "underflow");

        assert_eq!(value, Err("underflow"));
    }

    #[test]
    fn checked_usize_to_u64_lazy_converts_host_width() {
        let value = checked_usize_to_u64_lazy(64, || "overflow");

        assert_eq!(value, Ok(64));
    }

    #[test]
    fn checked_usize_byte_range_end_lazy_is_lazy_on_success() {
        let overflow_called = Cell::new(false);
        let bounds_called = Cell::new(false);

        let end = checked_usize_byte_range_end_lazy(
            8,
            4,
            16,
            || {
                overflow_called.set(true);
                "overflow"
            },
            |_| {
                bounds_called.set(true);
                "bounds"
            },
        );

        assert_eq!(end, Ok(12));
        assert!(!overflow_called.get());
        assert!(!bounds_called.get());
    }

    #[test]
    fn checked_usize_byte_range_end_lazy_passes_computed_end_to_bounds_error() {
        let end = checked_usize_byte_range_end_lazy(8, 5, 12, || usize::MAX, |end| end);

        assert_eq!(end, Err(13));
    }

    #[test]
    fn checked_add_u64_usize_offset_lazy_is_lazy_on_success() {
        let conversion_called = Cell::new(false);
        let overflow_called = Cell::new(false);

        let value = checked_add_u64_usize_offset_lazy(
            64,
            8,
            || {
                conversion_called.set(true);
                "conversion"
            },
            || {
                overflow_called.set(true);
                "overflow"
            },
        );

        assert_eq!(value, Ok(72));
        assert!(!conversion_called.get());
        assert!(!overflow_called.get());
    }

    #[test]
    fn checked_add_u64_usize_offset_lazy_reports_pointer_overflow() {
        let value = checked_add_u64_usize_offset_lazy(u64::MAX, 1, || "conversion", || "overflow");

        assert_eq!(value, Err("overflow"));
    }
}

/// Add two `u32` values without wraparound.
pub fn checked_add_u32_value<E>(lhs: u32, rhs: u32, error: E) -> Result<u32, E> {
    lhs.checked_add(rhs).ok_or(error)
}

/// Multiply two `u32` values without wraparound.
pub fn checked_mul_u32_value<E>(lhs: u32, rhs: u32, error: E) -> Result<u32, E> {
    lhs.checked_mul(rhs).ok_or(error)
}

/// Domain error adapter for planner-specific arithmetic overflow fields.
pub trait ArithmeticOverflow: Sized {
    /// Build the planner-specific overflow error for `field`.
    fn arithmetic_overflow(field: &'static str) -> Self;
}

/// Add two `u64` counters and map overflow into the caller domain.
///
/// # Errors
///
/// Returns `E` when the addition would overflow.
pub fn checked_add_u64_count<E>(lhs: u64, rhs: u64, field: &'static str) -> Result<u64, E>
where
    E: ArithmeticOverflow,
{
    checked_add_u64_value(lhs, rhs, E::arithmetic_overflow(field))
}

/// Multiply two `u64` counters and map overflow into the caller domain.
///
/// # Errors
///
/// Returns `E` when the multiplication would overflow.
pub fn checked_mul_u64_count<E>(lhs: u64, rhs: u64, field: &'static str) -> Result<u64, E>
where
    E: ArithmeticOverflow,
{
    checked_mul_u64_value(lhs, rhs, E::arithmetic_overflow(field))
}

/// Subtract two `u64` counters and map underflow into the caller domain.
///
/// # Errors
///
/// Returns `E` when the subtraction would underflow.
pub fn checked_sub_u64_count<E>(lhs: u64, rhs: u64, field: &'static str) -> Result<u64, E>
where
    E: ArithmeticOverflow,
{
    checked_sub_u64_value(lhs, rhs, E::arithmetic_overflow(field))
}

/// Add two `usize` counters and map overflow into the caller domain.
///
/// # Errors
///
/// Returns `E` when the addition would overflow.
pub fn checked_add_usize_count<E>(lhs: usize, rhs: usize, field: &'static str) -> Result<usize, E>
where
    E: ArithmeticOverflow,
{
    checked_add_usize_value(lhs, rhs, E::arithmetic_overflow(field))
}

/// Add two `u32` counters and map overflow into the caller domain.
///
/// # Errors
///
/// Returns `E` when the addition would overflow.
pub fn checked_add_u32_count<E>(lhs: u32, rhs: u32, field: &'static str) -> Result<u32, E>
where
    E: ArithmeticOverflow,
{
    checked_add_u32_value(lhs, rhs, E::arithmetic_overflow(field))
}

/// Add `value` to a `u64` counter without allowing wraparound or saturation.
///
/// # Errors
///
/// Returns [`BackendError`] from `overflow` when the addition would overflow.
pub fn checked_atomic_add_u64(
    counter: &AtomicU64,
    value: u64,
    overflow: impl Fn(u64, u64) -> BackendError,
) -> Result<(), BackendError> {
    checked_atomic_add_u64_with_order(
        counter,
        value,
        Ordering::Relaxed,
        Ordering::Relaxed,
        Ordering::Relaxed,
        overflow,
    )
}

/// Add `value` to a `u64` counter with caller-selected atomic orderings.
///
/// # Errors
///
/// Returns `E` from `overflow` when the addition would overflow.
pub fn checked_atomic_add_u64_with_order<E>(
    counter: &AtomicU64,
    value: u64,
    load_order: Ordering,
    success_order: Ordering,
    failure_order: Ordering,
    overflow: impl Fn(u64, u64) -> E,
) -> Result<(), E> {
    checked_atomic_add_u64_guarded_with_order(
        counter,
        value,
        load_order,
        success_order,
        failure_order,
        overflow,
        |_| Ok(()),
    )
}

/// Add `value` to a `u64` counter with overflow checking and a pre-CAS next-value guard.
///
/// # Errors
///
/// Returns `E` from `overflow` when the addition would overflow, or from
/// `validate_next` when the computed next value violates a caller invariant.
pub fn checked_atomic_add_u64_guarded_with_order<E>(
    counter: &AtomicU64,
    value: u64,
    load_order: Ordering,
    success_order: Ordering,
    failure_order: Ordering,
    overflow: impl Fn(u64, u64) -> E,
    mut validate_next: impl FnMut(u64) -> Result<(), E>,
) -> Result<(), E> {
    let mut observed = counter.load(load_order);
    loop {
        let next = observed
            .checked_add(value)
            .ok_or_else(|| overflow(observed, value))?;
        validate_next(next)?;
        match counter.compare_exchange_weak(observed, next, success_order, failure_order) {
            Ok(_) => return Ok(()),
            Err(actual) => observed = actual,
        }
    }
}

/// Add `value` to a `usize` counter without allowing wraparound.
///
/// # Errors
///
/// Returns [`BackendError`] from `overflow` when the addition would overflow.
pub fn checked_atomic_add_usize(
    counter: &AtomicUsize,
    value: usize,
    overflow: impl Fn(usize, usize) -> BackendError,
) -> Result<(), BackendError> {
    checked_atomic_add_usize_with_order(
        counter,
        value,
        Ordering::Acquire,
        Ordering::AcqRel,
        Ordering::Acquire,
        overflow,
    )
}

/// Add `value` to a `usize` counter with caller-selected atomic orderings.
///
/// # Errors
///
/// Returns `E` from `overflow` when the addition would overflow.
pub fn checked_atomic_add_usize_with_order<E>(
    counter: &AtomicUsize,
    value: usize,
    load_order: Ordering,
    success_order: Ordering,
    failure_order: Ordering,
    overflow: impl Fn(usize, usize) -> E,
) -> Result<(), E> {
    checked_atomic_add_usize_guarded_with_order(
        counter,
        value,
        load_order,
        success_order,
        failure_order,
        overflow,
        |_| Ok(()),
    )
}

/// Add `value` to a `usize` counter with overflow checking and a pre-CAS next-value guard.
///
/// # Errors
///
/// Returns `E` from `overflow` when the addition would overflow, or from
/// `validate_next` when the computed next value violates a caller invariant.

pub fn checked_atomic_add_usize_guarded_with_order<E>(
    counter: &AtomicUsize,
    value: usize,
    load_order: Ordering,
    success_order: Ordering,
    failure_order: Ordering,
    overflow: impl Fn(usize, usize) -> E,
    mut validate_next: impl FnMut(usize) -> Result<(), E>,
) -> Result<(), E> {
    let mut observed = counter.load(load_order);
    loop {
        let next = observed
            .checked_add(value)
            .ok_or_else(|| overflow(observed, value))?;
        validate_next(next)?;
        match counter.compare_exchange_weak(observed, next, success_order, failure_order) {
            Ok(_) => return Ok(()),
            Err(actual) => observed = actual,
        }
    }
}

/// Subtract `value` from a `u64` counter without allowing underflow.
///
/// # Errors
///
/// Returns [`BackendError`] from `underflow` when the subtraction would underflow.
pub fn checked_atomic_sub_u64(
    counter: &AtomicU64,
    value: u64,
    underflow: impl Fn(u64, u64) -> BackendError,
) -> Result<(), BackendError> {
    checked_atomic_sub_u64_with_order(
        counter,
        value,
        Ordering::Acquire,
        Ordering::AcqRel,
        Ordering::Acquire,
        underflow,
    )
}

/// Subtract `value` from a `u64` counter with caller-selected atomic orderings.
///
/// # Errors
///
/// Returns `E` from `underflow` when the subtraction would underflow.
pub fn checked_atomic_sub_u64_with_order<E>(
    counter: &AtomicU64,
    value: u64,
    load_order: Ordering,
    success_order: Ordering,
    failure_order: Ordering,
    underflow: impl Fn(u64, u64) -> E,
) -> Result<(), E> {
    if value == 0 {
        return Ok(());
    }
    let mut observed = counter.load(load_order);
    loop {
        let next = observed
            .checked_sub(value)
            .ok_or_else(|| underflow(observed, value))?;
        match counter.compare_exchange_weak(observed, next, success_order, failure_order) {
            Ok(_) => return Ok(()),
            Err(actual) => observed = actual,
        }
    }
}

/// Subtract `value` from a `usize` counter without allowing underflow.
///
/// # Errors
///
/// Returns [`BackendError`] from `underflow` when the subtraction would underflow.
pub fn checked_atomic_sub_usize(
    counter: &AtomicUsize,
    value: usize,
    underflow: impl Fn(usize, usize) -> BackendError,
) -> Result<(), BackendError> {
    checked_atomic_sub_usize_with_order(
        counter,
        value,
        Ordering::Acquire,
        Ordering::AcqRel,
        Ordering::Acquire,
        underflow,
    )
}

/// Subtract `value` from a `usize` counter with caller-selected atomic orderings.
///
/// # Errors
///
/// Returns `E` from `underflow` when the subtraction would underflow.
pub fn checked_atomic_sub_usize_with_order<E>(
    counter: &AtomicUsize,
    value: usize,
    load_order: Ordering,
    success_order: Ordering,
    failure_order: Ordering,
    underflow: impl Fn(usize, usize) -> E,
) -> Result<(), E> {
    if value == 0 {
        return Ok(());
    }
    let mut observed = counter.load(load_order);
    loop {
        let next = observed
            .checked_sub(value)
            .ok_or_else(|| underflow(observed, value))?;
        match counter.compare_exchange_weak(observed, next, success_order, failure_order) {
            Ok(_) => return Ok(()),
            Err(actual) => observed = actual,
        }
    }
}

/// Apply a checked update to a `u64` atomic counter with caller-selected
/// orderings.
///
/// Returns the value observed before the successful publish. `update` receives
/// each observed value and must return the next value to publish. `on_retry`
/// runs after a failed CAS and may abort the update by returning `Err`.
pub fn checked_atomic_update_u64_with_order<E>(
    counter: &AtomicU64,
    load_order: Ordering,
    success_order: Ordering,
    failure_order: Ordering,
    mut update: impl FnMut(u64) -> Result<u64, E>,
    mut on_retry: impl FnMut(u64, u64) -> Result<(), E>,
) -> Result<u64, E> {
    let mut observed = counter.load(load_order);
    loop {
        let next = update(observed)?;
        match counter.compare_exchange_weak(observed, next, success_order, failure_order) {
            Ok(previous) => return Ok(previous),
            Err(actual) => {
                on_retry(observed, actual)?;
                observed = actual;
            }
        }
    }
}

/// Apply a checked update to a `u32` atomic counter with caller-selected
/// orderings.
///
/// Returns the value observed before the successful publish. This keeps
/// bounded sequence allocators from copying CAS retry loops into consumers.
pub fn checked_atomic_update_u32_with_order<E>(
    counter: &AtomicU32,
    load_order: Ordering,
    success_order: Ordering,
    failure_order: Ordering,
    mut update: impl FnMut(u32) -> Result<u32, E>,
    mut on_retry: impl FnMut(u32, u32) -> Result<(), E>,
) -> Result<u32, E> {
    let mut observed = counter.load(load_order);
    loop {
        let next = update(observed)?;
        match counter.compare_exchange_weak(observed, next, success_order, failure_order) {
            Ok(previous) => return Ok(previous),
            Err(actual) => {
                on_retry(observed, actual)?;
                observed = actual;
            }
        }
    }
}

#[cfg(test)]
mod checked_atomic_update_with_order_tests {
    use super::*;

    #[test]
    fn checked_atomic_update_u64_publishes_checked_next_and_returns_observed() {
        let counter = AtomicU64::new(41);

        let previous = checked_atomic_update_u64_with_order(
            &counter,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |observed| observed.checked_add(1).ok_or("overflow"),
            |_, _| Ok(()),
        )
        .expect("Fix: reject accounting updates that overflow the tracked counter range - update should fit");

        assert_eq!(previous, 41);
        assert_eq!(counter.load(Ordering::Acquire), 42);
    }

    #[test]
    fn checked_atomic_update_u32_rejects_without_publishing() {
        let counter = AtomicU32::new(u32::MAX);

        let error = checked_atomic_update_u32_with_order(
            &counter,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |observed| observed.checked_add(1).ok_or("overflow"),
            |_, _| Ok(()),
        )
        .expect_err("overflow should be surfaced");

        assert_eq!(error, "overflow");
        assert_eq!(counter.load(Ordering::Acquire), u32::MAX);
    }
}

/// Subtract `value` from a `usize` counter, repairing underflow to zero.
///
/// This is only for release-path accounting where the caller has already
/// decided that a corrupt counter must be repaired rather than propagated as a
/// dispatch error. `on_repair` is called after a successful repair CAS.
pub fn repair_atomic_sub_usize_with_order(
    counter: &AtomicUsize,
    value: usize,
    load_order: Ordering,
    success_order: Ordering,
    failure_order: Ordering,
    on_repair: impl FnMut(usize, usize),
) {
    let _ = repair_atomic_sub_usize_fetch_with_order(
        counter,
        value,
        load_order,
        success_order,
        failure_order,
        on_repair,
    );
}

/// Subtract `value` from a `usize` counter, repairing underflow to zero and
/// returning the observed value before the successful publish.
///
/// This preserves `fetch_sub`-style previous-value semantics for padded or
/// wrapper counters while keeping the underflow repair policy single-sourced.
pub fn repair_atomic_sub_usize_fetch_with_order(
    counter: &AtomicUsize,
    value: usize,
    load_order: Ordering,
    success_order: Ordering,
    failure_order: Ordering,
    mut on_repair: impl FnMut(usize, usize),
) -> usize {
    if value == 0 {
        return counter.load(load_order);
    }
    let mut observed = counter.load(load_order);
    loop {
        let Some(next) = observed.checked_sub(value) else {
            match counter.compare_exchange_weak(observed, 0, success_order, failure_order) {
                Ok(_) => {
                    on_repair(observed, value);
                    return observed;
                }
                Err(actual) => {
                    observed = actual;
                    continue;
                }
            }
        };
        match counter.compare_exchange_weak(observed, next, success_order, failure_order) {
            Ok(_) => return observed,
            Err(actual) => observed = actual,
        }
    }
}

/// Add `value` to a `usize` atomic counter, pinning it at `usize::MAX` instead
/// of wrapping, and return the observed value before the successful publish.
///
/// `on_pinned` is called exactly once when a successful publish moves a
/// non-pinned counter to `usize::MAX`.
pub fn pinning_atomic_add_usize_with_order(
    counter: &AtomicUsize,
    value: usize,
    success_order: Ordering,
    failure_order: Ordering,
    on_pinned: impl FnOnce(usize, usize),
) -> usize {
    if value == 0 {
        return counter.load(failure_order);
    }
    let mut current = counter.load(failure_order);
    loop {
        let next = current.checked_add(value).unwrap_or(usize::MAX);
        match counter.compare_exchange_weak(current, next, success_order, failure_order) {
            Ok(previous) => {
                if next == usize::MAX && previous != usize::MAX {
                    on_pinned(previous, value);
                }
                return previous;
            }
            Err(observed) => current = observed,
        }
    }
}

#[cfg(test)]
mod pinning_atomic_add_usize_with_order_tests {
    use super::*;

    #[test]
    fn pinning_atomic_add_usize_pins_without_wrapping_and_returns_previous() {
        let counter = AtomicUsize::new(usize::MAX - 1);
        let mut pinned = None;

        let previous = pinning_atomic_add_usize_with_order(
            &counter,
            2,
            Ordering::AcqRel,
            Ordering::Acquire,
            |observed, value| pinned = Some((observed, value)),
        );

        assert_eq!(previous, usize::MAX - 1);
        assert_eq!(counter.load(Ordering::Acquire), usize::MAX);
        assert_eq!(pinned, Some((usize::MAX - 1, 2)));

        let mut called_again = false;
        let previous = pinning_atomic_add_usize_with_order(
            &counter,
            1,
            Ordering::AcqRel,
            Ordering::Acquire,
            |_, _| called_again = true,
        );

        assert_eq!(previous, usize::MAX);
        assert_eq!(counter.load(Ordering::Acquire), usize::MAX);
        assert!(!called_again);
    }

    #[test]
    fn repair_atomic_sub_usize_fetch_repairs_and_returns_observed() {
        let counter = AtomicUsize::new(3);
        let mut repair = None;

        let previous = repair_atomic_sub_usize_fetch_with_order(
            &counter,
            5,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |observed, value| repair = Some((observed, value)),
        );

        assert_eq!(previous, 3);
        assert_eq!(counter.load(Ordering::Acquire), 0);
        assert_eq!(repair, Some((3, 5)));
    }
}

/// Increment a `u64` atomic counter, pinning it at `u64::MAX` instead of wrapping.
///
/// Returns `true` when the counter was incremented and `false` when it was
/// already pinned. `on_pinned` is called exactly once on the pinned path.
pub fn pinning_atomic_increment_u64(
    counter: &AtomicU64,
    success_order: Ordering,
    failure_order: Ordering,
    on_pinned: impl FnOnce(),
) -> bool {
    let mut current = counter.load(failure_order);
    loop {
        let Some(next) = current.checked_add(1) else {
            on_pinned();
            return false;
        };
        match counter.compare_exchange_weak(current, next, success_order, failure_order) {
            Ok(_) => return true,
            Err(observed) => current = observed,
        }
    }
}

/// Increment a `u32` atomic counter, pinning it at `u32::MAX` instead of wrapping.
///
/// Returns `true` when the counter was incremented and `false` when it was
/// already pinned. `on_pinned` is called exactly once on the pinned path.
pub fn pinning_atomic_increment_u32(
    counter: &AtomicU32,
    success_order: Ordering,
    failure_order: Ordering,
    on_pinned: impl FnOnce(),
) -> bool {
    let mut current = counter.load(failure_order);
    loop {
        let Some(next) = current.checked_add(1) else {
            on_pinned();
            return false;
        };
        match counter.compare_exchange_weak(current, next, success_order, failure_order) {
            Ok(_) => return true,
            Err(observed) => current = observed,
        }
    }
}

/// Allocate the current `u64` atomic sequence value and publish the next value.
///
/// When incrementing would overflow, publishes `rebase_to` instead of wrapping.
/// Returns the allocated value observed before the publish. `on_rebase` is
/// called exactly once for each successful overflow rebase.
pub fn rebasing_atomic_next_u64(
    counter: &AtomicU64,
    rebase_to: u64,
    load_order: Ordering,
    success_order: Ordering,
    failure_order: Ordering,
    mut on_rebase: impl FnMut(u64, u64),
) -> u64 {
    let mut observed = counter.load(load_order);
    loop {
        let next = match observed.checked_add(1) {
            Some(next) => next,
            None => rebase_to,
        };
        match counter.compare_exchange_weak(observed, next, success_order, failure_order) {
            Ok(_) => {
                if next == rebase_to && observed == u64::MAX {
                    on_rebase(observed, rebase_to);
                }
                return observed;
            }
            Err(actual) => observed = actual,
        }
    }
}

/// Allocate the current `u64` atomic sequence value and publish `current + 1`.
///
/// # Errors
///
/// Returns `E` from `overflow` when the sequence cannot advance without
/// wrapping.
pub fn checked_atomic_next_u64_with_order<E>(
    counter: &AtomicU64,
    load_order: Ordering,
    success_order: Ordering,
    failure_order: Ordering,
    overflow: impl Fn(u64) -> E,
) -> Result<u64, E> {
    let mut observed = counter.load(load_order);
    loop {
        let next = observed.checked_add(1).ok_or_else(|| overflow(observed))?;
        match counter.compare_exchange_weak(observed, next, success_order, failure_order) {
            Ok(_) => return Ok(observed),
            Err(actual) => observed = actual,
        }
    }
}

/// Raise a `u64` atomic counter to at least `value` using one atomic max update.
///
/// Returns the previous value observed by the atomic operation.

pub fn atomic_max_u64(counter: &AtomicU64, value: u64, order: Ordering) -> u64 {
    counter.fetch_max(value, order)
}

/// Increment a `u64` scalar counter, pinning it at `u64::MAX` instead of wrapping.
///
/// Returns `true` when the counter was incremented and `false` when it was
/// already pinned. `on_pinned` is called exactly once on the pinned path.
pub fn pinning_increment_u64(counter: &mut u64, on_pinned: impl FnOnce()) -> bool {
    match counter.checked_add(1) {
        Some(next) => {
            *counter = next;
            true
        }
        None => {
            on_pinned();
            *counter = u64::MAX;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

    use super::{
        atomic_max_u64, checked_add_u32_count, checked_add_u32_value, checked_add_u64_count,
        checked_add_u64_lazy, checked_add_u64_value, checked_add_usize_count,
        checked_add_usize_lazy, checked_add_usize_value, checked_atomic_add_u64,
        checked_atomic_add_u64_guarded_with_order, checked_atomic_add_u64_with_order,
        checked_atomic_add_usize, checked_atomic_add_usize_guarded_with_order,
        checked_atomic_add_usize_with_order, checked_atomic_next_u64_with_order,
        checked_atomic_sub_u64, checked_atomic_sub_u64_with_order, checked_atomic_sub_usize,
        checked_atomic_sub_usize_with_order, checked_mul_u64_count, checked_mul_u64_value,
        checked_mul_usize_lazy, checked_sub_u64_count, checked_sub_u64_value,
        pinning_atomic_increment_u32, pinning_atomic_increment_u64, pinning_increment_u64,
        rebasing_atomic_next_u64, repair_atomic_sub_usize_with_order, ArithmeticOverflow,
    };

    #[derive(Debug, Eq, PartialEq)]
    enum ArithmeticError {
        Overflow(&'static str),
    }

    impl ArithmeticOverflow for ArithmeticError {
        fn arithmetic_overflow(field: &'static str) -> Self {
            Self::Overflow(field)
        }
    }

    #[test]
    fn checked_value_helpers_preserve_domain_errors() {
        assert_eq!(checked_add_u64_value(2, 3, "overflow"), Ok(5));
        assert_eq!(checked_mul_u64_value(2, 3, "overflow"), Ok(6));
        assert_eq!(checked_sub_u64_value(5, 3, "underflow"), Ok(2));
        assert_eq!(checked_add_usize_value(2, 3, "overflow"), Ok(5));
        assert_eq!(checked_add_u32_value(2, 3, "overflow"), Ok(5));

        assert_eq!(
            checked_add_u64_value(u64::MAX, 1, "overflow"),
            Err("overflow")
        );
        assert_eq!(
            checked_mul_u64_value(u64::MAX, 2, "overflow"),
            Err("overflow")
        );
        assert_eq!(checked_sub_u64_value(0, 1, "underflow"), Err("underflow"));
        assert_eq!(
            checked_add_usize_value(usize::MAX, 1, "overflow"),
            Err("overflow")
        );
        assert_eq!(
            checked_add_u32_value(u32::MAX, 1, "overflow"),
            Err("overflow")
        );
    }

    #[test]
    fn checked_add_usize_lazy_does_not_build_success_error() {
        let mut constructed = false;

        assert_eq!(
            checked_add_usize_lazy(2, 3, || {
                constructed = true;
                "overflow"
            }),
            Ok(5)
        );
        assert!(
            !constructed,
            "Fix: hot-path checked usize accounting must not construct error strings on success."
        );
        assert_eq!(
            checked_add_usize_lazy(usize::MAX, 1, || "overflow"),
            Err("overflow")
        );
    }

    #[test]
    fn checked_add_u64_lazy_does_not_build_success_error() {
        let mut constructed = false;

        assert_eq!(
            checked_add_u64_lazy(2, 3, || {
                constructed = true;
                "overflow"
            }),
            Ok(5)
        );
        assert!(
            !constructed,
            "Fix: hot-path checked u64 accounting must not construct error strings on success."
        );
        assert_eq!(
            checked_add_u64_lazy(u64::MAX, 1, || "overflow"),
            Err("overflow")
        );
    }

    #[test]
    fn checked_mul_usize_lazy_does_not_build_success_error() {
        let mut constructed = false;

        assert_eq!(
            checked_mul_usize_lazy(2, 3, || {
                constructed = true;
                "overflow"
            }),
            Ok(6)
        );
        assert!(
            !constructed,
            "Fix: hot-path checked usize multiplication must not construct error strings on success."
        );
        assert_eq!(
            checked_mul_usize_lazy(usize::MAX, 2, || "overflow"),
            Err("overflow")
        );
    }

    #[test]
    fn typed_checked_arithmetic_helpers_preserve_domain_error_fields() {
        assert_eq!(
            checked_add_u64_count::<ArithmeticError>(u64::MAX, 1, "u64 add"),
            Err(ArithmeticError::Overflow("u64 add"))
        );
        assert_eq!(
            checked_mul_u64_count::<ArithmeticError>(u64::MAX, 2, "u64 mul"),
            Err(ArithmeticError::Overflow("u64 mul"))
        );
        assert_eq!(
            checked_sub_u64_count::<ArithmeticError>(0, 1, "u64 sub"),
            Err(ArithmeticError::Overflow("u64 sub"))
        );
        assert_eq!(
            checked_add_usize_count::<ArithmeticError>(usize::MAX, 1, "usize add"),
            Err(ArithmeticError::Overflow("usize add"))
        );
        assert_eq!(
            checked_add_u32_count::<ArithmeticError>(u32::MAX, 1, "u32 add"),
            Err(ArithmeticError::Overflow("u32 add"))
        );
    }

    #[test]
    fn generated_checked_arithmetic_matrix_matches_primitive_semantics() {
        const VALUES: [u64; 12] = [
            0,
            1,
            2,
            3,
            7,
            31,
            255,
            1024,
            u32::MAX as u64,
            u64::MAX / 2,
            u64::MAX - 1,
            u64::MAX,
        ];

        for lhs in VALUES {
            for rhs in VALUES {
                assert_eq!(
                    checked_add_u64_value(lhs, rhs, "overflow").ok(),
                    lhs.checked_add(rhs)
                );
                assert_eq!(
                    checked_mul_u64_value(lhs, rhs, "overflow").ok(),
                    lhs.checked_mul(rhs)
                );
                assert_eq!(
                    checked_sub_u64_value(lhs, rhs, "underflow").ok(),
                    lhs.checked_sub(rhs)
                );
            }
        }
    }

    #[test]
    fn checked_atomic_accounting_reports_overflow_and_underflow_without_saturation() {
        let add_counter = AtomicU64::new(u64::MAX - 1);
        checked_atomic_add_u64(&add_counter, 1, |_, _| unreachable!("one fits"))
            .expect("Fix: atomic add should accept exact non-overflow");
        assert_eq!(add_counter.load(Ordering::Relaxed), u64::MAX);
        let add_error = checked_atomic_add_u64(&add_counter, 1, |current, attempted| {
            crate::BackendError::InvalidProgram {
                fix: format!("Fix: overflow {current} {attempted}"),
            }
        })
        .expect_err("overflowing atomic add should fail");
        assert!(add_error.to_string().contains("overflow"));
        assert_eq!(add_counter.load(Ordering::Relaxed), u64::MAX);

        let sub_counter = AtomicU64::new(1);
        checked_atomic_sub_u64(&sub_counter, 1, |_, _| unreachable!("one fits"))
            .expect("Fix: atomic sub should accept exact subtraction");
        assert_eq!(sub_counter.load(Ordering::Acquire), 0);
        let sub_error = checked_atomic_sub_u64(&sub_counter, 1, |current, attempted| {
            crate::BackendError::InvalidProgram {
                fix: format!("Fix: underflow {current} {attempted}"),
            }
        })
        .expect_err("underflowing atomic sub should fail");
        assert!(sub_error.to_string().contains("underflow"));
        assert_eq!(sub_counter.load(Ordering::Acquire), 0);

        let usize_add_counter = AtomicUsize::new(usize::MAX - 1);
        checked_atomic_add_usize(&usize_add_counter, 1, |_, _| unreachable!("one fits"))
            .expect("Fix: usize atomic add should accept exact non-overflow");
        assert_eq!(usize_add_counter.load(Ordering::Acquire), usize::MAX);
        let usize_add_error =
            checked_atomic_add_usize(&usize_add_counter, 1, |current, attempted| {
                crate::BackendError::InvalidProgram {
                    fix: format!("Fix: usize overflow {current} {attempted}"),
                }
            })
            .expect_err("overflowing usize atomic add should fail");
        assert!(usize_add_error.to_string().contains("usize overflow"));
        assert_eq!(usize_add_counter.load(Ordering::Acquire), usize::MAX);

        let usize_counter = AtomicUsize::new(0);
        let usize_error = checked_atomic_sub_usize(&usize_counter, 1, |current, attempted| {
            crate::BackendError::InvalidProgram {
                fix: format!("Fix: usize underflow {current} {attempted}"),
            }
        })
        .expect_err("underflowing usize atomic sub should fail");
        assert!(usize_error.to_string().contains("usize underflow"));
        assert_eq!(usize_counter.load(Ordering::Acquire), 0);
    }

    #[test]
    fn ordered_atomic_helpers_preserve_domain_errors() {
        let add_counter = AtomicU64::new(40);
        checked_atomic_add_u64_with_order(
            &add_counter,
            2,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |_, _| "overflow",
        )
        .expect("Fix: reject adds that would overflow; use checked accounting API on hostile sizes - ordered atomic add should accept non-overflow");
        assert_eq!(add_counter.load(Ordering::Acquire), 42);

        let sub_counter = AtomicU64::new(42);
        checked_atomic_sub_u64_with_order(
            &sub_counter,
            2,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |_, _| "underflow",
        )
        .expect("Fix: reject subs that would underflow; use checked accounting API on hostile sizes - ordered atomic sub should accept non-underflow");
        assert_eq!(sub_counter.load(Ordering::Acquire), 40);

        let usize_counter = AtomicUsize::new(10);
        checked_atomic_add_usize_with_order(
            &usize_counter,
            5,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |_, _| "usize overflow",
        )
        .expect("Fix: reject usize atomics that overflow/underflow; return Err from guarded helpers - ordered usize atomic add should accept non-overflow");
        assert_eq!(usize_counter.load(Ordering::Acquire), 15);
        checked_atomic_sub_usize_with_order(
            &usize_counter,
            3,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |_, _| "usize underflow",
        )
        .expect("Fix: reject usize atomics that overflow/underflow; return Err from guarded helpers - ordered usize atomic sub should accept non-underflow");
        assert_eq!(usize_counter.load(Ordering::Acquire), 12);
    }

    #[test]
    fn guarded_atomic_add_helpers_validate_next_value_before_publish() {
        let u64_counter = AtomicU64::new(8);
        let u64_error = checked_atomic_add_u64_guarded_with_order(
            &u64_counter,
            5,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |_, _| "overflow",
            |next| {
                if next <= 12 {
                    Ok(())
                } else {
                    Err("budget")
                }
            },
        )
        .expect_err("guarded u64 add should reject over-budget next value");
        assert_eq!(u64_error, "budget");
        assert_eq!(u64_counter.load(Ordering::Acquire), 8);

        checked_atomic_add_u64_guarded_with_order(
            &u64_counter,
            4,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |_, _| "overflow",
            |next| if next <= 12 { Ok(()) } else { Err("budget") },
        )
        .expect("Fix: reject guarded adds that overflow; surface Err to caller instead of panicking - guarded u64 add should publish accepted next value");
        assert_eq!(u64_counter.load(Ordering::Acquire), 12);

        let usize_counter = AtomicUsize::new(3);
        let usize_error = checked_atomic_add_usize_guarded_with_order(
            &usize_counter,
            2,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |_, _| "overflow",
            |next| {
                if next < 5 {
                    Ok(())
                } else {
                    Err("usize budget")
                }
            },
        )
        .expect_err("guarded usize add should reject over-budget next value");
        assert_eq!(usize_error, "usize budget");
        assert_eq!(usize_counter.load(Ordering::Acquire), 3);
    }

    #[test]
    fn pinning_atomic_increment_helpers_never_wrap() {
        let u64_counter = AtomicU64::new(u64::MAX - 1);
        assert!(pinning_atomic_increment_u64(
            &u64_counter,
            Ordering::Relaxed,
            Ordering::Relaxed,
            || unreachable!("first increment should fit")
        ));
        assert_eq!(u64_counter.load(Ordering::Relaxed), u64::MAX);
        let mut u64_pinned = false;
        assert!(!pinning_atomic_increment_u64(
            &u64_counter,
            Ordering::Relaxed,
            Ordering::Relaxed,
            || u64_pinned = true
        ));
        assert!(u64_pinned);
        assert_eq!(u64_counter.load(Ordering::Relaxed), u64::MAX);

        let u32_counter = AtomicU32::new(u32::MAX - 1);
        assert!(pinning_atomic_increment_u32(
            &u32_counter,
            Ordering::Relaxed,
            Ordering::Relaxed,
            || unreachable!("first increment should fit")
        ));
        assert_eq!(u32_counter.load(Ordering::Relaxed), u32::MAX);
        let mut u32_pinned = false;
        assert!(!pinning_atomic_increment_u32(
            &u32_counter,
            Ordering::Relaxed,
            Ordering::Relaxed,
            || u32_pinned = true
        ));
        assert!(u32_pinned);
        assert_eq!(u32_counter.load(Ordering::Relaxed), u32::MAX);

        let mut scalar_counter = u64::MAX - 1;
        assert!(pinning_increment_u64(&mut scalar_counter, || {
            unreachable!("first scalar increment should fit")
        }));
        assert_eq!(scalar_counter, u64::MAX);
        let mut scalar_pinned = false;
        assert!(!pinning_increment_u64(&mut scalar_counter, || {
            scalar_pinned = true;
        }));
        assert!(scalar_pinned);
        assert_eq!(scalar_counter, u64::MAX);
    }

    #[test]
    fn atomic_max_helper_raises_without_lowering() {
        let counter = AtomicU64::new(10);
        assert_eq!(atomic_max_u64(&counter, 42, Ordering::Relaxed), 10);
        assert_eq!(counter.load(Ordering::Relaxed), 42);
        assert_eq!(atomic_max_u64(&counter, 7, Ordering::Relaxed), 42);
        assert_eq!(counter.load(Ordering::Relaxed), 42);
    }

    #[test]
    fn rebasing_atomic_next_returns_observed_and_rebases_on_overflow() {
        let counter = AtomicU64::new(7);
        let mut rebase_count = 0;
        assert_eq!(
            rebasing_atomic_next_u64(
                &counter,
                1,
                Ordering::Acquire,
                Ordering::AcqRel,
                Ordering::Acquire,
                |_, _| rebase_count += 1,
            ),
            7
        );
        assert_eq!(counter.load(Ordering::Acquire), 8);
        assert_eq!(rebase_count, 0);

        counter.store(u64::MAX, Ordering::Release);
        assert_eq!(
            rebasing_atomic_next_u64(
                &counter,
                1,
                Ordering::Acquire,
                Ordering::AcqRel,
                Ordering::Acquire,
                |observed, rebase_to| {
                    assert_eq!(observed, u64::MAX);
                    assert_eq!(rebase_to, 1);
                    rebase_count += 1;
                },
            ),
            u64::MAX
        );
        assert_eq!(counter.load(Ordering::Acquire), 1);
        assert_eq!(rebase_count, 1);
    }

    #[test]
    fn checked_atomic_next_returns_observed_and_rejects_wraparound() {
        let counter = AtomicU64::new(41);
        assert_eq!(
            checked_atomic_next_u64_with_order(
                &counter,
                Ordering::Acquire,
                Ordering::AcqRel,
                Ordering::Acquire,
                |_| "overflow",
            )
            .expect("Fix: allocation of next atomic value must not overflow; return None/Err on hostile input - checked atomic next should allocate non-overflowing value"),
            41
        );
        assert_eq!(counter.load(Ordering::Acquire), 42);

        counter.store(u64::MAX, Ordering::Release);
        let error = checked_atomic_next_u64_with_order(
            &counter,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |observed| {
                assert_eq!(observed, u64::MAX);
                "overflow"
            },
        )
        .expect_err("checked atomic next should reject u64 wraparound");
        assert_eq!(error, "overflow");
        assert_eq!(counter.load(Ordering::Acquire), u64::MAX);
    }

    #[test]
    fn repair_atomic_sub_usize_repairs_underflow_to_zero_once() {
        let counter = AtomicUsize::new(10);
        let mut repairs = 0;
        repair_atomic_sub_usize_with_order(
            &counter,
            4,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |_, _| repairs += 1,
        );
        assert_eq!(counter.load(Ordering::Acquire), 6);
        assert_eq!(repairs, 0);

        repair_atomic_sub_usize_with_order(
            &counter,
            99,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |observed, attempted| {
                assert_eq!(observed, 6);
                assert_eq!(attempted, 99);
                repairs += 1;
            },
        );
        assert_eq!(counter.load(Ordering::Acquire), 0);
        assert_eq!(repairs, 1);
    }
}
