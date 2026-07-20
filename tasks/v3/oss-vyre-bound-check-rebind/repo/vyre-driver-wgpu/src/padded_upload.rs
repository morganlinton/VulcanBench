//! Shared WGPU padded upload helpers.
//!
//! WGPU buffer writes must be 4-byte aligned for the tail write paths used by
//! vyre. Centralizing the prefix/tail split keeps hot upload paths consistent
//! and prevents each caller from allocating or zero-filling differently.
//!
//! # Upload strategy
//!
//! `write_padded_and_zero_fill` is used for IN-PLACE updates of pre-existing
//! GPU buffers (e.g. persistent pipeline outputs). The primary NEW-buffer
//! upload path (`GpuBufferHandle::upload`) uses `write_padded_into_mapped`
//! instead: the caller creates the destination buffer directly with
//! `mapped_at_creation: true` (no extra `MAP_WRITE`/`COPY_SRC` usage and no
//! separate staging buffer — `mapped_at_creation` is legal for any usage),
//! obtains the mapped range, calls this function to fill it, then `unmap`s.
//! That is a single host memcpy into HOST_VISIBLE / BAR memory, bypassing
//! wgpu's internal per-write `StagingBelt` allocation entirely and writing at
//! DRAM / PCIe-BAR speed instead of the ~90 MB/s staged path.

use crate::numeric::usize_to_u64;
use vyre_driver::BackendError;

/// Write the aligned byte prefix and one padded 4-byte tail, returning the
/// first byte after the logical padded payload.
pub(crate) fn write_padded_prefix(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    bytes: &[u8],
    tail_offset_label: &'static str,
) -> Result<usize, BackendError> {
    let aligned_len = bytes.len() & !3;
    if aligned_len > 0 {
        queue.write_buffer(buffer, 0, &bytes[..aligned_len]);
    }

    let tail_len = bytes.len() - aligned_len;
    if tail_len == 0 {
        return Ok(aligned_len);
    }

    let mut tail = [0u8; 4];
    tail[..tail_len].copy_from_slice(&bytes[aligned_len..]);
    queue.write_buffer(buffer, usize_to_u64(aligned_len, tail_offset_label)?, &tail);
    Ok(aligned_len + 4)
}

/// Write a padded prefix and zero-fill the remaining allocation.
pub(crate) fn write_padded_and_zero_fill(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    bytes: &[u8],
    allocation_len: u64,
) -> Result<(), BackendError> {
    let allocation_len = usize::try_from(allocation_len).map_err(|source| {
        BackendError::new(format!(
            "GPU allocation length {allocation_len} cannot fit usize: {source}. Fix: split the dispatch input."
        ))
    })?;
    let zero_start = write_padded_prefix(queue, buffer, bytes, "GPU padded tail offset")?;
    write_zero_fill(queue, buffer, zero_start, allocation_len)
}

fn write_zero_fill(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    zero_start: usize,
    allocation_len: usize,
) -> Result<(), BackendError> {
    if allocation_len <= zero_start {
        return Ok(());
    }

    static SCRATCH_ZEROS: [u8; 65_536] = [0u8; 65_536];
    let mut offset = zero_start;
    while offset < allocation_len {
        let chunk = (allocation_len - offset).min(SCRATCH_ZEROS.len());
        queue.write_buffer(
            buffer,
            usize_to_u64(offset, "GPU zero-fill offset")?,
            &SCRATCH_ZEROS[..chunk],
        );
        offset += chunk;
    }
    Ok(())
}

/// Write `bytes` into a pre-mapped GPU buffer range and zero-fill the tail.
///
/// `mapped` must be the full allocation slice obtained from
/// `buffer.slice(..).get_mapped_range_mut()` on a buffer created with
/// `mapped_at_creation: true` (any usage flags — initial mapping does not
/// require `MAP_WRITE`). Its length must be `>= bytes.len()` and must equal
/// the allocation size passed to `create_buffer`.
///
/// This is the fast path for new-buffer uploads: the caller writes directly
/// into BAR / HOST_VISIBLE memory without wgpu allocating a separate staging
/// buffer, avoiding the per-upload `VkAllocateMemory` + `VkMapMemory` round-
/// trip that causes ~90 MB/s throughput on large one-shot catalog uploads.
///
/// # Errors
///
/// Returns a backend error when `mapped.len() < bytes.len()`.
pub(crate) fn write_padded_into_mapped(
    mapped: &mut [u8],
    bytes: &[u8],
) -> Result<(), BackendError> {
    if mapped.len() < bytes.len() {
        return Err(BackendError::new(format!(
            "write_padded_into_mapped: mapped slice is {} bytes but data is {} bytes. Fix: allocate the upload buffer with at least the padded data length.",
            mapped.len(),
            bytes.len(),
        )));
    }
    mapped[..bytes.len()].copy_from_slice(bytes);
    // Zero-fill the alignment tail so the GPU sees deterministic padding.
    // The mapped range is already zeroed by the driver for MAP_WRITE buffers
    // created with mapped_at_creation:true on most Vulkan implementations,
    // but we zero it explicitly for correctness on all platforms.
    mapped[bytes.len()..].fill(0);
    Ok(())
}
