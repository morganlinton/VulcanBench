//! Atomic operation semantics enforced by the parity engine.
//!
//! GPU atomic instructions vary in memory ordering and return-value behavior
//! across backends. This module exists to define one sequentially consistent,
//! return-old-value semantics that every backend must match byte-for-byte.

use vyre::ir::AtomicOp;

use vyre::Error;

/// Apply one sequentially consistent atomic operation.
///
/// # Errors
///
/// Returns [`Error::Interp`] if `AtomicOp::CompareExchange` is
/// invoked without an `expected` value.
pub fn apply(
    op: AtomicOp,
    old: u32,
    expected: Option<u32>,
    value: u32,
) -> Result<(u32, u32), vyre::Error> {
    match op {
        AtomicOp::Add => Ok(atomic_add(old, value)),
        AtomicOp::Or => Ok(atomic_or(old, value)),
        AtomicOp::And => Ok(atomic_and(old, value)),
        AtomicOp::Xor => Ok(atomic_xor(old, value)),
        AtomicOp::Min => Ok(atomic_min(old, value)),
        AtomicOp::Max => Ok(atomic_max(old, value)),
        AtomicOp::Exchange => Ok(atomic_exchange(old, value)),
        AtomicOp::CompareExchange => atomic_compare_exchange(old, expected, value),
        AtomicOp::LruUpdate => Ok(atomic_lru_update(old, value)),
        _ => Err(Error::interp(format!(
            "unsupported atomic op `{op:?}` reached the reference interpreter. Fix: define sequential semantics before constructing this AtomicOp."
        ))),
    }
}

/// Return the old value and the value after atomic add.
pub fn atomic_add(old: u32, value: u32) -> (u32, u32) {
    (old, old.wrapping_add(value))
}

/// Return the old value and the value after atomic bitwise OR.
pub fn atomic_or(old: u32, value: u32) -> (u32, u32) {
    (old, old | value)
}

/// Return the old value and the value after atomic bitwise AND.
pub fn atomic_and(old: u32, value: u32) -> (u32, u32) {
    (old, old & value)
}

/// Return the old value and the value after atomic bitwise XOR.
pub fn atomic_xor(old: u32, value: u32) -> (u32, u32) {
    (old, old ^ value)
}

/// Return the old value and the value after atomic unsigned minimum.
pub fn atomic_min(old: u32, value: u32) -> (u32, u32) {
    (old, old.min(value))
}

/// Return the old value and the value after atomic unsigned maximum.
pub fn atomic_max(old: u32, value: u32) -> (u32, u32) {
    (old, old.max(value))
}

/// Return the old value and the replacement value for atomic exchange.
pub fn atomic_exchange(old: u32, value: u32) -> (u32, u32) {
    (old, value)
}

/// LRU-update semantics: replace the slot with `value` only when
/// `value` is strictly greater than `old` (the "more recent
/// timestamp" wins). Identical tie-breaker behavior to `atomic_max`,
/// kept as a distinct op so backends can lower it to a dedicated
/// LRU-tracking instruction on hardware that has one. CPU reference is the
/// correctness oracle.
pub fn atomic_lru_update(old: u32, value: u32) -> (u32, u32) {
    (old, old.max(value))
}

/// Return the old value and the value after atomic compare-exchange.
///
/// # Errors
///
/// Returns [`Error::Interp`] if `expected` is `None`.
pub fn atomic_compare_exchange(
    old: u32,
    expected: Option<u32>,
    value: u32,
) -> Result<(u32, u32), vyre::Error> {
    let Some(expected) = expected else {
        return Err(Error::interp(
            "compare-exchange atomic is missing expected value. Fix: set Expr::Atomic.expected.",
        ));
    };
    let new = if old == expected { value } else { old };
    Ok((old, new))
}
