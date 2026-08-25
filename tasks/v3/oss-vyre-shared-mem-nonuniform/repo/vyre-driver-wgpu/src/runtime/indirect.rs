//! Indirect dispatch path (C-B4).
//!
//! Submits `ComputePass::dispatch_workgroups_indirect(buffer, 0)`
//! on a GPU-resident `[u32; 3]` workgroup-count buffer, so the
//! upstream kernel can decide the downstream dispatch shape
//! without a round-trip to host.
//!
//! The `core.indirect_dispatch` op (registered in
//! `vyre-core/src/dialect/core_indirect.rs`) names this operation
//! in vyre IR; this module is the wgpu-side implementation.

use std::sync::Arc;
use vyre_driver::BackendError;

use crate::buffer::GpuBufferHandle;

/// Minimum size of an indirect workgroup-count buffer: three
/// little-endian `u32`s, or 12 bytes.
pub const INDIRECT_ARGS_BYTES: u64 = 12;

/// Handle + offset identifying where in GPU memory the `[u32; 3]`
/// workgroup count lives.
pub struct IndirectArgs {
    /// GPU buffer containing the workgroup count at the given byte
    /// offset.
    pub buffer: Arc<wgpu::Buffer>,
    /// Byte offset within `buffer`. Must be 4-byte aligned per
    /// wgpu's contract.
    pub offset: u64,
}

impl IndirectArgs {
    /// Build an `IndirectArgs` from a `GpuBufferHandle` + offset.
    ///
    /// # Errors
    ///
    /// Returns a `BackendError` when:
    ///
    /// * `offset` is not 4-byte aligned (wgpu rejects unaligned
    ///   indirect dispatches).
    /// * `offset + INDIRECT_ARGS_BYTES` would exceed the buffer's
    ///   byte length.
    /// * The underlying buffer does not carry
    ///   `wgpu::BufferUsages::INDIRECT`.
    pub fn from_handle(handle: &GpuBufferHandle, offset: u64) -> Result<Self, BackendError> {
        if offset & 0b11 != 0 {
            return Err(BackendError::new(format!(
                "indirect dispatch offset {offset} is not 4-byte aligned. Fix: align to a u32 boundary."
            )));
        }
        if offset
            .checked_add(INDIRECT_ARGS_BYTES)
            .map(|end| end > handle.byte_len())
            .unwrap_or(true)
        {
            return Err(BackendError::new(format!(
                "indirect dispatch would read past buffer end (offset={offset}, args={INDIRECT_ARGS_BYTES}, buffer byte_len={}). Fix: grow the buffer or lower the offset.",
                handle.byte_len()
            )));
        }
        if !handle.usage().contains(wgpu::BufferUsages::INDIRECT) {
            return Err(BackendError::new(
                "indirect dispatch requires buffer with `wgpu::BufferUsages::INDIRECT`. Fix: allocate the workgroup-count buffer with INDIRECT usage.",
            ));
        }
        Ok(Self {
            buffer: handle.buffer_arc(),
            offset,
        })
    }
}

/// Record an indirect dispatch into an existing compute pass.
///
/// The caller sets the pipeline + bind group before calling this;
/// we just submit the `dispatch_workgroups_indirect`.
pub fn dispatch_indirect<'a>(pass: &mut wgpu::ComputePass<'a>, args: &'a IndirectArgs) {
    pass.dispatch_workgroups_indirect(&args.buffer, args.offset);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_bytes_is_twelve() {
        assert_eq!(INDIRECT_ARGS_BYTES, 12);
    }

    // Note: tests that actually construct IndirectArgs require a
    // real wgpu::Buffer and hence a GPU. The full dispatch path is
    // exercised from vyre-wgpu integration tests (`tests/indirect_dispatch.rs`).
}
