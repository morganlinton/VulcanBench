use super::{
    control, debug, slot, DebugRecord, ProtocolError, CONTROL_MIN_WORDS, MAX_DEBUG_RECORDS,
    MAX_ENCODED_DEBUG_RECORDS, MAX_ENCODED_OBSERVABLE_SLOTS, MAX_ENCODED_RING_SLOTS,
    MAX_OBSERVABLE_SLOTS, MAX_RING_SLOTS, SLOT_WORDS, STATUS_WORD,
};

/// Return the number of bytes required by a control buffer with `observable_slots`.
#[must_use]
pub fn control_byte_len(observable_slots: u32) -> Option<usize> {
    if observable_slots > MAX_OBSERVABLE_SLOTS {
        return None;
    }
    let words = control::OBSERVABLE_BASE.checked_add(observable_slots)?;
    words_to_bytes(words.max(CONTROL_MIN_WORDS))
}

/// Return the number of bytes required by a ring buffer with `slot_count` slots.
#[must_use]
pub fn ring_byte_len(slot_count: u32) -> Option<usize> {
    if slot_count > MAX_RING_SLOTS {
        return None;
    }
    let words = slot_count.checked_mul(SLOT_WORDS)?;
    words_to_bytes(words)
}

/// Return the number of bytes required by a debug-log buffer.
#[must_use]
pub fn debug_log_byte_len(record_capacity: u32) -> Option<usize> {
    if record_capacity > MAX_DEBUG_RECORDS {
        return None;
    }
    let record_words = record_capacity.checked_mul(debug::RECORD_WORDS)?;
    let words = debug::RECORDS_BASE.checked_add(record_words)?;
    words_to_bytes(words)
}

fn control_encode_capacity(observable_slots: u32) -> Result<usize, ProtocolError> {
    if observable_slots > MAX_ENCODED_OBSERVABLE_SLOTS {
        return Err(ProtocolError::ByteLengthOverflow {
            buffer: "control",
            fix: "shard observable results or reduce observable_slots to the megakernel allocation cap before encoding control",
        });
    }
    control_byte_len(observable_slots).ok_or(ProtocolError::ByteLengthOverflow {
        buffer: "control",
        fix: "shard observable results or reduce observable_slots to the megakernel protocol cap before encoding control",
    })
}

fn ring_encode_capacity(slot_count: u32) -> Result<usize, ProtocolError> {
    if slot_count > MAX_ENCODED_RING_SLOTS {
        return Err(ProtocolError::ByteLengthOverflow {
            buffer: "ring",
            fix: "split the dispatch into smaller ring shards before encoding; slot_count exceeds the megakernel allocation cap or host address space",
        });
    }
    ring_byte_len(slot_count).ok_or(ProtocolError::ByteLengthOverflow {
        buffer: "ring",
        fix: "split the dispatch into smaller ring shards before encoding; slot_count exceeds the megakernel protocol cap or host address space",
    })
}

fn debug_log_encode_capacity(record_capacity: u32) -> Result<usize, ProtocolError> {
    if record_capacity > MAX_ENCODED_DEBUG_RECORDS {
        return Err(ProtocolError::ByteLengthOverflow {
            buffer: "debug_log",
            fix:
                "reduce debug-log record_capacity to the megakernel allocation cap before encoding",
        });
    }
    debug_log_byte_len(record_capacity).ok_or(ProtocolError::ByteLengthOverflow {
        buffer: "debug_log",
        fix: "reduce debug-log record_capacity to the megakernel protocol cap before encoding",
    })
}

/// Encode a control-buffer payload.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the requested observable region overflows
/// host address space.
pub fn encode_control(
    shutdown: bool,
    tenant_count: u32,
    observable_slots: u32,
) -> Result<Vec<u8>, ProtocolError> {
    try_encode_control(shutdown, tenant_count, observable_slots)
}

