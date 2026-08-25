//! Poll/claim helpers. Read REQUEST slots out of the queue and (in the
//! claim variant) atomically transition them to CLAIMED.

use std::sync::atomic::{fence, Ordering};

use crate::PipelineError;

use super::super::protocol::slot;
use super::helpers::{
    read_queue_word, try_queue_word_index, validate_io_queue_view, write_queue_word, IoQueueView,
};
use super::{io_word, IoRequest};

/// contains a partial IO slot, or exceeds the compiled poll window.
pub fn try_poll_io_requests(io_queue_bytes: &[u8]) -> Result<Vec<IoRequest>, PipelineError> {
    let view = validate_io_queue_view(io_queue_bytes.len())?;
    let mut requests = Vec::new();
    try_poll_io_requests_into_validated(io_queue_bytes, view, &mut requests)?;
    Ok(requests)
}

/// Strictly poll pending requests into caller-owned storage without claiming.
///
/// # Errors
///
/// Returns [`PipelineError`] when the byte view is malformed or exceeds the
/// compiled poll window.
pub fn try_poll_io_requests_into(
    io_queue_bytes: &[u8],
    requests: &mut Vec<IoRequest>,
) -> Result<(), PipelineError> {
    let view = validate_io_queue_view(io_queue_bytes.len())?;
    try_poll_io_requests_into_validated(io_queue_bytes, view, requests)
}

fn try_poll_io_requests_into_validated(
    io_queue_bytes: &[u8],
    view: IoQueueView,
    requests: &mut Vec<IoRequest>,
) -> Result<(), PipelineError> {
    requests.clear();
    if let Ok(words) = bytemuck::try_cast_slice::<u8, u32>(io_queue_bytes) {
        reserve_target_capacity(requests, count_published_words(words, view)?)?;
        poll_io_requests_words(words, view, requests)?;
        return Ok(());
    }
    reserve_target_capacity(requests, count_published_unaligned(io_queue_bytes, view)?)?;
    for slot_idx in 0..view.slot_count {
        let slot_idx_u32 = slot_index_u32(slot_idx)?;
        let base = slot_base_word(slot_idx)?;
        let read_word = |offset: u32| -> Result<u32, PipelineError> {
            let word_offset = usize::try_from(offset).map_err(|error| {
                PipelineError::Backend(format!(
                    "IO queue word offset cannot fit usize: {error}. Fix: keep IO word offsets within the host index ABI."
                ))
            })?;
            let off = base
                .checked_add(word_offset)
                .and_then(|word| word.checked_mul(4))
                .ok_or_else(|| {
                    PipelineError::Backend(format!(
                        "IO queue slot {slot_idx} word {offset} byte offset overflowed. Fix: validate queue byte length before polling."
                    ))
                })?;
            let end = off.checked_add(4).ok_or_else(|| {
                PipelineError::Backend(format!(
                    "IO queue slot {slot_idx} word {offset} byte end overflowed. Fix: validate queue byte length before polling."
                ))
            })?;
            let bytes = io_queue_bytes.get(off..end).ok_or_else(|| {
                PipelineError::Backend(format!(
                    "IO queue slot {slot_idx} word {offset} is outside the validated queue view. Fix: validate queue byte length before polling."
                ))
            })?;
            let mut word = [0u8; 4];
            word.copy_from_slice(bytes);
            fence(Ordering::Acquire);
            Ok(u32::from_le_bytes(word))
        };

        let status = read_word(io_word::STATUS)?;
        if status == slot::PUBLISHED {
            let offset_lo = read_word(io_word::OFFSET_LO)?;
            let offset_hi = read_word(io_word::OFFSET_HI)?;
            requests.push(IoRequest {
                slot_idx: slot_idx_u32,
                op_type: read_word(io_word::OP_TYPE)?,
                src_handle: read_word(io_word::SRC_HANDLE)?,
                dst_handle: read_word(io_word::DST_HANDLE)?,
                offset: combine_offset(offset_hi, offset_lo),
                byte_count: read_word(io_word::BYTE_COUNT)?,
                tag: read_word(io_word::TAG)?,
            });
        }
    }

    Ok(())
}

