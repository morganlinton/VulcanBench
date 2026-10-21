//! Backend-neutral fallible output-slot vector management.
//!
//! CUDA, WGPU, and future native backends all resize caller-owned output slot
//! vectors on hot dispatch paths. The policy is identical: preserve existing
//! slots where possible, grow fallibly, initialize new slots from a caller
//! factory, and truncate stale slots. Keeping that policy here prevents
//! backend-local allocation drift.

use crate::BackendError;

/// Reserve enough capacity for a target vector length without mutating length.
///
/// # Errors
///
/// Returns [`BackendError`] when the vector cannot reserve the additional
/// capacity required for `target_len`.
pub fn reserve_vec_exact_for_len<T>(
    vec: &mut Vec<T>,
    target_len: usize,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError> {
    if vec.len() < target_len {
        let additional = target_len - vec.len();
        vec.try_reserve_exact(additional).map_err(|source| {
            BackendError::new(format!(
                "{context} could not reserve {additional} additional {item}(s) for target length {target_len}: {source}. Fix: {fix}."
            ))
        })?;
    }
    Ok(())
}

/// Resize a vector while preserving the existing prefix and growing fallibly.
///
/// # Errors
///
/// Returns [`BackendError`] when growth to `len` cannot reserve memory.
pub fn resize_vec_with<T, F>(
    vec: &mut Vec<T>,
    len: usize,
    make: F,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError>
where
    F: FnMut() -> T,
{
    if vec.len() < len {
        reserve_vec_exact_for_len(vec, len, context, item, fix)?;
        vec.resize_with(len, make);
    } else {
        vec.truncate(len);
    }
    Ok(())
}

/// Ensure a `Vec<Vec<T>>` has at least `slot_count` output slots.
///
/// Existing slot buffers are preserved; new slots are empty vectors.
///
/// # Errors
///
/// Returns [`BackendError`] when the outer slot vector cannot grow.
pub fn ensure_vec_slots_at_least<T>(
    slots: &mut Vec<Vec<T>>,
    slot_count: usize,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError> {
    if slots.len() < slot_count {
        reserve_vec_exact_for_len(slots, slot_count, context, item, fix)?;
        slots.resize_with(slot_count, Vec::new);
    }
    Ok(())
}

/// Resize a `Vec<Vec<T>>` to exactly `slot_count` output slots.
///
/// Existing slot buffers are preserved up to the new length; stale trailing
/// slots are dropped.
///
/// # Errors
///
/// Returns [`BackendError`] when the outer slot vector cannot grow.
pub fn resize_vec_slots<T>(
    slots: &mut Vec<Vec<T>>,
    slot_count: usize,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError> {
    resize_vec_with(slots, slot_count, Vec::new, context, item, fix)
}

/// Clear every inner output buffer without changing the slot count.
pub fn clear_vec_slots<T>(slots: &mut [Vec<T>]) {
    for slot in slots {
        slot.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{clear_vec_slots, ensure_vec_slots_at_least, resize_vec_slots, resize_vec_with};

    #[test]
    fn resize_vec_with_preserves_prefix_and_initializes_new_slots() {
        for case in 0..4096 {
            let initial_len = case % 17;
            let target_len = (case * 7 + 3) % 23;
            let mut slots = Vec::new();
            slots
                .try_reserve(initial_len)
                .expect("Fix: generated resize test must reserve initial slots");
            for idx in 0..initial_len {
                slots.push(vec![idx as u8; (idx % 5) + 1]);
            }
            let expected_prefix: Vec<Vec<u8>> = slots.iter().take(target_len).cloned().collect();

            resize_vec_with(
                &mut slots,
                target_len,
                Vec::new,
                "generated output slots",
                "slot",
                "split generated dispatch",
            )
            .expect("Fix: generated output slot resize should be fallible but successful");

            assert_eq!(
                slots.len(),
                target_len,
                "generated resize case {case} must match target length"
            );
            assert_eq!(
                &slots[..expected_prefix.len()],
                expected_prefix.as_slice(),
                "generated resize case {case} must preserve existing output slots"
            );
            for slot in slots.iter().skip(initial_len.min(target_len)) {
                assert!(
                    slot.is_empty(),
                    "generated resize case {case} must initialize new output slots as empty Vecs"
                );
            }
        }
    }

    #[test]
    fn vec_slot_helpers_can_grow_truncate_and_clear() {
        let mut slots = vec![vec![1_u8], vec![2, 3]];
        ensure_vec_slots_at_least(
            &mut slots,
            4,
            "generated slots",
            "slot",
            "split generated dispatch",
        )
        .expect("Fix: slot growth should reserve successfully");
        assert_eq!(slots.len(), 4);
        assert_eq!(slots[0], vec![1]);
        assert_eq!(slots[1], vec![2, 3]);
        assert!(slots[2].is_empty());
        assert!(slots[3].is_empty());

        resize_vec_slots(
            &mut slots,
            1,
            "generated slots",
            "slot",
            "split generated dispatch",
        )
        .expect("Fix: slot truncation should not allocate");
        assert_eq!(slots, vec![vec![1]]);

        clear_vec_slots(&mut slots);
        assert_eq!(slots, vec![Vec::<u8>::new()]);
    }
}
