//! Ring-buffer protocol constants  -  slot layout, control words, opcodes, debug log.
//!
//! Pure data module. No logic, no imports beyond std. Every constant
//! has a doc-comment that says what the GPU kernel does with it.

/// A single PRINTF event decoded out of the debug-log buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugRecord {
    /// Format-string id  -  resolved by the host against its
    /// registered format table.
    pub fmt_id: u32,
    /// Three argument words in the order the kernel wrote them.
    pub args: [u32; 3],
}

/// Megakernel host-protocol encoding and decoding error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ProtocolError {
    /// A requested buffer length overflowed host address space.
    #[error("{buffer} byte length overflow. Fix: {fix}")]
    ByteLengthOverflow {
        /// Protocol buffer being sized.
        buffer: &'static str,
        /// Actionable remediation.
        fix: &'static str,
    },
    /// A byte slice is not aligned to full u32 protocol words.
    #[error("{buffer} has {byte_len} bytes, not a whole number of u32 words. Fix: {fix}")]
    MisalignedByteLength {
        /// Protocol buffer being decoded.
        buffer: &'static str,
        /// Byte length received by the decoder.
        byte_len: usize,
        /// Actionable remediation.
        fix: &'static str,
    },
    /// A requested protocol word is outside the supplied byte slice.
    #[error("{buffer} is missing word {word_idx} in {byte_len} bytes. Fix: {fix}")]
    MissingWord {
        /// Protocol buffer being decoded.
        buffer: &'static str,
        /// Word index requested.
        word_idx: usize,
        /// Byte length received by the decoder.
        byte_len: usize,
        /// Actionable remediation.
        fix: &'static str,
    },
}

/// Number of u32 words each ring-buffer slot occupies. 16 words = 64 B,
/// a cache line on x86_64 and the slot size NVMe Submission Queue
/// Entries will use when the `uring-cmd-nvme` extension lands.
pub const SLOT_WORDS: u32 = 16;

/// Word index of the slot status header (the CAS target).
pub const STATUS_WORD: u32 = 0;

/// Word index of the slot opcode (dispatched via If-tree).
pub const OPCODE_WORD: u32 = 1;

/// Word index of the slot tenant id.
pub const TENANT_WORD: u32 = 2;

/// Word index of the slot priority level.
pub const PRIORITY_WORD: u32 = 3;

/// First argument word. Opcodes read args at
/// `ring_buffer[slot_base + ARG0_WORD .. slot_base + SLOT_WORDS]`.
pub const ARG0_WORD: u32 = 4;

/// Number of u32 argument words available per slot (12).
pub const ARGS_PER_SLOT: u32 = SLOT_WORDS - ARG0_WORD;

/// Control-buffer slot layout helpers.
pub mod control;
/// Debug helpers for inspecting megakernel slot/opcode state at runtime.
pub mod debug;
/// Opcode constants and decoding utilities for megakernel slots.
pub mod opcode;
/// Slot layout helpers (per-slot offsets, ARG0 helpers).
pub mod slot;

/// Minimum control-buffer words required by the compiled megakernel ABI.
///
/// This covers shutdown, done count, tenant masks, metrics, epoch, priority
/// offsets, and the statically declared read/write buffer count in the IR.
pub const CONTROL_MIN_WORDS: u32 = 160;
/// Maximum host-observable words whose control-buffer byte length is
/// representable by the u32 wire ABI.
pub const MAX_OBSERVABLE_SLOTS: u32 = u32::MAX - control::OBSERVABLE_BASE;
/// Maximum host-observable words the allocating encoder will materialize.
pub const MAX_ENCODED_OBSERVABLE_SLOTS: u32 = 1_048_576;
/// Maximum ring slots whose byte length is representable by the u32 wire ABI.
pub const MAX_RING_SLOTS: u32 = u32::MAX / SLOT_WORDS;
/// Maximum ring slots the allocating encoder will materialize.
pub const MAX_ENCODED_RING_SLOTS: u32 = 1_048_576;
/// Maximum debug-log records whose byte length is representable by the u32 wire ABI.
pub const MAX_DEBUG_RECORDS: u32 = u32::MAX / debug::RECORD_WORDS;
/// Maximum debug-log records the allocating encoder will materialize.
pub const MAX_ENCODED_DEBUG_RECORDS: u32 = 1_048_576;

