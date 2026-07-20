//! Resident resource lifecycle helpers for WGPU backend buffers.

use crate::WgpuBackend;

/// Allocate one backend-resident WGPU buffer.
pub(crate) fn allocate_resident(
    backend: &WgpuBackend,
    byte_len: usize,
) -> Result<vyre_driver::Resource, vyre_driver::BackendError> {
    let device_queue = backend.current_device_queue();
    let len = u64::try_from(byte_len).map_err(|source| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident allocation byte length {byte_len} cannot fit u64: {source}. Fix: shard the resident buffer before allocation."
        ))
    })?;
    let handle = crate::buffer::GpuBufferHandle::alloc(
        &device_queue.0,
        len,
        wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::INDIRECT,
    )?;
    let id = handle.id();
    backend.resident_handles.insert(id, handle);
    Ok(vyre_driver::Resource::Resident(id))
}

/// Free one backend-resident WGPU buffer.
pub(crate) fn free_resident(
    backend: &WgpuBackend,
    resource: vyre_driver::Resource,
) -> Result<(), vyre_driver::BackendError> {
    let vyre_driver::Resource::Resident(id) = resource else {
        return Err(vyre_driver::BackendError::new(
            "WGPU resident free received a borrowed resource. Fix: only free handles returned by allocate_resident.",
        ));
    };
    backend.resident_handles.remove(&id).ok_or_else(|| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident free received stale handle {id}. Fix: free each resident resource exactly once."
        ))
    })?;
    Ok(())
}
