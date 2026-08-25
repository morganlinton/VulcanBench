//! Encode/validate raw queue bytes.

use crate::PipelineError;

use super::helpers::validate_io_queue_view;
use super::{IO_SLOT_COUNT, IO_SLOT_WORDS};

/// contains a partial IO slot, or exceeds the compiled poll window.
pub fn validate_io_queue_bytes(io_queue_bytes: &[u8]) -> Result<(), PipelineError> {
    validate_io_queue_view(io_queue_bytes.len()).map(|_| ())
}

/// Strictly encode an empty IO queue buffer.
///
/// # Errors
///
/// Returns [`PipelineError::QueueFull`] when `slot_count` is zero, exceeds
/// the compiled megakernel poll window, or overflows the host byte length.
pub fn try_encode_empty_io_queue(slot_count: u32) -> Result<Vec<u8>, PipelineError> {
    let byte_count = empty_io_queue_byte_len(slot_count)?;
    let mut out = Vec::new();
    reserve_io_queue_bytes(&mut out, byte_count)?;
    out.resize(byte_count, 0);
    Ok(out)
}

/// Strictly encode an empty IO queue buffer into caller-owned storage.
///
/// # Errors
///
/// Returns [`PipelineError::QueueFull`] when `slot_count` is zero, exceeds
/// the compiled megakernel poll window, or overflows the host byte length.
pub fn try_encode_empty_io_queue_into(
    slot_count: u32,
    dst: &mut Vec<u8>,
) -> Result<(), PipelineError> {
    let byte_count = empty_io_queue_byte_len(slot_count)?;
    dst.clear();
    reserve_io_queue_bytes(dst, byte_count)?;
    dst.resize(byte_count, 0);
    Ok(())
}

fn reserve_io_queue_bytes(dst: &mut Vec<u8>, byte_count: usize) -> Result<(), PipelineError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(dst, byte_count).map_err(|source| {
        PipelineError::Backend(format!(
            "megakernel io_queue byte reservation failed for {byte_count} bytes: {source}. Fix: shard IO queue encoding or reuse a larger caller-owned queue buffer."
        ))
    })
}

pub(crate) fn empty_io_queue_byte_len(slot_count: u32) -> Result<usize, PipelineError> {
    if slot_count == 0 {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "io_queue requires at least one slot",
        });
    }
    if slot_count > IO_SLOT_COUNT {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "io_queue exceeds the compiled IO poll window of 64 slots; enlarge IO_SLOT_COUNT and rebuild the megakernel before encoding a larger queue",
        });
    }
    let word_count = slot_count
        .checked_mul(IO_SLOT_WORDS)
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "io_queue word count overflows u32; shard the queue before encoding",
        })?;
    usize::try_from(word_count)
        .ok()
        .and_then(|words| words.checked_mul(4))
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "io_queue byte count overflows usize; shard the queue before encoding",
        })
}

/// Encode an empty IO queue buffer.
///
/// # Errors
///
/// Returns [`PipelineError::QueueFull`] when `slot_count` is zero, exceeds
/// the compiled megakernel poll window, or overflows the host byte length.
pub fn encode_empty_io_queue(slot_count: u32) -> Result<Vec<u8>, PipelineError> {
    try_encode_empty_io_queue(slot_count)
}