mod codec;

pub use codec::{
    control_byte_len, count_done_ring_slots, debug_log_byte_len, encode_control,
    encode_empty_debug_log, encode_empty_ring, read_debug_log, read_debug_log_into,
    read_done_count, read_epoch, read_metrics, read_metrics_into, read_observable, ring_byte_len,
    try_count_done_ring_slots, try_encode_control, try_encode_control_into,
    try_encode_empty_debug_log, try_encode_empty_debug_log_into, try_encode_empty_ring,
    try_encode_empty_ring_into, try_read_debug_log, try_read_debug_log_into, try_read_done_count,
    try_read_epoch, try_read_metrics, try_read_metrics_into, try_read_observable,
};

/// Encode a single ring-buffer slot for a load-miss request.
///
/// Returns a 64-byte `Vec<u8>` containing the slot words:
/// - status = [`slot::PUBLISHED`]
/// - opcode = [`opcode::LOAD_MISS`]
/// - tenant = 0
/// - priority = 0
/// - arg0 = resource_id (opaque to vyre; consumer-defined)
/// - arg1 = prefetch as u32
#[must_use]
pub fn encode_load_miss(resource_id: u32, prefetch: bool) -> Vec<u8> {
    try_encode_load_miss(resource_id, prefetch).unwrap_or_default()
}

/// Strictly encode a single ring-buffer slot for a load-miss request.
///
/// # Errors
///
/// Returns [`ProtocolError`] when slot sizing, word indexing, or host staging
/// reservation fails.
pub fn try_encode_load_miss(resource_id: u32, prefetch: bool) -> Result<Vec<u8>, ProtocolError> {
    let total_bytes = try_slot_byte_len()?;
    let mut bytes = Vec::new();
    codec::try_reserve_protocol_capacity(
        &mut bytes,
        total_bytes,
        "slot",
        "load-miss slot encode could not reserve host staging bytes; reuse a preallocated slot buffer",
    )?;
    try_encode_load_miss_into(resource_id, prefetch, &mut bytes)?;
    Ok(bytes)
}

/// Strictly encode a load-miss slot into caller-owned storage.
///
/// Clears and resizes `dst` to exactly one protocol slot, reusing allocation.
///
/// # Errors
///
/// Returns [`ProtocolError`] when slot sizing, word indexing, or host staging
/// reservation fails.
pub fn try_encode_load_miss_into(
    resource_id: u32,
    prefetch: bool,
    dst: &mut Vec<u8>,
) -> Result<(), ProtocolError> {
    let total_bytes = try_slot_byte_len()?;
    dst.clear();
    codec::try_reserve_protocol_capacity(
        dst,
        total_bytes,
        "slot",
        "load-miss slot encode could not reserve caller-owned staging bytes; reuse a larger slot buffer",
    )?;
    dst.resize(total_bytes, 0);

    codec::write_word(dst, try_slot_word_index(0, STATUS_WORD)?, slot::PUBLISHED);
    codec::write_word(dst, try_slot_word_index(0, OPCODE_WORD)?, opcode::LOAD_MISS);
    codec::write_word(dst, try_slot_word_index(0, TENANT_WORD)?, 0);
    codec::write_word(dst, try_slot_word_index(0, PRIORITY_WORD)?, 0);
    codec::write_word(dst, try_slot_word_index(0, ARG0_WORD)?, resource_id);
    let prefetch_word = ARG0_WORD
        .checked_add(1)
        .ok_or(ProtocolError::ByteLengthOverflow {
            buffer: "slot",
            fix: "load-miss argument word overflows u32; keep ARG0_WORD within SLOT_WORDS",
        })?;
    codec::write_word(
        dst,
        try_slot_word_index(0, prefetch_word)?,
        u32::from(prefetch),
    );
    Ok(())
}

