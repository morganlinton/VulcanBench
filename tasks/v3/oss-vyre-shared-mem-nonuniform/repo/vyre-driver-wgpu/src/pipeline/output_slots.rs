//! WGPU wrapper around backend-neutral output-slot resizing.

use vyre_driver::BackendError;

pub(crate) fn resize_vec_with<T, F>(
    vec: &mut Vec<T>,
    len: usize,
    make: F,
    label: &'static str,
) -> Result<(), BackendError>
where
    F: FnMut() -> T,
{
    vyre_driver::output_slots::resize_vec_with(
        vec,
        len,
        make,
        "WGPU pipeline",
        label,
        "split the dispatch batch before readback",
    )
}

#[cfg(test)]
mod tests {
    use super::resize_vec_with;

    #[test]
    fn generated_output_slot_resize_preserves_prefix_and_matches_requested_len() {
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

            resize_vec_with(&mut slots, target_len, Vec::new, "generated output slots")
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
}