/// Strictly encode a control-buffer payload.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the requested observable region overflows
/// host address space.
pub fn try_encode_control(
    shutdown: bool,
    tenant_count: u32,
    observable_slots: u32,
) -> Result<Vec<u8>, ProtocolError> {
    let total_bytes = control_encode_capacity(observable_slots)?;
    let mut bytes = Vec::new();
    try_reserve_protocol_capacity(
        &mut bytes,
        total_bytes,
        "control",
        "control encode could not reserve host staging bytes; reduce observable_slots or reuse a preallocated control buffer",
    )?;
    try_encode_control_into(shutdown, tenant_count, observable_slots, &mut bytes)?;
    Ok(bytes)
}

/// Strictly encode a control-buffer payload into caller-owned storage.
///
/// Clears and resizes `dst` to the exact control-buffer byte length, reusing
/// any existing allocation.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the requested observable region overflows
/// host address space.
pub fn try_encode_control_into(
    shutdown: bool,
    tenant_count: u32,
    observable_slots: u32,
    dst: &mut Vec<u8>,
) -> Result<(), ProtocolError> {
    let total_bytes = control_encode_capacity(observable_slots)?;
    dst.clear();
    try_reserve_protocol_capacity(
        dst,
        total_bytes,
        "control",
        "control encode could not reserve caller-owned staging bytes; reduce observable_slots or reuse a larger control buffer",
    )?;
    dst.resize(total_bytes, 0);

    if shutdown {
        write_word(
            dst,
            control_word_index(control::SHUTDOWN, "shutdown word")?,
            1,
        );
    }
    write_word(
        dst,
        control_word_index(control::TENANT_BASE, "tenant base word")?,
        control::TENANT_BASE + 1,
    );

    let tenant_table_start = control_word_index(control::TENANT_BASE, "tenant base word")?
        .checked_add(1)
        .ok_or(ProtocolError::ByteLengthOverflow {
            buffer: "control",
            fix: "tenant table start overflowed usize; reduce control protocol constants",
        })?;
    let requested_tenant_words =
        usize::try_from(tenant_count).map_err(|_| ProtocolError::ByteLengthOverflow {
            buffer: "control",
            fix: "tenant_count cannot fit host usize; split tenant tables before encoding",
        })?;
    let tenant_table_end = core::cmp::min(
        tenant_table_start
            .checked_add(requested_tenant_words)
            .ok_or(ProtocolError::ByteLengthOverflow {
                buffer: "control",
                fix: "tenant table end overflowed usize; split tenant tables before encoding",
            })?,
        control_word_index(control::TENANT_QUOTA_BASE, "tenant quota base word")?,
    );
    for word_idx in tenant_table_start..tenant_table_end {
        write_word(dst, word_idx, !0u32);
    }

    let quota_table_start =
        control_word_index(control::TENANT_QUOTA_BASE, "tenant quota base word")?;
    let quota_table_end = core::cmp::min(
        quota_table_start
            .checked_add(requested_tenant_words)
            .ok_or(ProtocolError::ByteLengthOverflow {
                buffer: "control",
                fix: "quota table end overflowed usize; split tenant tables before encoding",
            })?,
        control_word_index(control::TENANT_FAIRNESS_BASE, "tenant fairness base word")?,
    );
    for word_idx in quota_table_start..quota_table_end {
        write_word(dst, word_idx, 1_000_000);
    }
    Ok(())
}

/// Encode an empty ring buffer with `slot_count` slots.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the requested ring size overflows host
/// address space.
pub fn encode_empty_ring(slot_count: u32) -> Result<Vec<u8>, ProtocolError> {
    try_encode_empty_ring(slot_count)
}

