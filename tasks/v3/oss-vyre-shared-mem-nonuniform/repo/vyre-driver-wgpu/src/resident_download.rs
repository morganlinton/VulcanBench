//! Resident download helpers for WGPU backend resources.

use smallvec::SmallVec;
use vyre_driver::resident_transfer_fusion::{
    fuse_resident_transfer_intervals, ResidentTransferInterval, ResidentTransferView,
};

use crate::buffer::GpuBufferHandle;
use crate::numeric::usize_to_u64;
use crate::WgpuBackend;

/// Download a complete backend-resident WGPU buffer into a new host buffer.
pub(crate) fn download_resident(
    backend: &WgpuBackend,
    resource: &vyre_driver::Resource,
) -> Result<Vec<u8>, vyre_driver::BackendError> {
    let vyre_driver::Resource::Resident(id) = resource else {
        return Err(vyre_driver::BackendError::new(
            "WGPU resident download received a borrowed resource. Fix: allocate a resident buffer before calling download_resident_into.",
        ));
    };
    let handle = backend.resident_handles.get(id).ok_or_else(|| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident download received stale handle {id}. Fix: keep the resource allocated until all resident readbacks finish."
        ))
    })?;
    let allocation_len = usize::try_from(handle.allocation_len()).map_err(|source| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident download allocation length cannot fit usize: {source}. Fix: split the resident readback before downloading."
        ))
    })?;
    let mut bytes = Vec::new();
    vyre_driver::allocation::try_reserve_vec_to_capacity(&mut bytes, allocation_len).map_err(|source| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident download could not reserve {allocation_len} output byte(s): {source}. Fix: split the resident readback before downloading."
        ))
    })?;
    let device_queue = backend.current_device_queue();
    handle.readback_until(&device_queue.0, None, &device_queue.1, &mut bytes, None)?;
    Ok(bytes)
}

/// Download a complete backend-resident WGPU buffer into caller-owned storage.
pub(crate) fn download_resident_into(
    backend: &WgpuBackend,
    resource: &vyre_driver::Resource,
    out: &mut Vec<u8>,
) -> Result<(), vyre_driver::BackendError> {
    let vyre_driver::Resource::Resident(id) = resource else {
        return Err(vyre_driver::BackendError::new(
            "WGPU resident download received a borrowed resource. Fix: allocate a resident buffer before calling download_resident_into.",
        ));
    };
    let handle = backend.resident_handles.get(id).ok_or_else(|| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident download received stale handle {id}. Fix: keep the resource allocated until all resident readbacks finish."
        ))
    })?;
    let device_queue = backend.current_device_queue();
    handle.readback_until(&device_queue.0, None, &device_queue.1, out, None)
}

/// Download one byte range from a backend-resident WGPU buffer into a new host buffer.
pub(crate) fn download_resident_range(
    backend: &WgpuBackend,
    resource: &vyre_driver::Resource,
    byte_offset: usize,
    byte_len: usize,
) -> Result<Vec<u8>, vyre_driver::BackendError> {
    let mut bytes = Vec::new();
    vyre_driver::allocation::try_reserve_vec_to_capacity(&mut bytes, byte_len).map_err(|source| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident ranged download could not reserve {byte_len} output byte(s): {source}. Fix: split the resident readback range before downloading."
        ))
    })?;
    download_resident_range_into(backend, resource, byte_offset, byte_len, &mut bytes)?;
    Ok(bytes)
}

/// Download one byte range from a backend-resident WGPU buffer into caller-owned storage.
pub(crate) fn download_resident_range_into(
    backend: &WgpuBackend,
    resource: &vyre_driver::Resource,
    byte_offset: usize,
    byte_len: usize,
    out: &mut Vec<u8>,
) -> Result<(), vyre_driver::BackendError> {
    let vyre_driver::Resource::Resident(id) = resource else {
        return Err(vyre_driver::BackendError::new(
            "WGPU resident ranged download received a borrowed resource. Fix: allocate a resident buffer before calling download_resident_range_into.",
        ));
    };
    let handle = backend.resident_handles.get(id).ok_or_else(|| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident ranged download received stale handle {id}. Fix: keep the resource allocated until all resident readbacks finish."
        ))
    })?;
    let byte_offset = usize_to_u64(byte_offset, "resident ranged download offset")?;
    let byte_len = usize_to_u64(byte_len, "resident ranged download length")?;
    validate_resident_readback_range(
        *id,
        handle.allocation_len(),
        byte_offset,
        byte_len,
        "ranged download",
    )?;
    let device_queue = backend.current_device_queue();
    handle.readback_range_until(
        &device_queue.0,
        None,
        &device_queue.1,
        byte_offset,
        byte_len,
        out,
        None,
    )
}

