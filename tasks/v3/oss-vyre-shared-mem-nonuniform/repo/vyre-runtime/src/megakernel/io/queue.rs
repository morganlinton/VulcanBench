//! [`MegakernelIoQueue`]  -  high-level wrapper around the raw queue bytes.

use std::sync::atomic::{fence, Ordering};

use crate::PipelineError;

use super::super::protocol::slot;
use super::helpers::try_queue_word_index;
use super::{io_op, io_status, io_word, IoCompletion, IO_SLOT_COUNT, IO_SLOT_WORDS};

/// Host-side handle to the megakernel IO queue. Wraps a `Vec<u32>` slot ring
/// and exposes typed poll/publish/complete entry points.
#[derive(Debug, Clone)]
pub struct MegakernelIoQueue {
    words: Vec<u32>,
    slot_count: u32,
}

impl MegakernelIoQueue {
    /// Allocate an empty queue with `slot_count` entries.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when `slot_count` is zero or
    /// exceeds the IR/program's fixed poll window of [`IO_SLOT_COUNT`].
    pub fn new(slot_count: u32) -> Result<Self, PipelineError> {
        if slot_count == 0 {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "MegakernelIoQueue requires at least one slot",
            });
        }
        if slot_count > IO_SLOT_COUNT {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "MegakernelIoQueue exceeds the compiled IO poll window of 64 slots; enlarge IO_SLOT_COUNT and rebuild the megakernel before publishing more than 64 completions",
            });
        }
        let word_count = slot_count
            .checked_mul(IO_SLOT_WORDS)
            .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix: "io_queue word count overflows u32; shard the queue before allocating",
            })?;
        let word_count = usize::try_from(word_count).map_err(|error| {
            PipelineError::Backend(format!(
                "io_queue word count cannot fit host usize: {error}. Fix: shard the queue before allocating."
            ))
        })?;
        Ok(Self {
            words: vec![0; word_count],
            slot_count,
        })
    }

    /// Borrow the raw bytes for backend upload / readback.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.words)
    }

    /// Mutably borrow the raw bytes for backend upload / host updates.
    #[must_use]
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        bytemuck::cast_slice_mut(&mut self.words)
    }

    /// Queue capacity in slots.
    #[must_use]
    pub fn slot_count(&self) -> u32 {
        self.slot_count
    }

    /// Publish a completed DMA slot so the megakernel can consume it.
    ///
    /// The host writes the metadata first, then flips `STATUS` to
    /// `slot::PUBLISHED` as the publication barrier.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the slot is out of bounds or
    /// still owned by the GPU/host from a prior ingest.
    pub fn publish_slot(
        &mut self,
        queue_slot: u32,
        mapped_slot: u32,
        byte_count: u32,
        tag: u32,
    ) -> Result<(), PipelineError> {
        if queue_slot >= self.slot_count {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "io_queue slot exceeds MegakernelIoQueue::slot_count; enlarge the queue or publish into a valid slot id",
            });
        }
        let current_status = self.read_word(queue_slot, io_word::STATUS)?;
        if current_status != slot::EMPTY {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "io_queue slot still in flight; wait for the GPU to recycle it before publishing again",
            });
        }
        self.write_word_unfenced(queue_slot, io_word::OP_TYPE, io_op::READ)?;
        self.write_word_unfenced(queue_slot, io_word::SRC_HANDLE, 0)?;
        self.write_word_unfenced(queue_slot, io_word::DST_HANDLE, mapped_slot)?;
        self.write_word_unfenced(queue_slot, io_word::OFFSET_LO, 0)?;
        self.write_word_unfenced(queue_slot, io_word::OFFSET_HI, 0)?;
        self.write_word_unfenced(queue_slot, io_word::BYTE_COUNT, byte_count)?;
        self.write_word_unfenced(queue_slot, io_word::TAG, tag)?;
        fence(Ordering::Release);
        self.write_word_unfenced(queue_slot, io_word::STATUS, slot::PUBLISHED)?;
        fence(Ordering::Release);
        Ok(())
    }

    /// Submit a DMA-read request to the IO queue.
    ///
    /// This is the GPU-initiated path: the caller writes the request metadata,
    /// then flips `STATUS` to `slot::PUBLISHED` so the host/runtime can claim
    /// and service it.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the slot is out of bounds or
    /// not empty.
    pub fn submit_dma_read(
        &mut self,
        queue_slot: u32,
        src_handle: u32,
        dst_handle: u32,
        byte_count: u32,
        tag: u32,
    ) -> Result<(), PipelineError> {
        if queue_slot >= self.slot_count {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "io_queue slot exceeds MegakernelIoQueue::slot_count; enlarge the queue or submit into a valid slot id",
            });
        }
        let current_status = self.read_word(queue_slot, io_word::STATUS)?;
        if current_status != slot::EMPTY {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "io_queue slot still in flight; wait for completion before submitting a new request",
            });
        }
        self.write_word_unfenced(queue_slot, io_word::OP_TYPE, io_op::READ)?;
        self.write_word_unfenced(queue_slot, io_word::SRC_HANDLE, src_handle)?;
        self.write_word_unfenced(queue_slot, io_word::DST_HANDLE, dst_handle)?;
        self.write_word_unfenced(queue_slot, io_word::OFFSET_LO, 0)?;
        self.write_word_unfenced(queue_slot, io_word::OFFSET_HI, 0)?;
        self.write_word_unfenced(queue_slot, io_word::BYTE_COUNT, byte_count)?;
        self.write_word_unfenced(queue_slot, io_word::TAG, tag)?;
        fence(Ordering::Release);
        self.write_word_unfenced(queue_slot, io_word::STATUS, slot::PUBLISHED)?;
        fence(Ordering::Release);
        Ok(())
    }

    /// Read the queue slot back as a completion record.
    #[must_use]
    pub fn completion(&self, queue_slot: u32) -> Option<IoCompletion> {
        if queue_slot >= self.slot_count {
            return None;
        }
        // INVARIANT: queue_slot < slot_count <= IO_SLOT_COUNT (64) and io_word
        // constants are all <= 7, so slot_idx * IO_SLOT_WORDS + word <= 511 which
        // cannot overflow usize on any supported platform. The expect here documents
        // that invariant loudly rather than silently reading the wrong word.
        let status = self
            .read_word(queue_slot, io_word::STATUS)
            .expect("IO queue word index overflow is impossible after slot bounds check; Fix: ensure slot_count <= IO_SLOT_COUNT before calling completion");
        if status == slot::EMPTY {
            return None;
        }
        Some(IoCompletion {
            slot_idx: queue_slot,
            mapped_slot: self
                .read_word_unfenced(queue_slot, io_word::DST_HANDLE)
                .expect("IO queue word index overflow is impossible after slot bounds check; Fix: ensure slot_count <= IO_SLOT_COUNT before calling completion"),
            byte_count: self
                .read_word_unfenced(queue_slot, io_word::BYTE_COUNT)
                .expect("IO queue word index overflow is impossible after slot bounds check; Fix: ensure slot_count <= IO_SLOT_COUNT before calling completion"),
            tag: self
                .read_word_unfenced(queue_slot, io_word::TAG)
                .expect("IO queue word index overflow is impossible after slot bounds check; Fix: ensure slot_count <= IO_SLOT_COUNT before calling completion"),
        })
    }

    /// Return true when the GPU has recycled the slot to `EMPTY`.
    #[must_use]
    pub fn is_recycled(&self, queue_slot: u32) -> bool {
        if queue_slot >= self.slot_count {
            return false;
        }
        // INVARIANT: same as completion — slot_idx * IO_SLOT_WORDS + word fits usize
        // after the queue_slot < slot_count guard above.
        let status = self
            .read_word(queue_slot, io_word::STATUS)
            .expect("IO queue word index overflow is impossible after slot bounds check; Fix: ensure slot_count <= IO_SLOT_COUNT before calling is_recycled");
        match status {
            slot::EMPTY => true,
            slot::PUBLISHED | slot::CLAIMED | io_status::OK | io_status::ERROR | slot::DONE => {
                false
            }
            _ => false,
        }
    }

    fn read_word(&self, slot_idx: u32, word: u32) -> Result<u32, PipelineError> {
        let idx = try_queue_word_index(slot_idx, word)?;
        fence(Ordering::Acquire);
        Ok(self.words[idx])
    }

    fn read_word_unfenced(&self, slot_idx: u32, word: u32) -> Result<u32, PipelineError> {
        let idx = try_queue_word_index(slot_idx, word)?;
        Ok(self.words[idx])
    }

    fn write_word_unfenced(
        &mut self,
        slot_idx: u32,
        word: u32,
        value: u32,
    ) -> Result<(), PipelineError> {
        let idx = try_queue_word_index(slot_idx, word)?;
        self.words[idx] = value;
        Ok(())
    }
}
