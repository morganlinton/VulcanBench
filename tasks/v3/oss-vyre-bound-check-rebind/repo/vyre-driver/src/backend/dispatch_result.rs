//! Dispatch output payloads shared by every backend.

/// Output of one dispatch: a vector per output buffer slot, each
/// vector holding the raw bytes read back from the GPU. Consumers
/// decode the bytes per the Program's output buffer declarations.
/// The outer vec is indexed in the same order as the Program's
/// `is_output: true` buffers.
pub type OutputBuffers = Vec<Vec<u8>>;

/// Slot-reuse accounting from output-buffer replacement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OutputSlotStats {
    /// Total output slots written after replacement.
    pub total_slots: usize,
    /// Existing output slots whose allocation was reused.
    pub reused_slots: usize,
    /// Existing output slots replaced by moving an oversized incoming allocation.
    pub moved_slots: usize,
    /// New output slots appended beyond the previous output vector length.
    pub appended_slots: usize,
}

/// Byte-pressure accounting from output-buffer replacement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OutputSlotByteStats {
    /// Bytes presented by incoming output buffers before replacement.
    pub incoming_bytes: usize,
    /// Bytes copied into retained caller-owned slots.
    pub copied_bytes: usize,
    /// Bytes moved into place by swapping oversized incoming allocations.
    pub moved_bytes: usize,
    /// Bytes appended beyond the previous output vector length.
    pub appended_bytes: usize,
    /// Total retained capacity of output slots after replacement.
    pub retained_capacity_bytes: usize,
}

/// Full output replacement accounting: slot decisions plus byte pressure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OutputReplacementStats {
    /// Slot-level reuse/move/append accounting.
    pub slots: OutputSlotStats,
    /// Byte-level copy/move/append/capacity accounting.
    pub bytes: OutputSlotByteStats,
}

/// Replace `outputs` with `incoming` while preserving already-allocated output
/// slots whenever their positions still exist.
pub fn replace_output_buffers_preserving_slots(
    incoming: OutputBuffers,
    outputs: &mut OutputBuffers,
) {
    let _ = replace_output_buffers_preserving_slots_with_stats(incoming, outputs);
}

/// Replace output buffers and return allocation-reuse accounting.
pub fn replace_output_buffers_preserving_slots_with_stats(
    incoming: OutputBuffers,
    outputs: &mut OutputBuffers,
) -> OutputSlotStats {
    replace_output_buffers_preserving_slots_with_memory_stats(incoming, outputs).slots
}

/// Replace output buffers and return allocation-reuse plus byte-pressure
/// accounting.
pub fn replace_output_buffers_preserving_slots_with_memory_stats(
    incoming: OutputBuffers,
    outputs: &mut OutputBuffers,
) -> OutputReplacementStats {
    let total_slots = incoming.len();
    let previous_slots = outputs.len();
    reserve_output_slots_for_replacement(outputs, total_slots);
    let mut incoming = incoming.into_iter();
    let mut retained_slots = 0usize;
    let mut reused_slots = 0usize;
    let mut moved_slots = 0usize;
    let mut incoming_bytes = 0usize;
    let mut copied_bytes = 0usize;
    let mut moved_bytes = 0usize;
    let mut appended_bytes = 0usize;
    for (slot, mut bytes) in outputs.iter_mut().zip(incoming.by_ref()) {
        incoming_bytes = add_bytes(incoming_bytes, bytes.len(), "incoming output bytes");
        if bytes.len() <= slot.capacity() {
            slot.clear();
            copied_bytes = add_bytes(copied_bytes, bytes.len(), "copied output bytes");
            slot.extend_from_slice(&bytes);
            reused_slots += 1;
        } else {
            moved_bytes = add_bytes(moved_bytes, bytes.len(), "moved output bytes");
            std::mem::swap(slot, &mut bytes);
            moved_slots += 1;
        }
        retained_slots += 1;
    }
    outputs.truncate(retained_slots);
    for bytes in incoming {
        incoming_bytes = add_bytes(incoming_bytes, bytes.len(), "incoming output bytes");
        appended_bytes = add_bytes(appended_bytes, bytes.len(), "appended output bytes");
        outputs.push(bytes);
    }
    let retained_capacity_bytes = outputs.iter().fold(0usize, |sum, output| {
        add_bytes(sum, output.capacity(), "retained output capacity bytes")
    });
    OutputReplacementStats {
        slots: OutputSlotStats {
            total_slots,
            reused_slots,
            moved_slots,
            appended_slots: total_slots.checked_sub(previous_slots).unwrap_or(0),
        },
        bytes: OutputSlotByteStats {
            incoming_bytes,
            copied_bytes,
            moved_bytes,
            appended_bytes,
            retained_capacity_bytes,
        },
    }
}

fn reserve_output_slots_for_replacement(outputs: &mut OutputBuffers, total_slots: usize) {
    let _ = crate::allocation::try_reserve_vec_to_capacity(outputs, total_slots);
}

fn add_bytes(current: usize, incoming: usize, _label: &str) -> usize {
    current.saturating_add(incoming)
}