/// Download several validated resident byte ranges into caller-owned buffers.
pub(crate) fn download_resident_ranges_into(
    backend: &WgpuBackend,
    ranges: &[(&vyre_driver::Resource, usize, usize)],
    outputs: &mut [&mut Vec<u8>],
) -> Result<(), vyre_driver::BackendError> {
    if ranges.len() != outputs.len() {
        return Err(vyre_driver::BackendError::new(format!(
            "WGPU resident ranged batch download expected matching range/output counts but got {} range(s) and {} output(s). Fix: pass one caller-owned output Vec per readback range.",
            ranges.len(),
            outputs.len()
        )));
    }
    let mut copies = SmallVec::<[ResidentTransferInterval; 8]>::new();
    let mut handles = SmallVec::<[(u64, GpuBufferHandle); 8]>::new();
    vyre_foundation::allocation::try_reserve_smallvec_to_capacity(&mut copies, ranges.len())
        .map_err(|source| {
            vyre_driver::BackendError::new(format!(
                "WGPU resident ranged batch download could not reserve {} validated readback descriptor(s): {source}. Fix: split the resident readback batch before staging.",
                ranges.len()
            ))
        })?;
    vyre_foundation::allocation::try_reserve_smallvec_to_capacity(&mut handles, ranges.len())
        .map_err(|source| {
            vyre_driver::BackendError::new(format!(
                "WGPU resident ranged batch download could not reserve {} resident handle descriptor(s): {source}. Fix: split the resident readback batch before staging.",
                ranges.len()
            ))
        })?;
    for &(resource, byte_offset, byte_len) in ranges {
        let vyre_driver::Resource::Resident(id) = resource else {
            return Err(vyre_driver::BackendError::new(
                "WGPU resident ranged batch download received a borrowed resource. Fix: allocate every resident buffer before calling download_resident_ranges_into.",
            ));
        };
        let handle = backend.resident_handles.get(id).ok_or_else(|| {
            vyre_driver::BackendError::new(format!(
                "WGPU resident ranged batch download received stale handle {id}. Fix: keep every resource allocated until all resident readbacks finish."
            ))
        })?;
        let byte_offset_u64 = usize_to_u64(byte_offset, "resident ranged batch download offset")?;
        let byte_len_u64 = usize_to_u64(byte_len, "resident ranged batch download length")?;
        validate_resident_readback_range(
            *id,
            handle.allocation_len(),
            byte_offset_u64,
            byte_len_u64,
            "ranged batch download",
        )?;
        handles.push((*id, handle.clone()));
        copies.push(ResidentTransferInterval {
            handle_id: *id,
            src: byte_offset_u64,
            byte_len,
        });
    }
    handles.sort_unstable_by_key(|(handle_id, _)| *handle_id);
    handles.dedup_by_key(|(handle_id, _)| *handle_id);
    let fused = fuse_resident_transfer_intervals(&copies)?;
    reserve_fused_resident_view_outputs(&fused.copies, &fused.views, outputs)?;
    let mut fused_outputs = SmallVec::<[Vec<u8>; 8]>::new();
    vyre_foundation::allocation::try_reserve_smallvec_to_capacity(
        &mut fused_outputs,
        fused.copies.len(),
    )
    .map_err(|source| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident ranged batch download could not reserve {} fused readback output slot(s): {source}. Fix: split the resident readback batch before staging.",
            fused.copies.len()
        ))
    })?;
    let device_queue = backend.current_device_queue();
    for copy in fused.copies.iter().copied() {
        let handle_index = handles
            .binary_search_by_key(&copy.handle_id, |(handle_id, _)| *handle_id)
            .map_err(|_| {
                vyre_driver::BackendError::new(format!(
                    "WGPU resident ranged batch download fused copy references unknown handle {}. Fix: rebuild the fused readback plan after validation.",
                    copy.handle_id
                ))
            })?;
        let handle = &handles[handle_index].1;
        let byte_len = usize_to_u64(copy.byte_len, "resident fused ranged batch download length")?;
        let mut fused_output = Vec::new();
        handle.readback_range_until(
            &device_queue.0,
            None,
            &device_queue.1,
            copy.src,
            byte_len,
            &mut fused_output,
            None,
        )?;
        fused_outputs.push(fused_output);
    }
    for (view, output) in fused.views.iter().copied().zip(outputs.iter_mut()) {
        copy_fused_resident_view_into(&fused_outputs, view, output)?;
    }
    Ok(())
}

