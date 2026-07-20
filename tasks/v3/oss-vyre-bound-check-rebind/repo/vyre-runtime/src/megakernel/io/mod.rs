//! IO subsystem  -  GPU↔runtime DMA request queue for the persistent megakernel.
//!
//! Module ownership:
//!  - `mod.rs`: doc + constants + IoRequest/IoCompletion + word/op/status modules
//!  - `queue.rs`: [`MegakernelIoQueue`] + view
//!  - `poll.rs`: poll/claim/peek surface
//!  - `complete.rs`: completion-write surface
//!  - `encode.rs`: bytes <-> validated queue helpers
//!  - `helpers.rs`: low-level queue-word + validation utilities + IR builders
//!  - `tests.rs`: full test suite
//!
//! ## Protocol
//!
//! Each IO slot is 8 × u32 words:
//! ```text
//! [op_type, src_handle, dst_handle, offset_lo, offset_hi, byte_count, status, tag]
//! ```
//!
//! The GPU CAS-claims slots like the work ring, but uses the io_queue
//! buffer. The host polls `status` for REQUEST and services the DMA.

mod complete;
mod encode;
mod helpers;
mod poll;
mod queue;

#[cfg(test)]
mod tests;

pub use complete::{
    complete_io_request, complete_io_requests_batch, try_complete_io_request,
    try_complete_io_requests_batch,
};
pub(crate) use encode::empty_io_queue_byte_len;
pub use encode::{
    encode_empty_io_queue, try_encode_empty_io_queue, try_encode_empty_io_queue_into,
    validate_io_queue_bytes,
};
pub use helpers::io_completion_poll_body;
pub use poll::{
    claim_io_requests_into, poll_io_requests, try_claim_io_requests_into, try_poll_io_requests,
    try_poll_io_requests_into,
};
pub use queue::MegakernelIoQueue;

/// Number of u32 words per IO queue slot.
pub const IO_SLOT_WORDS: u32 = 8;

/// Default number of IO queue slots.
pub const IO_SLOT_COUNT: u32 = 64;

/// Resource table name used for resolving IO source handles.
pub const IO_SOURCE_CAPABILITY_TABLE: &str = "io_source_capability_table";

/// Resource table name used for resolving IO destination handles.
pub const IO_DESTINATION_CAPABILITY_TABLE: &str = "io_destination_capability_table";

/// Async stream tag used by megakernel IO DMA requests.
pub const IO_QUEUE_DMA_TAG: &str = "io_queue_dma";

/// Word offsets within an IO slot.
pub mod io_word {
    /// DMA operation type (see `IoOp`).
    pub const OP_TYPE: u32 = 0;
    /// Source buffer handle id.
    pub const SRC_HANDLE: u32 = 1;
    /// Destination buffer handle id.
    pub const DST_HANDLE: u32 = 2;
    /// Byte offset into source (low 32 bits).
    pub const OFFSET_LO: u32 = 3;
    /// Byte offset into source (high 32 bits, for >4GB transfers).
    pub const OFFSET_HI: u32 = 4;
    /// Number of bytes to transfer.
    pub const BYTE_COUNT: u32 = 5;
    /// Slot status  -  same semantics as work ring (EMPTY/PUBLISHED/CLAIMED/DONE).
    pub const STATUS: u32 = 6;
    /// Caller-supplied tag for correlating completions.
    pub const TAG: u32 = 7;
}

/// IO operation types.
pub mod io_op {
    /// Read from storage into GPU buffer.
    pub const READ: u32 = 0x01;
    /// Write from GPU buffer to storage.
    pub const WRITE: u32 = 0x02;
    /// Memory fence  -  ensure all prior IO ops are visible.
    pub const FENCE: u32 = 0x03;
}

/// IO completion status codes written by the host pump.
pub mod io_status {
    /// Operation completed successfully.
    pub const OK: u32 = 0x10;
    /// Operation failed  -  error code in the tag word.
    pub const ERROR: u32 = 0x11;
}

/// Host-side IO request decoded from the io_queue buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoRequest {
    /// Slot index in the io_queue.
    pub slot_idx: u32,
    /// Operation type.
    pub op_type: u32,
    /// Source buffer handle.
    pub src_handle: u32,
    /// Destination buffer handle.
    pub dst_handle: u32,
    /// 64-bit byte offset into source.
    pub offset: u64,
    /// Byte count to transfer.
    pub byte_count: u32,
    /// Caller tag.
    pub tag: u32,
}

/// Host-side completion record published into `io_queue` for a mapped
/// ingest slot the GPU can consume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoCompletion {
    /// Queue slot index.
    pub slot_idx: u32,
    /// Mapped ingest slot id / destination handle.
    pub mapped_slot: u32,
    /// Number of bytes now valid in the mapped slot.
    pub byte_count: u32,
    /// Caller-defined completion tag.
    pub tag: u32,
}
