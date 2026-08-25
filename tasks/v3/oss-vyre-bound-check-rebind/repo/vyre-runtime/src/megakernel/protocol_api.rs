//! Host protocol API wrappers for megakernel control/ring buffers.

mod publish;
pub use publish::RingSlotTransition;

use crate::PipelineError;

use super::protocol::{self, DebugRecord};
use super::Megakernel;

impl Megakernel {
    /// Byte length of a control buffer for `observable_slots`.
    #[must_use]
    pub fn control_byte_len(observable_slots: u32) -> Option<usize> {
        protocol::control_byte_len(observable_slots)
    }

    /// Byte length of a ring buffer for `slot_count`.
    #[must_use]
    pub fn ring_byte_len(slot_count: u32) -> Option<usize> {
        protocol::ring_byte_len(slot_count)
    }

    /// Byte length of a debug-log buffer for `record_capacity`.
    #[must_use]
    pub fn debug_log_byte_len(record_capacity: u32) -> Option<usize> {
        protocol::debug_log_byte_len(record_capacity)
    }

    /// Default debug-log record capacity owned by the runtime protocol.
    #[must_use]
    pub fn debug_record_capacity() -> u32 {
        protocol::debug::RECORD_CAPACITY
    }

    /// Encode a control-buffer payload.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the requested observable region
    /// cannot fit in process address space.
    pub fn encode_control(
        shutdown: bool,
        tenant_count: u32,
        observable_slots: u32,
    ) -> Result<Vec<u8>, PipelineError> {
        protocol::encode_control(shutdown, tenant_count, observable_slots).map_err(protocol_error)
    }

    /// Fallible control-buffer encoder for callers accepting untrusted sizing.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the requested observable region
    /// cannot fit in process address space.
    pub fn try_encode_control(
        shutdown: bool,
        tenant_count: u32,
        observable_slots: u32,
    ) -> Result<Vec<u8>, PipelineError> {
        Self::encode_control(shutdown, tenant_count, observable_slots)
    }

    /// Fallible control-buffer encoder into caller-owned storage.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the requested observable region
    /// cannot fit in process address space.
    pub fn try_encode_control_into(
        shutdown: bool,
        tenant_count: u32,
        observable_slots: u32,
        dst: &mut Vec<u8>,
    ) -> Result<(), PipelineError> {
        protocol::try_encode_control_into(shutdown, tenant_count, observable_slots, dst)
            .map_err(protocol_error)
    }

    /// Encode an empty ring buffer with `slot_count` slots.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when `slot_count * SLOT_WORDS * 4`
    /// overflows.
    pub fn encode_empty_ring(slot_count: u32) -> Result<Vec<u8>, PipelineError> {
        protocol::encode_empty_ring(slot_count).map_err(protocol_error)
    }

    /// Fallible ring-buffer encoder for callers accepting untrusted slot counts.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when `slot_count * SLOT_WORDS * 4`
    /// overflows.
    pub fn try_encode_empty_ring(slot_count: u32) -> Result<Vec<u8>, PipelineError> {
        Self::encode_empty_ring(slot_count)
    }

    /// Fallible ring-buffer encoder into caller-owned storage.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when `slot_count * SLOT_WORDS * 4`
    /// overflows.
    pub fn try_encode_empty_ring_into(
        slot_count: u32,
        dst: &mut Vec<u8>,
    ) -> Result<(), PipelineError> {
        protocol::try_encode_empty_ring_into(slot_count, dst).map_err(protocol_error)
    }

    /// Encode an empty PRINTF channel buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the record capacity overflows.
    pub fn encode_empty_debug_log(record_capacity: u32) -> Result<Vec<u8>, PipelineError> {
        protocol::encode_empty_debug_log(record_capacity).map_err(protocol_error)
    }

    /// Fallible debug-log encoder for callers accepting untrusted capacities.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the record capacity overflows.
    pub fn try_encode_empty_debug_log(record_capacity: u32) -> Result<Vec<u8>, PipelineError> {
        Self::encode_empty_debug_log(record_capacity)
    }

    /// Fallible debug-log encoder into caller-owned storage.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the record capacity overflows.
    pub fn try_encode_empty_debug_log_into(
        record_capacity: u32,
        dst: &mut Vec<u8>,
    ) -> Result<(), PipelineError> {
        protocol::try_encode_empty_debug_log_into(record_capacity, dst).map_err(protocol_error)
    }

    /// Decode the kernel's `done_count` from a control buffer.
    #[must_use]
    pub fn read_done_count(control_bytes: &[u8]) -> u32 {
        protocol::read_done_count(control_bytes)
    }

    /// Strictly decode the kernel's `done_count` from a control buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when the control buffer is malformed or too
    /// short to contain the done counter.
    pub fn try_read_done_count(control_bytes: &[u8]) -> Result<u32, PipelineError> {
        protocol::try_read_done_count(control_bytes).map_err(protocol_error)
    }

    /// Strictly count DONE slots in a ring-buffer readback.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when the ring readback is malformed or too
    /// short for `item_count` complete protocol slots.
    pub fn try_count_done_ring_slots(
        ring_bytes: &[u8],
        item_count: usize,
    ) -> Result<u64, PipelineError> {
        protocol::try_count_done_ring_slots(ring_bytes, item_count).map_err(protocol_error)
    }