/// Strictly encode an empty ring buffer with `slot_count` slots.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the requested ring size overflows host
/// address space.
pub fn try_encode_empty_ring(slot_count: u32) -> Result<Vec<u8>, ProtocolError> {
    let total_bytes = ring_encode_capacity(slot_count)?;
    let mut bytes = Vec::new();
    try_reserve_protocol_capacity(
        &mut bytes,
        total_bytes,
        "ring",
        "ring encode could not reserve host staging bytes; split the dispatch into smaller ring shards or reuse a preallocated ring buffer",
    )?;
    try_encode_empty_ring_into(slot_count, &mut bytes)?;
    Ok(bytes)
}

/// Strictly encode an empty ring buffer into caller-owned storage.
///
/// Clears and resizes `dst` to the exact ring byte length, reusing allocation.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the requested ring size overflows host
/// address space.
pub fn try_encode_empty_ring_into(slot_count: u32, dst: &mut Vec<u8>) -> Result<(), ProtocolError> {
    let total_bytes = ring_encode_capacity(slot_count)?;
    dst.clear();
    try_reserve_protocol_capacity(
        dst,
        total_bytes,
        "ring",
        "ring encode could not reserve caller-owned staging bytes; split the dispatch into smaller ring shards or reuse a larger ring buffer",
    )?;
    dst.resize(total_bytes, 0);
    Ok(())
}

/// Encode an empty PRINTF channel buffer.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the requested debug-log size overflows host
/// address space.
pub fn encode_empty_debug_log(record_capacity: u32) -> Result<Vec<u8>, ProtocolError> {
    try_encode_empty_debug_log(record_capacity)
}

/// Strictly encode an empty PRINTF channel buffer.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the requested debug-log size overflows host
/// address space.
pub fn try_encode_empty_debug_log(record_capacity: u32) -> Result<Vec<u8>, ProtocolError> {
    let total_bytes = debug_log_encode_capacity(record_capacity)?;
    let mut bytes = Vec::new();
    try_reserve_protocol_capacity(
        &mut bytes,
        total_bytes,
        "debug_log",
        "debug-log encode could not reserve host staging bytes; reduce record_capacity or reuse a preallocated debug-log buffer",
    )?;
    try_encode_empty_debug_log_into(record_capacity, &mut bytes)?;
    Ok(bytes)
}

/// Strictly encode an empty PRINTF channel buffer into caller-owned storage.
///
/// Clears and resizes `dst` to the exact debug-log byte length, reusing allocation.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the requested debug-log size overflows host
/// address space.
pub fn try_encode_empty_debug_log_into(
    record_capacity: u32,
    dst: &mut Vec<u8>,
) -> Result<(), ProtocolError> {
    let total_bytes = debug_log_encode_capacity(record_capacity)?;
    dst.clear();
    try_reserve_protocol_capacity(
        dst,
        total_bytes,
        "debug_log",
        "debug-log encode could not reserve caller-owned staging bytes; reduce record_capacity or reuse a larger debug-log buffer",
    )?;
    dst.resize(total_bytes, 0);
    Ok(())
}

/// Decode the kernel's `done_count` from a control buffer.
///
/// On a malformed or truncated control buffer this returns `0` **and emits a
/// `tracing::error!`** so the stall is operator-visible. Prefer
/// [`try_read_done_count`] in new code — a silent `0` is indistinguishable from
/// the initial kernel state and can cause the host pump to stall indefinitely.
#[must_use]
pub fn read_done_count(control_bytes: &[u8]) -> u32 {
    match try_read_done_count(control_bytes) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(
                error = %e,
                buf_len = control_bytes.len(),
                "read_done_count: malformed control buffer — returning 0 instead of real count. Fix: ensure the DMA readback covers the full control buffer produced by the matching encoder."
            );
            0
        }
    }
}