/// Decode a load-miss slot from ring-buffer bytes.
///
/// Returns `Some((resource_id, prefetch))` if the slot contains the
/// [`opcode::LOAD_MISS`] opcode. Returns `None` if the byte slice is
/// too short or the opcode does not match.
#[must_use]
pub fn decode_load_miss(ring_bytes: &[u8], slot: u32) -> Option<(u32, bool)> {
    let opcode_word = codec::read_word(ring_bytes, try_slot_word_index(slot, OPCODE_WORD).ok()?)?;
    if opcode_word != opcode::LOAD_MISS {
        return None;
    }
    let resource_id = codec::read_word(ring_bytes, try_slot_word_index(slot, ARG0_WORD).ok()?)?;
    let prefetch_word = ARG0_WORD.checked_add(1)?;
    let prefetch =
        codec::read_word(ring_bytes, try_slot_word_index(slot, prefetch_word).ok()?)? != 0;
    Some((resource_id, prefetch))
}

/// Return the byte length of one ring-buffer slot.
///
/// # Errors
///
/// Returns [`ProtocolError`] when slot sizing overflows host address space.
pub fn try_slot_byte_len() -> Result<usize, ProtocolError> {
    codec::words_to_bytes(SLOT_WORDS).ok_or(ProtocolError::ByteLengthOverflow {
        buffer: "slot",
        fix: "slot byte length overflows host address space; keep SLOT_WORDS within the host index ABI",
    })
}

/// Convert a protocol-local word index into a host index.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the word index cannot fit host address space.
pub fn try_word_index(word: u32) -> Result<usize, ProtocolError> {
    usize::try_from(word).map_err(|_| ProtocolError::ByteLengthOverflow {
        buffer: "slot",
        fix: "protocol word index cannot fit host usize; keep protocol word constants within the host index ABI",
    })
}

/// Return the first word index for `slot`.
///
/// # Errors
///
/// Returns [`ProtocolError`] when slot multiplication overflows the u32 wire
/// ABI or host address space.
pub fn try_slot_word_base(slot: u32) -> Result<usize, ProtocolError> {
    let base_words = slot
        .checked_mul(SLOT_WORDS)
        .ok_or(ProtocolError::ByteLengthOverflow {
            buffer: "ring",
            fix: "slot word base overflows the u32 protocol word ABI; shard the megakernel ring before host access",
        })?;
    try_word_index(base_words)
}

/// Return the absolute ring word index for a slot-local word.
///
/// # Errors
///
/// Returns [`ProtocolError`] when `word` is outside one slot or when addition
/// overflows host address space.
pub fn try_slot_word_index(slot: u32, word: u32) -> Result<usize, ProtocolError> {
    if word >= SLOT_WORDS {
        return Err(ProtocolError::ByteLengthOverflow {
            buffer: "slot",
            fix: "slot-local word is outside SLOT_WORDS; keep protocol constants within the slot layout",
        });
    }
    try_slot_word_base(slot)?
        .checked_add(try_word_index(word)?)
        .ok_or(ProtocolError::ByteLengthOverflow {
            buffer: "ring",
            fix: "slot word index overflows host address space; shard the megakernel ring before host access",
        })
}

/// Deprecated alias for [`encode_load_miss`]. The old MoE-specific
/// parameter name was a boundary violation  -  vyre is a generic GPU
/// substrate. New code must use [`encode_load_miss`]; this shim will
/// be removed once consumers have migrated.
#[deprecated(since = "0.5.0", note = "use `encode_load_miss`")]
#[must_use]
pub fn encode_expert_miss(resource_id: u32, prefetch: bool) -> Vec<u8> {
    encode_load_miss(resource_id, prefetch)
}

/// Deprecated alias for [`decode_load_miss`]; see [`encode_expert_miss`].
#[deprecated(since = "0.5.0", note = "use `decode_load_miss`")]
#[must_use]
pub fn decode_expert_miss(ring_bytes: &[u8], slot: u32) -> Option<(u32, bool)> {
    decode_load_miss(ring_bytes, slot)
}

#[cfg(test)]
mod tests;