/// Strictly poll and claim pending requests into caller-owned storage.
///
/// Unlike [`try_poll_io_requests`], this mutates each `PUBLISHED` slot to
/// `CLAIMED` before returning it. Host IO pumps must use this entry point so a
/// still-in-flight request is not submitted again on the next poll.
///
/// # Errors
///
/// Returns [`PipelineError`] when the byte view is malformed or exceeds the
/// compiled poll window.
pub fn try_claim_io_requests_into(
    io_queue_bytes: &mut [u8],
    requests: &mut Vec<IoRequest>,
) -> Result<(), PipelineError> {
    let view = validate_io_queue_view(io_queue_bytes.len())?;
    requests.clear();
    if let Ok(words) = bytemuck::try_cast_slice_mut::<u8, u32>(io_queue_bytes) {
        reserve_target_capacity(requests, count_published_words(words, view)?)?;
        claim_io_requests_words(words, view, requests)?;
        return Ok(());
    }
    reserve_target_capacity(requests, count_published_unaligned(io_queue_bytes, view)?)?;

    for slot_idx in 0..view.slot_count {
        let slot_idx_u32 = slot_index_u32(slot_idx)?;
        let base = slot_base_word(slot_idx)?;
        let status = read_queue_word(io_queue_bytes, base, io_word::STATUS)?;
        if status != slot::PUBLISHED {
            continue;
        }

        write_queue_word(io_queue_bytes, base, io_word::STATUS, slot::CLAIMED)?;
        let offset_lo = read_queue_word(io_queue_bytes, base, io_word::OFFSET_LO)?;
        let offset_hi = read_queue_word(io_queue_bytes, base, io_word::OFFSET_HI)?;
        requests.push(IoRequest {
            slot_idx: slot_idx_u32,
            op_type: read_queue_word(io_queue_bytes, base, io_word::OP_TYPE)?,
            src_handle: read_queue_word(io_queue_bytes, base, io_word::SRC_HANDLE)?,
            dst_handle: read_queue_word(io_queue_bytes, base, io_word::DST_HANDLE)?,
            offset: combine_offset(offset_hi, offset_lo),
            byte_count: read_queue_word(io_queue_bytes, base, io_word::BYTE_COUNT)?,
            tag: read_queue_word(io_queue_bytes, base, io_word::TAG)?,
        });
    }

    Ok(())
}

/// Poll and claim pending requests into caller-owned storage.
///
/// # Errors
///
/// Returns [`PipelineError`] when the byte view is malformed or exceeds the
/// compiled poll window.
pub fn claim_io_requests_into(
    io_queue_bytes: &mut [u8],
    requests: &mut Vec<IoRequest>,
) -> Result<(), PipelineError> {
    try_claim_io_requests_into(io_queue_bytes, requests)
}

/// Public alias for [`try_poll_io_requests`] (legacy name kept for compatibility).
///
/// # Errors
/// See [`try_poll_io_requests`].
pub fn poll_io_requests(io_queue_bytes: &[u8]) -> Result<Vec<IoRequest>, PipelineError> {
    try_poll_io_requests(io_queue_bytes)
}
fn poll_io_requests_words(
    words: &[u32],
    view: IoQueueView,
    requests: &mut Vec<IoRequest>,
) -> Result<(), PipelineError> {
    for slot_idx in 0..view.slot_count {
        let slot_idx_u32 = slot_index_u32(slot_idx)?;
        let base = slot_base_word(slot_idx)?;
        let status = read_aligned_queue_word(words, base, io_word::STATUS)?;
        if status == slot::PUBLISHED {
            fence(Ordering::Acquire);
            let offset_lo = read_aligned_queue_word(words, base, io_word::OFFSET_LO)?;
            let offset_hi = read_aligned_queue_word(words, base, io_word::OFFSET_HI)?;
            requests.push(IoRequest {
                slot_idx: slot_idx_u32,
                op_type: read_aligned_queue_word(words, base, io_word::OP_TYPE)?,
                src_handle: read_aligned_queue_word(words, base, io_word::SRC_HANDLE)?,
                dst_handle: read_aligned_queue_word(words, base, io_word::DST_HANDLE)?,
                offset: combine_offset(offset_hi, offset_lo),
                byte_count: read_aligned_queue_word(words, base, io_word::BYTE_COUNT)?,
                tag: read_aligned_queue_word(words, base, io_word::TAG)?,
            });
        }
    }
    Ok(())
}