/// Read the epoch counter from a control buffer.
///
/// On a malformed or truncated control buffer this returns `0` **and emits a
/// `tracing::error!`** so the stall is operator-visible. Prefer
/// [`try_read_epoch`] in new code — a silent `0` lets the host pump conclude no
/// batch has completed, stalling dispatch indefinitely.
#[must_use]
pub fn read_epoch(control_bytes: &[u8]) -> u32 {
    match try_read_epoch(control_bytes) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(
                error = %e,
                buf_len = control_bytes.len(),
                "read_epoch: malformed control buffer — returning 0 instead of real epoch. Fix: ensure the DMA readback covers the full control buffer produced by the matching encoder."
            );
            0
        }
    }
}

/// Strictly decode the kernel's `done_count` from a control buffer.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the buffer is not word-aligned or is too
/// short to contain the fixed control header.
pub fn try_read_done_count(control_bytes: &[u8]) -> Result<u32, ProtocolError> {
    read_required_word(
        "control",
        control_bytes,
        control_word_index(control::DONE_COUNT, "done-count word")?,
    )
}

/// Strictly decode the epoch counter from a control buffer.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the buffer is not word-aligned or is too
/// short to contain the epoch word.
pub fn try_read_epoch(control_bytes: &[u8]) -> Result<u32, ProtocolError> {
    read_required_word(
        "control",
        control_bytes,
        control_word_index(control::EPOCH, "epoch word")?,
    )
}

/// Read an observable result word from a control buffer.
///
/// On a malformed or truncated control buffer this returns `0` **and emits a
/// `tracing::error!`** so the miss is operator-visible. Prefer
/// [`try_read_observable`] in new code — a silent `0` is indistinguishable from
/// an observable slot that was never written.
#[must_use]
pub fn read_observable(control_bytes: &[u8], index: u32) -> u32 {
    match try_read_observable(control_bytes, index) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(
                error = %e,
                buf_len = control_bytes.len(),
                index,
                "read_observable: malformed control buffer — returning 0 instead of real observable. Fix: ensure the DMA readback covers the full control buffer produced by the matching encoder."
            );
            0
        }
    }
}

/// Strictly read an observable result word from a control buffer.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the buffer is not word-aligned, the index
/// overflows the observable word offset, or the word is outside the buffer.
pub fn try_read_observable(control_bytes: &[u8], index: u32) -> Result<u32, ProtocolError> {
    let word_idx = control_word_index(
        control::OBSERVABLE_BASE
            .checked_add(index)
            .ok_or(ProtocolError::ByteLengthOverflow {
                buffer: "control",
                fix: "observable index overflows the protocol word offset; shard observable reads",
            })?,
        "observable word index",
    )?;
    read_required_word("control", control_bytes, word_idx)
}

/// Read per-opcode metrics counters from a control buffer.
#[must_use]
pub fn read_metrics(control_bytes: &[u8]) -> Vec<(u32, u32)> {
    let mut result = Vec::new();
    read_metrics_into(control_bytes, &mut result);
    result
}

/// Read per-opcode metrics counters into caller-owned storage.
///
/// Clears `out`, then reuses its allocation.
///
/// On an allocation failure this returns with `out` cleared **and emits a
/// `tracing::error!`** so the empty result is operator-visible. An empty `out`
/// on non-empty control bytes therefore always indicates either (a) no non-zero
/// counters or (b) an OOM that was surfaced loudly. Prefer
/// [`try_read_metrics_into`] in code paths where the caller can propagate errors.
pub fn read_metrics_into(control_bytes: &[u8], out: &mut Vec<(u32, u32)>) {
    out.clear();
    let Ok(metrics_base) = control_word_index(control::METRICS_BASE, "metrics base word") else {
        return;
    };
    let available_words = control_bytes.len() / 4;
    if available_words <= metrics_base {
        return;
    }
    let available_slots = (available_words - metrics_base).min(control::METRICS_SLOTS as usize);
    let nonzero = count_nonzero_metrics_truncated(control_bytes, metrics_base, available_slots);
    if let Err(e) = try_reserve_target_capacity(out, nonzero) {
        tracing::error!(
            error = %e,
            nonzero_count = nonzero,
            "read_metrics_into: allocation failed — returning empty metrics. Fix: reduce metrics fanout or decode into a pre-allocated scratch vector."
        );
        return;
    }
    for slot in 0..available_slots {
        let word_idx = metrics_base + slot;
        let Some(count) = read_word_unaligned(control_bytes, word_idx) else {
            break;
        };
        if count > 0 {
            out.push((slot as u32, count));
        }
    }
}

