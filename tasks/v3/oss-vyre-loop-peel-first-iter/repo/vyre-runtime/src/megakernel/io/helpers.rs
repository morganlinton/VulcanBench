//! Low-level queue word access + queue-view validation + IR builders.

use crate::PipelineError;
use std::sync::atomic::{fence, Ordering};
use vyre_foundation::ir::{Expr, Node};

use super::super::protocol::slot;
use super::{io_status, io_word, IO_SLOT_COUNT, IO_SLOT_WORDS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct IoQueueView {
    pub(super) slot_count: usize,
}

pub(super) fn try_queue_word_index(slot_idx: u32, word: u32) -> Result<usize, PipelineError> {
    let slot = usize::try_from(slot_idx).map_err(|error| {
        PipelineError::Backend(format!(
            "IO queue slot index {slot_idx} cannot fit usize: {error}. Fix: shard the IO queue before polling."
        ))
    })?;
    let slot_words = usize::try_from(IO_SLOT_WORDS).map_err(|error| {
        PipelineError::Backend(format!(
            "IO_SLOT_WORDS cannot fit usize: {error}. Fix: keep IO_SLOT_WORDS within the host index ABI."
        ))
    })?;
    let word = usize::try_from(word).map_err(|error| {
        PipelineError::Backend(format!(
            "IO queue word index cannot fit usize: {error}. Fix: keep IO word offsets within the host index ABI."
        ))
    })?;
    slot.checked_mul(slot_words)
        .and_then(|base| base.checked_add(word))
        .ok_or_else(|| {
            PipelineError::Backend(format!(
                "IO queue word index overflow for slot {slot_idx}, word {word}. Fix: shard the IO queue before polling."
            ))
        })
}

pub(super) fn read_queue_word(
    io_queue_bytes: &[u8],
    base_word: usize,
    word: u32,
) -> Result<u32, PipelineError> {
    // Compute the byte offset with explicit overflow checks. The old
    // `(base_word + word as usize) * 4` wraps silently on overflow; on
    // 64-bit Linux the wrap would have to come from a deliberately
    // malformed caller, but the substrate should still fail closed.
    let word = usize::try_from(word).map_err(|error| {
        PipelineError::Backend(format!(
            "IO queue word index cannot fit usize: {error}. Fix: keep IO word offsets within the host index ABI."
        ))
    })?;
    let off = base_word
        .checked_add(word)
        .and_then(|w| w.checked_mul(4))
        .ok_or_else(|| {
            PipelineError::Backend(format!(
                "IO queue word offset overflow at base {base_word} + word {word}. Fix: pass a sane base_word/word pair within the validated queue view."
            ))
        })?;
    let bytes = io_queue_bytes.get(off..off + 4).ok_or_else(|| {
        PipelineError::Backend(format!(
            "IO queue word {word} at base {base_word} is outside the validated queue view. Fix: validate queue byte length before polling."
        ))
    })?;
    let mut word_bytes = [0u8; 4];
    fence(Ordering::Acquire);
    word_bytes.copy_from_slice(bytes);
    Ok(u32::from_le_bytes(word_bytes))
}

pub(super) fn write_queue_word(
    io_queue_bytes: &mut [u8],
    base_word: usize,
    word: u32,
    value: u32,
) -> Result<(), PipelineError> {
    write_queue_word_unfenced(io_queue_bytes, base_word, word, value)?;
    fence(Ordering::Release);
    Ok(())
}

pub(super) fn write_queue_word_unfenced(
    io_queue_bytes: &mut [u8],
    base_word: usize,
    word: u32,
    value: u32,
) -> Result<(), PipelineError> {
    // Same overflow-safe offset computation as read_queue_word.
    let word = usize::try_from(word).map_err(|error| {
        PipelineError::Backend(format!(
            "IO queue word index cannot fit usize: {error}. Fix: keep IO word offsets within the host index ABI."
        ))
    })?;
    let off = base_word
        .checked_add(word)
        .and_then(|w| w.checked_mul(4))
        .ok_or_else(|| {
            PipelineError::Backend(format!(
                "IO queue word offset overflow at base {base_word} + word {word}. Fix: pass a sane base_word/word pair within the validated queue view."
            ))
        })?;
    let bytes = io_queue_bytes.get_mut(off..off + 4).ok_or_else(|| {
        PipelineError::Backend(format!(
                "IO queue word {word} at base {base_word} is outside the validated queue view. Fix: validate queue byte length before completing."
            ))
    })?;
    bytes.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

pub(super) fn validate_io_queue_view(byte_len: usize) -> Result<IoQueueView, PipelineError> {
    if byte_len % 4 != 0 {
        return Err(PipelineError::Backend(format!(
            "io_queue has {byte_len} bytes, which is not 4-byte aligned. Fix: pass a whole u32 queue buffer."
        )));
    }
    let slot_words = usize::try_from(IO_SLOT_WORDS).map_err(|error| {
        PipelineError::Backend(format!(
            "IO_SLOT_WORDS cannot fit usize: {error}. Fix: keep IO_SLOT_WORDS within the host index ABI."
        ))
    })?;
    let slot_bytes = slot_words.checked_mul(4).ok_or(PipelineError::QueueFull {
        queue: "submission",
        fix: "io_queue slot byte width overflows usize; keep IO_SLOT_WORDS within the u32 ABI",
    })?;
    if byte_len % slot_bytes != 0 {
        return Err(PipelineError::Backend(format!(
            "io_queue has {byte_len} bytes, which is not a multiple of slot size {slot_bytes}. Fix: pass whole IO slots."
        )));
    }
    let slot_count = byte_len / slot_bytes;
    let max_slots = usize::try_from(IO_SLOT_COUNT).map_err(|error| {
        PipelineError::Backend(format!(
            "IO_SLOT_COUNT cannot fit usize: {error}. Fix: keep IO_SLOT_COUNT within the host index ABI."
        ))
    })?;
    if slot_count > max_slots {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "io_queue byte view exceeds the compiled IO poll window of 64 slots; split the queue or rebuild the megakernel with a larger IO_SLOT_COUNT",
        });
    }
    Ok(IoQueueView { slot_count })
}

