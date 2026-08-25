//! Resident upload helpers for WGPU backend resources.

use crate::numeric::usize_to_u64;
use crate::WgpuBackend;

/// Upload one full resident buffer with zero padding to its allocation size.
pub(crate) fn upload_resident(
    backend: &WgpuBackend,
    resource: &vyre_driver::Resource,
    bytes: &[u8],
) -> Result<(), vyre_driver::BackendError> {
    upload_resident_many(backend, &[(resource, bytes)])
}

/// Upload several full resident buffers as one validated staging operation.
pub(crate) fn upload_resident_many(
    backend: &WgpuBackend,
    uploads: &[(&vyre_driver::Resource, &[u8])],
) -> Result<(), vyre_driver::BackendError> {
    let mut resolved = smallvec::SmallVec::<[_; 8]>::new();
    vyre_foundation::allocation::try_reserve_smallvec_to_capacity(&mut resolved, uploads.len()).map_err(|source| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident batch upload could not reserve {} validated upload descriptor(s): {source}. Fix: split the resident upload batch before staging.",
            uploads.len()
        ))
    })?;
    for &(resource, bytes) in uploads {
        let vyre_driver::Resource::Resident(id) = resource else {
            return Err(vyre_driver::BackendError::new(
                "WGPU resident batch upload received a borrowed resource. Fix: allocate every resident buffer before calling upload_resident_many.",
            ));
        };
        let handle = backend.resident_handles.get(id).ok_or_else(|| {
            vyre_driver::BackendError::new(format!(
                "WGPU resident batch upload received stale handle {id}. Fix: keep every resource allocated until all resident uploads finish."
            ))
        })?;
        let byte_len = usize_to_u64(bytes.len(), "resident batch upload bytes")?;
        if byte_len > handle.allocation_len() {
            return Err(vyre_driver::BackendError::new(format!(
                "WGPU resident batch upload received {} bytes for allocation {} on handle {id}. Fix: resize the resident buffer or upload a bounded prefix.",
                bytes.len(),
                handle.allocation_len()
            )));
        }
        resolved.push((handle.clone(), bytes));
    }
    let device_queue = backend.current_device_queue();
    for (handle, bytes) in resolved {
        crate::buffer::write_padded(
            &device_queue.1,
            handle.buffer(),
            bytes,
            handle.allocation_len(),
        )?;
    }
    Ok(())
}

/// Upload one aligned byte range into a backend-resident WGPU buffer.
pub(crate) fn upload_resident_at(
    backend: &WgpuBackend,
    resource: &vyre_driver::Resource,
    dst_offset_bytes: usize,
    bytes: &[u8],
) -> Result<(), vyre_driver::BackendError> {
    upload_resident_at_many(backend, &[(resource, dst_offset_bytes, bytes)])
}

/// Upload several aligned byte ranges into backend-resident WGPU buffers.
pub(crate) fn upload_resident_at_many(
    backend: &WgpuBackend,
    uploads: &[(&vyre_driver::Resource, usize, &[u8])],
) -> Result<(), vyre_driver::BackendError> {
    let mut resolved = smallvec::SmallVec::<[_; 8]>::new();
    vyre_foundation::allocation::try_reserve_smallvec_to_capacity(&mut resolved, uploads.len()).map_err(|source| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident ranged batch upload could not reserve {} validated upload descriptor(s): {source}. Fix: split the resident upload batch before staging.",
            uploads.len()
        ))
    })?;
    for &(resource, dst_offset_bytes, bytes) in uploads {
        let vyre_driver::Resource::Resident(id) = resource else {
            return Err(vyre_driver::BackendError::new(
                "WGPU resident ranged batch upload received a borrowed resource. Fix: allocate every resident buffer before calling upload_resident_at_many.",
            ));
        };
        require_copy_alignment(*id, dst_offset_bytes, bytes.len())?;
        let handle = backend.resident_handles.get(id).ok_or_else(|| {
            vyre_driver::BackendError::new(format!(
                "WGPU resident ranged batch upload received stale handle {id}. Fix: keep every resource allocated until all resident uploads finish."
            ))
        })?;
        let dst_offset = usize_to_u64(dst_offset_bytes, "resident ranged upload offset")?;
        let byte_len = usize_to_u64(bytes.len(), "resident ranged upload bytes")?;
        let end = dst_offset.checked_add(byte_len).ok_or_else(|| {
            vyre_driver::BackendError::new(format!(
                "WGPU resident ranged batch upload overflows u64 at offset {dst_offset_bytes} len {} for handle {id}. Fix: split the upload before calling upload_resident_at_many.",
                bytes.len()
            ))
        })?;
        if end > handle.allocation_len() {
            return Err(vyre_driver::BackendError::new(format!(
                "WGPU resident ranged batch upload requested byte range [{dst_offset}..{end}) for allocation {} on handle {id}. Fix: resize the resident buffer or clamp the staged payload.",
                handle.allocation_len()
            )));
        }
        resolved.push((handle.clone(), dst_offset, bytes));
    }
    let device_queue = backend.current_device_queue();
    for (handle, dst_offset, bytes) in resolved {
        device_queue
            .1
            .write_buffer(handle.buffer(), dst_offset, bytes);
    }
    Ok(())
}

fn require_copy_alignment(
    handle_id: u64,
    dst_offset_bytes: usize,
    byte_len: usize,
) -> Result<(), vyre_driver::BackendError> {
    if dst_offset_bytes % wgpu::COPY_BUFFER_ALIGNMENT as usize != 0 {
        return Err(vyre_driver::BackendError::new(format!(
            "WGPU resident ranged batch upload offset {dst_offset_bytes} for handle {handle_id} is not {}-byte aligned. Fix: pack resident ranged uploads on u32 boundaries or use a full resident upload.",
            wgpu::COPY_BUFFER_ALIGNMENT
        )));
    }
    if byte_len % wgpu::COPY_BUFFER_ALIGNMENT as usize != 0 {
        return Err(vyre_driver::BackendError::new(format!(
            "WGPU resident ranged batch upload length {byte_len} for handle {handle_id} is not {}-byte aligned. Fix: pad the staged resident range to a u32 boundary before upload.",
            wgpu::COPY_BUFFER_ALIGNMENT
        )));
    }
    Ok(())
}