/// Strictly read per-opcode metrics counters from a control buffer.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the buffer is not word-aligned or is too
/// short for the fixed metrics window.
pub fn try_read_metrics(control_bytes: &[u8]) -> Result<Vec<(u32, u32)>, ProtocolError> {
    let mut result = Vec::new();
    try_read_metrics_into(control_bytes, &mut result)?;
    Ok(result)
}

/// Strictly read per-opcode metrics counters into caller-owned storage.
///
/// Clears `out`, then reuses its allocation.
///
/// # Errors
///
/// Returns [`ProtocolError`] when the buffer is not word-aligned or is too
/// short for the fixed metrics window.
pub fn try_read_metrics_into(
    control_bytes: &[u8],
    out: &mut Vec<(u32, u32)>,
) -> Result<(), ProtocolError> {
    validate_word_aligned("control", control_bytes)?;
    out.clear();
    if let Ok(words) = bytemuck::try_cast_slice::<u8, u32>(control_bytes) {
        try_reserve_target_capacity(
            out,
            count_nonzero_metrics_words_strict(words, control_bytes.len())?,
        )?;
        for i in 0..control::METRICS_SLOTS {
            let word_idx = metrics_word_index(i)?;
            let count =
                words
                    .get(word_idx)
                    .copied()
                    .map(u32::from_le)
                    .ok_or(ProtocolError::MissingWord {
                        buffer: "control",
                        word_idx,
                        byte_len: control_bytes.len(),
                        fix: "decode only control buffers produced by the matching megakernel protocol encoder",
                    })?;
            if count > 0 {
                out.push((i, count));
            }
        }
        return Ok(());
    }
    try_reserve_target_capacity(out, count_nonzero_metrics_unaligned_strict(control_bytes)?)?;
    for i in 0..control::METRICS_SLOTS {
        let word_idx = metrics_word_index(i)?;
        let count = read_word_unaligned(control_bytes, word_idx)
            .ok_or(ProtocolError::MissingWord {
            buffer: "control",
            word_idx,
            byte_len: control_bytes.len(),
            fix: "decode only control buffers produced by the matching megakernel protocol encoder",
        })?;
        if count > 0 {
            out.push((i, count));
        }
    }
    Ok(())
}

mod debug_log;

pub use debug_log::{
    read_debug_log, read_debug_log_into, try_read_debug_log, try_read_debug_log_into,
};

/// Count DONE slots in a ring-buffer readback.
///
/// Returns `None` when the supplied bytes cannot contain `item_count` whole
/// slots. This is intentionally part of the protocol module: DONE status is an
/// ABI word, not a backend-specific readback rule.
#[must_use]
pub fn count_done_ring_slots(ring_bytes: &[u8], item_count: usize) -> Option<u64> {
    if item_count == 0 {
        return None;
    }
    let slot_words = usize::try_from(SLOT_WORDS).ok()?;
    let required_bytes = item_count.checked_mul(slot_words)?.checked_mul(4)?;
    if ring_bytes.len() < required_bytes {
        return None;
    }
    let status_word = usize::try_from(STATUS_WORD).ok()?;
    let words = bytemuck::try_cast_slice::<u8, u32>(ring_bytes).ok();
    let done = (0..item_count)
        .filter(|slot_idx| {
            let word_idx = slot_idx
                .checked_mul(slot_words)
                .and_then(|base| base.checked_add(status_word));
            word_idx.and_then(|idx| read_word_from_optional_words(words, ring_bytes, idx))
                == Some(slot::DONE)
        })
        .count();
    u64::try_from(done).ok()
}