fn claim_io_requests_words(
    words: &mut [u32],
    view: IoQueueView,
    requests: &mut Vec<IoRequest>,
) -> Result<(), PipelineError> {
    for slot_idx in 0..view.slot_count {
        let slot_idx_u32 = slot_index_u32(slot_idx)?;
        let base = slot_base_word(slot_idx)?;
        let status = read_aligned_queue_word(words, base, io_word::STATUS)?;
        if status != slot::PUBLISHED {
            continue;
        }
        fence(Ordering::Acquire);
        let status_index = queue_word(base, io_word::STATUS)?;
        *words.get_mut(status_index).ok_or_else(|| {
            PipelineError::Backend(format!(
                "IO queue aligned status word index {status_index} is outside the validated queue view. Fix: validate queue byte length before claiming."
            ))
        })? = slot::CLAIMED.to_le();
        fence(Ordering::Release);
        let offset_lo = read_aligned_queue_word(words, base, io_word::OFFSET_LO)?;
        let offset_hi = read_aligned_queue_word(words, base, io_word::OFFSET_HI)?;
        requests.push(IoRequest {
            slot_idx: slot_idx_u32,
            op_type: read_aligned_queue_word(words, base, io_word::OP_TYPE)?,
            src_handle: read_aligned_queue_word(words, base, io_word::SRC_HANDLE)?,
            dst_handle: read_aligned_queue_word(words, base, io_word::DST_HANDLE)?,
            offset: combine_offset(offset_hi, offset_lo),
            byte_count: read_aligned_queue_word(words, base, io_word::BYTE_COUNT)?,
            tag: read_aligned_queue_word(words, base, io_word::TAG)?,
        });
    }
    Ok(())
}

fn count_published_words(words: &[u32], view: IoQueueView) -> Result<usize, PipelineError> {
    let mut published = 0usize;
    for slot_idx in 0..view.slot_count {
        let base = slot_base_word(slot_idx)?;
        if read_aligned_queue_word(words, base, io_word::STATUS)? == slot::PUBLISHED {
            published = published.checked_add(1).ok_or_else(|| {
                PipelineError::Backend(
                    "IO queue published-request count overflowed usize. Fix: shard the IO queue before polling."
                        .to_string(),
                )
            })?;
        }
    }
    Ok(published)
}

fn count_published_unaligned(
    io_queue_bytes: &[u8],
    view: IoQueueView,
) -> Result<usize, PipelineError> {
    let mut published = 0usize;
    for slot_idx in 0..view.slot_count {
        let base = slot_base_word(slot_idx)?;
        if read_queue_word(io_queue_bytes, base, io_word::STATUS)? == slot::PUBLISHED {
            published = published.checked_add(1).ok_or_else(|| {
                PipelineError::Backend(
                    "IO queue published-request count overflowed usize. Fix: shard the IO queue before polling."
                        .to_string(),
                )
            })?;
        }
    }
    Ok(published)
}

fn slot_index_u32(slot_idx: usize) -> Result<u32, PipelineError> {
    u32::try_from(slot_idx).map_err(|error| {
        PipelineError::Backend(format!(
            "IO queue slot index {slot_idx} cannot fit u32: {error}. Fix: shard the IO queue before polling."
        ))
    })
}

fn slot_base_word(slot_idx: usize) -> Result<usize, PipelineError> {
    try_queue_word_index(slot_index_u32(slot_idx)?, 0)
}

fn queue_word(base: usize, word: u32) -> Result<usize, PipelineError> {
    let word = usize::try_from(word).map_err(|error| {
        PipelineError::Backend(format!(
            "IO queue word offset cannot fit usize: {error}. Fix: keep IO word offsets within the host index ABI."
        ))
    })?;
    base.checked_add(word).ok_or_else(|| {
        PipelineError::Backend(format!(
            "IO queue aligned word index overflowed at base {base}. Fix: shard the IO queue before polling."
        ))
    })
}

fn read_aligned_queue_word(words: &[u32], base: usize, word: u32) -> Result<u32, PipelineError> {
    let index = queue_word(base, word)?;
    words.get(index).copied().map(u32::from_le).ok_or_else(|| {
        PipelineError::Backend(format!(
            "IO queue aligned word index {index} is outside the validated queue view. Fix: validate queue byte length before polling."
        ))
    })
}

fn combine_offset(offset_hi: u32, offset_lo: u32) -> u64 {
    (u64::from(offset_hi) << 32) | u64::from(offset_lo)
}

fn reserve_target_capacity<T>(
    out: &mut Vec<T>,
    target_capacity: usize,
) -> Result<(), PipelineError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(out, target_capacity).map_err(|_| {
        PipelineError::QueueFull {
            queue: "io_poll_requests",
            fix: "host IO polling could not reserve request records; reduce IO_SLOT_COUNT or drain the megakernel IO queue more frequently",
        }
    })
}