/// Output plus timing captured by a backend-owned dispatch path.
///
/// `wall_ns` is always populated by the shared default implementation.
/// `device_ns` is populated only when a backend can measure elapsed device
/// stream time without crossing the driver boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimedDispatchResult {
    /// Output buffers in the same order as [`crate::backend::VyreBackend::dispatch`].
    pub outputs: OutputBuffers,
    /// Host-observed dispatch duration.
    pub wall_ns: u64,
    /// Device-observed elapsed time when the backend exposes a timer.
    pub device_ns: Option<u64>,
    /// Host time spent enqueueing backend work before the caller begins
    /// waiting for completion.
    pub enqueue_ns: Option<u64>,
    /// Host time spent waiting for completion and collecting output buffers.
    pub wait_ns: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_output_buffers_preserves_existing_slots() {
        let mut outputs = vec![Vec::with_capacity(8), Vec::with_capacity(4)];
        let outputs_addr = outputs.as_ptr() as usize;
        let first_slot_addr = outputs[0].as_ptr() as usize;
        let second_slot_addr = outputs[1].as_ptr() as usize;

        replace_output_buffers_preserving_slots(vec![vec![1, 2], vec![3]], &mut outputs);

        assert_eq!(outputs, vec![vec![1, 2], vec![3]]);
        assert_eq!(outputs.as_ptr() as usize, outputs_addr);
        assert_eq!(outputs[0].as_ptr() as usize, first_slot_addr);
        assert_eq!(outputs[1].as_ptr() as usize, second_slot_addr);
    }

    #[test]
    fn replace_output_buffers_truncates_without_dropping_reused_slots() {
        let mut outputs = vec![Vec::with_capacity(8), Vec::with_capacity(4)];
        let outputs_addr = outputs.as_ptr() as usize;
        let first_slot_addr = outputs[0].as_ptr() as usize;

        replace_output_buffers_preserving_slots(vec![vec![9]], &mut outputs);

        assert_eq!(outputs, vec![vec![9]]);
        assert_eq!(outputs.as_ptr() as usize, outputs_addr);
        assert_eq!(outputs[0].as_ptr() as usize, first_slot_addr);
    }

    #[test]
    fn replace_output_buffers_moves_oversized_incoming_slot_without_copy() {
        let mut outputs = vec![Vec::with_capacity(1)];
        let incoming = vec![vec![1, 2, 3, 4]];
        let incoming_ptr = incoming[0].as_ptr() as usize;

        replace_output_buffers_preserving_slots(incoming, &mut outputs);

        assert_eq!(outputs, vec![vec![1, 2, 3, 4]]);
        assert_eq!(
            outputs[0].as_ptr() as usize,
            incoming_ptr,
            "oversized incoming output should be moved into place instead of copied through a too-small retained slot"
        );
    }

    #[test]
    fn replace_output_buffers_reports_reuse_move_and_append_stats() {
        let mut outputs = vec![Vec::with_capacity(8), Vec::with_capacity(1)];

        let stats = replace_output_buffers_preserving_slots_with_stats(
            vec![vec![1, 2], vec![3, 4], vec![5]],
            &mut outputs,
        );

        assert_eq!(outputs, vec![vec![1, 2], vec![3, 4], vec![5]]);
        assert_eq!(
            stats,
            OutputSlotStats {
                total_slots: 3,
                reused_slots: 1,
                moved_slots: 1,
                appended_slots: 1,
            }
        );
    }

    #[test]
    fn replace_output_buffers_reserves_outer_slots_before_appending() {
        let mut outputs: OutputBuffers = Vec::with_capacity(3);
        outputs.push(Vec::with_capacity(4));
        outputs[0].extend_from_slice(&[0xaa]);
        let outer_ptr = outputs.as_ptr() as usize;
        let first_slot_ptr = outputs[0].as_ptr() as usize;

        let stats = replace_output_buffers_preserving_slots_with_memory_stats(
            vec![vec![1, 2], vec![3], vec![4, 5, 6]],
            &mut outputs,
        );

        assert_eq!(outputs, vec![vec![1, 2], vec![3], vec![4, 5, 6]]);
        assert_eq!(
            outputs.as_ptr() as usize,
            outer_ptr,
            "outer output vector had enough capacity and must not reallocate while appending new readback slots"
        );
        assert_eq!(
            outputs[0].as_ptr() as usize,
            first_slot_ptr,
            "first output slot should be reused because the incoming bytes fit its retained allocation"
        );
        assert_eq!(stats.slots.appended_slots, 2);
        assert_eq!(stats.bytes.appended_bytes, 4);
    }

    #[test]
    fn replace_output_buffers_reports_byte_pressure_stats() {
        let mut outputs = vec![Vec::with_capacity(8), Vec::with_capacity(1)];

        let stats = replace_output_buffers_preserving_slots_with_memory_stats(
            vec![vec![1, 2], vec![3, 4], vec![5]],
            &mut outputs,
        );

        assert_eq!(outputs, vec![vec![1, 2], vec![3, 4], vec![5]]);
        assert_eq!(
            stats.bytes,
            OutputSlotByteStats {
                incoming_bytes: 5,
                copied_bytes: 2,
                moved_bytes: 2,
                appended_bytes: 1,
                retained_capacity_bytes: 11,
            }
        );
    }
}