/// Strictly count DONE slots in a ring-buffer readback.
///
/// # Errors
///
/// Returns [`ProtocolError`] when `ring_bytes` cannot contain `item_count`
/// complete ring slots or when the byte count overflows the host protocol
/// domain.
pub fn try_count_done_ring_slots(
    ring_bytes: &[u8],
    item_count: usize,
) -> Result<u64, ProtocolError> {
    if item_count == 0 {
        return Ok(0);
    }
    validate_word_aligned("ring", ring_bytes)?;
    let slot_words =
        usize::try_from(SLOT_WORDS).map_err(|_| ProtocolError::ByteLengthOverflow {
            buffer: "ring",
            fix: "keep SLOT_WORDS representable in host usize before decoding ring status",
        })?;
    let required_bytes = item_count
        .checked_mul(slot_words)
        .and_then(|words| words.checked_mul(4))
        .ok_or(ProtocolError::ByteLengthOverflow {
            buffer: "ring",
            fix: "split the dispatch before ring status decode overflows host address space",
        })?;
    if ring_bytes.len() < required_bytes {
        return Err(ProtocolError::MissingWord {
            buffer: "ring",
            word_idx: required_bytes / 4,
            byte_len: ring_bytes.len(),
            fix: "decode only full ring readbacks sized for the submitted megakernel item_count",
        });
    }
    let status_word =
        usize::try_from(STATUS_WORD).map_err(|_| ProtocolError::ByteLengthOverflow {
            buffer: "ring",
            fix: "keep STATUS_WORD representable in host usize before decoding ring status",
        })?;
    let words = bytemuck::try_cast_slice::<u8, u32>(ring_bytes).ok();
    let mut done = 0_u64;
    for slot_idx in 0..item_count {
        let word_idx = slot_idx
            .checked_mul(slot_words)
            .and_then(|base| base.checked_add(status_word))
            .ok_or(ProtocolError::ByteLengthOverflow {
                buffer: "ring",
                fix: "split the dispatch before ring status word indexing overflows host address space",
            })?;
        if read_word_from_optional_words(words, ring_bytes, word_idx) == Some(slot::DONE) {
            done = done
                .checked_add(1)
                .ok_or(ProtocolError::ByteLengthOverflow {
                    buffer: "ring",
                    fix: "split the dispatch before DONE slot count exceeds u64",
                })?;
        }
    }
    Ok(done)
}

fn try_reserve_target_capacity<T>(
    out: &mut Vec<T>,
    target_capacity: usize,
) -> Result<(), ProtocolError> {
    try_reserve_protocol_capacity(
        out,
        target_capacity,
        "control",
        "host metrics decode could not reserve output records; reduce metrics fanout or decode into a reused scratch vector",
    )
}

pub(super) fn try_reserve_protocol_capacity<T>(
    out: &mut Vec<T>,
    target_capacity: usize,
    buffer: &'static str,
    fix: &'static str,
) -> Result<(), ProtocolError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(out, target_capacity)
        .map_err(|_| ProtocolError::ByteLengthOverflow { buffer, fix })
}

fn count_nonzero_metrics_words_strict(
    words: &[u32],
    byte_len: usize,
) -> Result<usize, ProtocolError> {
    let mut count = 0usize;
    for i in 0..control::METRICS_SLOTS {
        let word_idx = metrics_word_index(i)?;
        let word = words
            .get(word_idx)
            .copied()
            .map(u32::from_le)
            .ok_or(ProtocolError::MissingWord {
            buffer: "control",
            word_idx,
            byte_len,
            fix: "decode only control buffers produced by the matching megakernel protocol encoder",
        })?;
        if word > 0 {
            count += 1;
        }
    }
    Ok(count)
}

