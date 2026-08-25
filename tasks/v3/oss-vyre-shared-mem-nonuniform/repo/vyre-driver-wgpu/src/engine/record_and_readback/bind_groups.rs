use super::binding_lookup::BindingLookup;
use super::{GpuBuffers, RecordAndReadback};
use crate::allocation::reserve_smallvec_to_capacity;
use smallvec::SmallVec;
use std::sync::Arc;
use vyre_driver::BackendError;

pub(super) fn build_bind_groups(
    device: &wgpu::Device,
    request: &RecordAndReadback<'_>,
    gpu_buffers: &GpuBuffers,
    gpu_idx_by_binding: &BindingLookup,
    buffer_ids: &mut Vec<u64>,
    bound_indices: &mut Vec<usize>,
) -> Result<SmallVec<[Arc<wgpu::BindGroup>; 4]>, BackendError> {
    let mut bind_groups: SmallVec<[Arc<wgpu::BindGroup>; 4]> = SmallVec::new();
    reserve_smallvec_to_capacity(
        &mut bind_groups,
        request.bind_group_layouts.len(),
        "record-and-readback bind groups",
        "bind group",
        "split the bind group layout set before dispatch",
    )?;
    for (group_index, layout) in request.bind_group_layouts.iter().enumerate() {
        let group_index_u32 = u32::try_from(group_index).map_err(|_| {
            BackendError::new(
                "record-and-readback bind group index exceeds u32::MAX. Fix: reduce bind group fanout before dispatch.",
            )
        })?;
        buffer_ids.clear();
        bound_indices.clear();
        for info in request
            .buffer_bindings
            .iter()
            .filter(|b| b.group == group_index_u32)
        {
            if info.kind == vyre_foundation::ir::MemoryKind::Shared {
                continue;
            }
            let idx = gpu_idx_by_binding.get(info.binding).ok_or_else(|| {
                BackendError::new(format!(
                    "GPU buffer for binding {} (`{}`) missing. Fix: ensure all declared buffers are allocated.",
                    info.binding, info.name
                ))
            })?;
            let (buffer, logical_size_bytes) =
                gpu_buffers
                    .get(idx)
                    .map(|(_, buf, size)| (buf, size))
                    .ok_or_else(|| {
                        BackendError::new(format!(
                            "GPU buffer for binding {} (`{}`) missing. Fix: ensure all declared buffers are allocated.",
                            info.binding, info.name
                        ))
            })?;
            buffer_ids.push(buffer.id());
            buffer_ids.push(padded_wgpu_u64(
                *logical_size_bytes,
                "record-and-readback bind-group cache key byte length",
            )?);
            bound_indices.push(idx);
        }
        let layout_id = Arc::as_ptr(layout).addr();
        if let Some(cached) = request
            .bind_group_cache
            .and_then(|cache| cache.get_by_ids(layout_id, buffer_ids.as_slice()))
        {
            bind_groups.push(cached);
            continue;
        }

        let mut entries: SmallVec<[wgpu::BindGroupEntry<'_>; 16]> = SmallVec::new();
        reserve_smallvec_to_capacity(
            &mut entries,
            bound_indices.len(),
            "record-and-readback bind groups",
            "bind group entry",
            "split the bind group binding set before dispatch",
        )?;
        for &idx in bound_indices.iter() {
            let (binding, buffer, logical_size_bytes) = gpu_buffers.get(idx).ok_or_else(|| {
                BackendError::new(format!(
                    "GPU buffer index {idx} missing while building bind group {group_index}. Fix: keep bind-group scratch indices synchronized with allocated buffers."
                ))
            })?;
            let buffer_arc = buffer.buffer();
            let bind_size = wgpu::BufferSize::new(padded_wgpu_u64(
                *logical_size_bytes,
                "record-and-readback bind-group binding size",
            )?);
            entries.push(wgpu::BindGroupEntry {
                binding: *binding,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: buffer_arc,
                    offset: 0,
                    size: bind_size,
                }),
            });
        }
        let bind_group = if let Some(cache) = request.bind_group_cache {
            cache.insert_by_ids(
                layout_id,
                buffer_ids.as_slice(),
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(request.labels.bind_group),
                    layout,
                    entries: &entries,
                }),
            )
        } else {
            Arc::new(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(request.labels.bind_group),
                layout,
                entries: &entries,
            }))
        };
        bind_groups.push(bind_group);
    }
    Ok(bind_groups)
}

fn padded_wgpu_u64(size: u64, label: &'static str) -> Result<u64, BackendError> {
    let normalized = size.max(4);
    let remainder = normalized % 4;
    if remainder == 0 {
        return Ok(normalized);
    }
    normalized.checked_add(4 - remainder).ok_or_else(|| {
        BackendError::new(format!(
            "{label} overflows u64 while padding to WGPU's 4-byte buffer alignment. Fix: split the dispatch buffer."
        ))
    })
}
