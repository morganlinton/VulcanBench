//! Trimmed WGPU output readback helpers.
//!
//! Output byte ranges are part of the public dispatch contract. Readback must
//! transfer only the prefix required to satisfy that range; reading the whole
//! device allocation and trimming on the host silently turns small logical
//! outputs into full-buffer PCIe transfers.

use std::time::Instant;

use vyre_driver::program_walks::OutputBindingLayout;
use vyre_driver::BackendError;

use crate::buffer::{GpuBufferHandle, StagingBufferPool};

pub(crate) fn read_trimmed_output(
    handle: &GpuBufferHandle,
    output: &OutputBindingLayout,
    device: &wgpu::Device,
    staging_pool: &StagingBufferPool,
    queue: &wgpu::Queue,
    label: &str,
    deadline: Option<Instant>,
    bytes: &mut Vec<u8>,
) -> Result<(), BackendError> {
    let end = output
        .layout
        .trim_start
        .checked_add(output.layout.read_size)
        .ok_or_else(|| {
            BackendError::new(format!(
                "{label} readback slice for `{}` overflows usize. Fix: verify OutputLayout before dispatch.",
                output.name
            ))
        })?;
    let trim_start_u64 = u64::try_from(output.layout.trim_start).map_err(|error| {
        BackendError::new(format!(
            "{label} readback trim_start {} for `{}` does not fit u64: {error}. Fix: shard the GPU output before readback.",
            output.layout.trim_start,
            output.name
        ))
    })?;
    let read_size_u64 = u64::try_from(output.layout.read_size).map_err(|error| {
        BackendError::new(format!(
            "{label} readback read_size {} for `{}` does not fit u64: {error}. Fix: shard the GPU output before readback.",
            output.layout.read_size,
            output.name
        ))
    })?;
    let end_u64 = u64::try_from(end).map_err(|error| {
        BackendError::new(format!(
            "{label} readback prefix {end} for `{}` does not fit u64: {error}. Fix: shard the GPU output before readback.",
            output.name
        ))
    })?;
    if end_u64 > handle.byte_len() {
        return Err(BackendError::new(format!(
            "{label} readback slice for `{}` ends at byte {end_u64} but the GPU output allocation is only {} bytes. Fix: verify OutputLayout against the GPU output allocation.",
            output.name,
            handle.byte_len()
        )));
    }

    handle.readback_range_until(
        device,
        Some(staging_pool),
        queue,
        trim_start_u64,
        read_size_u64,
        bytes,
        deadline,
    )?;
    if output.layout.read_size > bytes.len() {
        return Err(BackendError::new(format!(
            "{label} readback slice for `{}` returned {} bytes but {} bytes are required. Fix: verify staging readback length handling.",
            output.name,
            bytes.len(),
            output.layout.read_size
        )));
    }
    bytes.truncate(output.layout.read_size);
    Ok(())
}