fn count_nonzero_metrics_unaligned_strict(control_bytes: &[u8]) -> Result<usize, ProtocolError> {
    let mut count = 0usize;
    for i in 0..control::METRICS_SLOTS {
        let word_idx = metrics_word_index(i)?;
        let word = read_word_unaligned(control_bytes, word_idx)
            .ok_or(ProtocolError::MissingWord {
            buffer: "control",
            word_idx,
            byte_len: control_bytes.len(),
            fix: "decode only control buffers produced by the matching megakernel protocol encoder",
        })?;
        if word > 0 {
            count += 1;
        }
    }
    Ok(count)
}

fn count_nonzero_metrics_truncated(
    control_bytes: &[u8],
    metrics_base: usize,
    available_slots: usize,
) -> usize {
    let mut count = 0usize;
    for slot in 0..available_slots {
        if read_word_unaligned(control_bytes, metrics_base + slot).unwrap_or(0) > 0 {
            count += 1;
        }
    }
    count
}

fn metrics_word_index(slot: u32) -> Result<usize, ProtocolError> {
    let word =
        control::METRICS_BASE
            .checked_add(slot)
            .ok_or(ProtocolError::ByteLengthOverflow {
                buffer: "control",
                fix: "metrics slot index overflows the protocol word offset; shard metrics reads",
            })?;
    control_word_index(word, "metrics word index")
}

fn control_word_index(word: u32, label: &'static str) -> Result<usize, ProtocolError> {
    usize::try_from(word).map_err(|_| ProtocolError::ByteLengthOverflow {
        buffer: "control",
        fix: match label {
            "observable word index" => {
                "observable word index cannot fit host usize; shard observable reads"
            }
            "metrics word index" => "metrics word index cannot fit host usize; shard metrics reads",
            _ => "control word index cannot fit host usize; shard protocol reads",
        },
    })
}

pub(crate) fn read_word(bytes: &[u8], word_idx: usize) -> Option<u32> {
    if let Ok(words) = bytemuck::try_cast_slice::<u8, u32>(bytes) {
        return words.get(word_idx).copied().map(u32::from_le);
    }
    read_word_unaligned(bytes, word_idx)
}

fn read_word_from_optional_words(
    words: Option<&[u32]>,
    bytes: &[u8],
    word_idx: usize,
) -> Option<u32> {
    if let Some(words) = words {
        return words.get(word_idx).copied().map(u32::from_le);
    }
    read_word_unaligned(bytes, word_idx)
}

fn read_word_unaligned(bytes: &[u8], word_idx: usize) -> Option<u32> {
    let off = word_idx.checked_mul(4)?;
    let end = off.checked_add(4)?;
    let word = bytes.get(off..end)?;
    Some(u32::from_le_bytes(word.try_into().ok()?))
}

fn read_required_word(
    buffer: &'static str,
    bytes: &[u8],
    word_idx: usize,
) -> Result<u32, ProtocolError> {
    validate_word_aligned(buffer, bytes)?;
    read_word(bytes, word_idx).ok_or(ProtocolError::MissingWord {
        buffer,
        word_idx,
        byte_len: bytes.len(),
        fix: "decode only buffers produced by the matching megakernel protocol encoder",
    })
}

fn validate_word_aligned(buffer: &'static str, bytes: &[u8]) -> Result<(), ProtocolError> {
    if bytes.len() % 4 == 0 {
        Ok(())
    } else {
        Err(ProtocolError::MisalignedByteLength {
            buffer,
            byte_len: bytes.len(),
            fix: "pass whole u32 protocol words; do not decode partial DMA/readback buffers",
        })
    }
}

pub(crate) fn write_word(bytes: &mut [u8], word_idx: usize, value: u32) {
    let off = word_idx * 4;
    bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn words_to_bytes(words: u32) -> Option<usize> {
    usize::try_from(words).ok()?.checked_mul(4)
}