fn reserve_fused_resident_view_outputs(
    copies: &[ResidentTransferInterval],
    views: &[ResidentTransferView],
    outputs: &mut [&mut Vec<u8>],
) -> Result<(), vyre_driver::BackendError> {
    if views.len() != outputs.len() {
        return Err(vyre_driver::BackendError::new(format!(
            "WGPU resident ranged batch download fused {} output view(s) for {} requested output slot(s). Fix: keep resident transfer fusion cardinality-preserving before materializing outputs.",
            views.len(),
            outputs.len()
        )));
    }
    for (view_index, (view, output)) in views.iter().copied().zip(outputs.iter_mut()).enumerate() {
        if view.byte_len != 0 {
            let Some(copy) = copies.get(view.copy_slot) else {
                return Err(vyre_driver::BackendError::new(format!(
                    "WGPU resident ranged batch download view {view_index} references missing fused copy slot {}. Fix: rebuild the fused readback plan before materializing outputs.",
                    view.copy_slot
                )));
            };
            let view_end = view.byte_offset.checked_add(view.byte_len).ok_or_else(|| {
                vyre_driver::BackendError::new(format!(
                    "WGPU resident ranged batch download view {view_index} overflows usize at offset {} len {}. Fix: rebuild the fused readback plan before materializing outputs.",
                    view.byte_offset, view.byte_len
                ))
            })?;
            if view_end > copy.byte_len {
                return Err(vyre_driver::BackendError::new(format!(
                    "WGPU resident ranged batch download view {view_index} requested bytes [{}..{}) from a {} byte fused output. Fix: rebuild the fused readback plan before materializing outputs.",
                    view.byte_offset, view_end, copy.byte_len
                )));
            }
        }
        vyre_driver::allocation::try_reserve_vec_to_capacity(*output, view.byte_len).map_err(
            |source| {
                vyre_driver::BackendError::new(format!(
                    "WGPU resident ranged batch download could not reserve {} output byte(s) for view {view_index}: {source}. Fix: split the resident readback batch before materializing outputs.",
                    view.byte_len
                ))
            },
        )?;
    }
    Ok(())
}

fn validate_resident_readback_range(
    handle_id: u64,
    allocation_len: u64,
    byte_offset: u64,
    byte_len: u64,
    context: &'static str,
) -> Result<(), vyre_driver::BackendError> {
    let end = byte_offset.checked_add(byte_len).ok_or_else(|| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident {context} overflows u64 at offset {byte_offset} len {byte_len} for handle {handle_id}. Fix: split the readback before staging resident download."
        ))
    })?;
    if end > allocation_len {
        return Err(vyre_driver::BackendError::new(format!(
            "WGPU resident {context} requested byte range [{byte_offset}..{end}) from allocation {allocation_len} on handle {handle_id}. Fix: clamp the readback to the resident buffer size or split the resource."
        )));
    }
    Ok(())
}

fn copy_fused_resident_view_into(
    fused_outputs: &[Vec<u8>],
    view: ResidentTransferView,
    output: &mut Vec<u8>,
) -> Result<(), vyre_driver::BackendError> {
    if view.byte_len == 0 {
        output.clear();
        return Ok(());
    }
    let Some(fused_output) = fused_outputs.get(view.copy_slot) else {
        return Err(vyre_driver::BackendError::new(format!(
            "WGPU resident ranged batch download view references missing fused copy slot {}. Fix: rebuild the fused readback plan before materializing outputs.",
            view.copy_slot
        )));
    };
    let view_end = view.byte_offset.checked_add(view.byte_len).ok_or_else(|| {
        vyre_driver::BackendError::new(format!(
            "WGPU resident ranged batch download view overflows usize at offset {} len {}. Fix: rebuild the fused readback plan before materializing outputs.",
            view.byte_offset, view.byte_len
        ))
    })?;
    let Some(bytes) = fused_output.get(view.byte_offset..view_end) else {
        return Err(vyre_driver::BackendError::new(format!(
            "WGPU resident ranged batch download view requested bytes [{}..{}) from a {} byte fused output. Fix: rebuild the fused readback plan before materializing outputs.",
            view.byte_offset,
            view_end,
            fused_output.len()
        )));
    };
    if output.len() == bytes.len() {
        output.copy_from_slice(bytes);
        return Ok(());
    }
    if bytes.len() > output.capacity() {
        output
            .try_reserve_exact(bytes.len() - output.capacity())
            .map_err(|source| {
                vyre_driver::BackendError::new(format!(
                    "WGPU resident ranged batch download could not reserve {} output byte(s): {source}. Fix: split the resident readback batch before materializing outputs.",
                    bytes.len()
                ))
            })?;
    }
    output.clear();
    output.extend_from_slice(bytes);
    Ok(())
}