    /// Decode PRINTF records out of the debug-log buffer.
    #[must_use]
    pub fn read_debug_log(debug_bytes: &[u8]) -> Vec<DebugRecord> {
        protocol::read_debug_log(debug_bytes)
    }

    /// Decode PRINTF records into caller-owned storage.
    pub fn read_debug_log_into(debug_bytes: &[u8], out: &mut Vec<DebugRecord>) {
        protocol::read_debug_log_into(debug_bytes, out);
    }

    /// Strictly decode PRINTF records out of the debug-log buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when the debug-log buffer is malformed or the
    /// cursor points at a partial record.
    pub fn try_read_debug_log(debug_bytes: &[u8]) -> Result<Vec<DebugRecord>, PipelineError> {
        protocol::try_read_debug_log(debug_bytes).map_err(protocol_error)
    }

    /// Strictly decode PRINTF records into caller-owned storage.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when the debug-log buffer is malformed or the
    /// cursor points at a partial record.
    pub fn try_read_debug_log_into(
        debug_bytes: &[u8],
        out: &mut Vec<DebugRecord>,
    ) -> Result<(), PipelineError> {
        protocol::try_read_debug_log_into(debug_bytes, out).map_err(protocol_error)
    }

    /// Read the epoch counter from a control buffer. The epoch
    /// increments on each `BATCH_FENCE` execution  -  the host polls
    /// this to detect batch completion without scanning the ring.
    #[must_use]
    pub fn read_epoch(control_bytes: &[u8]) -> u32 {
        protocol::read_epoch(control_bytes)
    }

    /// Strictly read the epoch counter from a control buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when the control buffer is malformed or too
    /// short to contain the epoch counter.
    pub fn try_read_epoch(control_bytes: &[u8]) -> Result<u32, PipelineError> {
        protocol::try_read_epoch(control_bytes).map_err(protocol_error)
    }

    /// Read an observable result word from a control buffer.
    /// Opcodes like `LOAD_U32`, `COMPARE_SWAP`, and `BATCH_FENCE`
    /// write results here.
    #[must_use]
    pub fn read_observable(control_bytes: &[u8], index: u32) -> u32 {
        protocol::read_observable(control_bytes, index)
    }

    /// Strictly read an observable result word from a control buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when the buffer is malformed or the
    /// observable index is outside the supplied readback.
    pub fn try_read_observable(control_bytes: &[u8], index: u32) -> Result<u32, PipelineError> {
        protocol::try_read_observable(control_bytes, index).map_err(protocol_error)
    }

    /// Read per-opcode metrics counters from a control buffer.
    /// Returns a map of `opcode_id → execution_count` for any
    /// non-zero counters.
    #[must_use]
    pub fn read_metrics(control_bytes: &[u8]) -> Vec<(u32, u32)> {
        protocol::read_metrics(control_bytes)
    }

    /// Read per-opcode metrics counters into caller-owned storage.
    pub fn read_metrics_into(control_bytes: &[u8], out: &mut Vec<(u32, u32)>) {
        protocol::read_metrics_into(control_bytes, out);
    }

    /// Strictly read per-opcode metrics counters from a control buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when the buffer is malformed or too short for
    /// the fixed metrics window.
    pub fn try_read_metrics(control_bytes: &[u8]) -> Result<Vec<(u32, u32)>, PipelineError> {
        protocol::try_read_metrics(control_bytes).map_err(protocol_error)
    }

    /// Strictly read per-opcode metrics counters into caller-owned storage.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when the buffer is malformed or too short for
    /// the fixed metrics window.
    pub fn try_read_metrics_into(
        control_bytes: &[u8],
        out: &mut Vec<(u32, u32)>,
    ) -> Result<(), PipelineError> {
        protocol::try_read_metrics_into(control_bytes, out).map_err(protocol_error)
    }
}

fn protocol_error(error: protocol::ProtocolError) -> PipelineError {
    match error {
        protocol::ProtocolError::ByteLengthOverflow { fix, .. } => PipelineError::QueueFull {
            queue: "submission",
            fix,
        },
        other => PipelineError::Backend(other.to_string()),
    }
}

pub(super) fn validate_control_bytes(control_bytes: &[u8]) -> Result<(), PipelineError> {
    let min = protocol::control_byte_len(0).ok_or_else(|| {
        PipelineError::Backend(
            "megakernel minimum control-buffer length overflowed usize. Fix: keep CONTROL_MIN_WORDS within host address limits."
                .to_string(),
        )
    })?;
    if control_bytes.len() < min || control_bytes.len() % 4 != 0 {
        return Err(PipelineError::Backend(format!(
            "megakernel control buffer has {} bytes, expected at least {min} bytes and 4-byte alignment. Fix: build it with Megakernel::encode_control.",
            control_bytes.len()
        )));
    }
    Ok(())
}

pub(super) fn validate_debug_log_bytes(debug_log_bytes: &[u8]) -> Result<(), PipelineError> {
    let expected = protocol::debug_log_byte_len(protocol::debug::RECORD_CAPACITY)
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "debug-log minimum length overflowed usize; keep debug ABI constants within host limits",
        })?;
    if debug_log_bytes.len() != expected {
        return Err(PipelineError::Backend(format!(
            "megakernel debug-log buffer has {} bytes, expected exactly {expected} bytes for {} PRINTF records. Fix: build it with Megakernel::encode_empty_debug_log(protocol::debug::RECORD_CAPACITY).",
            debug_log_bytes.len(),
            protocol::debug::RECORD_CAPACITY
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