/// Build the GPU-side IO poll body as `Vec<Node>` for composition
/// into the megakernel persistent loop.
///
/// Each iteration, the kernel scans IO slots for DONE status
/// (set by the host) and reads the completion result. This is
/// the GPU's "interrupt handler" for asynchronous DMA.
#[must_use]
pub fn io_completion_poll_body() -> Vec<Node> {
    vec![Node::loop_for(
        "io_poll_idx",
        Expr::u32(0),
        Expr::u32(IO_SLOT_COUNT),
        vec![
            Node::let_bind(
                "io_poll_base",
                Expr::mul(Expr::var("io_poll_idx"), Expr::u32(IO_SLOT_WORDS)),
            ),
            Node::let_bind(
                "io_poll_status",
                Expr::load(
                    "io_queue",
                    Expr::add(Expr::var("io_poll_base"), Expr::u32(io_word::STATUS)),
                ),
            ),
            // If host marked OK or ERROR, clear the slot for reuse.
            Node::if_then(
                Expr::or(
                    Expr::eq(Expr::var("io_poll_status"), Expr::u32(io_status::OK)),
                    Expr::eq(Expr::var("io_poll_status"), Expr::u32(io_status::ERROR)),
                ),
                vec![Node::store(
                    "io_queue",
                    Expr::add(Expr::var("io_poll_base"), Expr::u32(io_word::STATUS)),
                    Expr::u32(slot::EMPTY),
                )],
            ),
        ],
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: the old `(base_word + word as usize) * 4` formula
    /// silently wrapped on overflow. With the checked_add+checked_mul
    /// fix the caller now sees a structured error instead of either a
    /// panic or a silent wrong-word read.
    #[test]
    fn read_queue_word_rejects_word_index_overflow() {
        let bytes = vec![0u8; 1024];
        let err = read_queue_word(&bytes, usize::MAX, 1).unwrap_err();
        match err {
            PipelineError::Backend(msg) => {
                assert!(
                    msg.contains("overflow"),
                    "expected overflow message, got: {msg}"
                );
            }
            other => panic!("expected Backend overflow error, got {other:?}"),
        }
    }

    #[test]
    fn write_queue_word_unfenced_rejects_word_index_overflow() {
        let mut bytes = vec![0u8; 1024];
        let err = write_queue_word_unfenced(&mut bytes, usize::MAX, 1, 0).unwrap_err();
        match err {
            PipelineError::Backend(msg) => {
                assert!(
                    msg.contains("overflow"),
                    "expected overflow message, got: {msg}"
                );
            }
            other => panic!("expected Backend overflow error, got {other:?}"),
        }
    }
}
