use super::binding_lookup::BindingLookup;
use super::readback::{SubmittedMap, SubmittedReadback};
use super::{pool_backend_error, GpuBuffers, RecordAndReadback};
use crate::allocation::reserve_smallvec_to_capacity;
use crate::numeric::usize_to_u64;
use smallvec::SmallVec;
use std::sync::Arc;
use vyre_driver::BackendError;
use vyre_emit_naga::program::TRAP_SIDECAR_WORDS;

pub(super) fn record_readback_copies(
    device: &wgpu::Device,
    pool: &crate::buffer::BufferPool,
    encoder: &mut wgpu::CommandEncoder,
    request: &RecordAndReadback<'_>,
    gpu_buffers: &GpuBuffers,
    gpu_idx_by_binding: &BindingLookup,
) -> Result<SmallVec<[SubmittedMap; 4]>, BackendError> {
    let output_count = request.output_bindings.len();
    let trap_info = request
        .buffer_bindings
        .iter()
        .find(|info| info.internal_trap);
    let trap_readback_count = usize::from(trap_info.is_some());
    let readback_count = output_count
        .checked_add(trap_readback_count)
        .ok_or_else(|| {
            BackendError::new(
                "readback staging count overflowed host usize. Fix: split the output set before recording GPU readbacks.",
            )
        })?;
    let trap_readback_size = u64::from(TRAP_SIDECAR_WORDS) * 4;
    let use_readback_rings = request.readback_rings.is_some();
    let readback_rings = request.readback_rings;
    let mut readback_buffers = SmallVec::<[SubmittedMap; 4]>::new();
    reserve_smallvec_to_capacity(
        &mut readback_buffers,
        readback_count,
        "readback staging",
        "submitted readback descriptor",
        "split the output set before recording GPU readbacks",
    )?;
    if let Some(ring_set) = readback_rings {
        let mut readback_rings_by_class: SmallVec<
            [(u64, Arc<crate::runtime::readback_ring::ReadbackRing>); 4],
        > = SmallVec::new();
        reserve_smallvec_to_capacity(
            &mut readback_rings_by_class,
            readback_count,
            "readback staging",
            "readback-ring class descriptor",
            "split the output set before recording GPU readbacks",
        )?;
        let mut ring_for_size = |byte_len: u64| -> Result<
            Arc<crate::runtime::readback_ring::ReadbackRing>,
            BackendError,
        > {
            let capacity =
                crate::runtime::readback_ring::ReadbackRingSet::capacity_class_for(byte_len)?;
            if let Some((_, ring)) = readback_rings_by_class
                .iter()
                .find(|(byte_len, _)| *byte_len == capacity)
            {
                return Ok(Arc::clone(ring));
            }
            let ring = ring_set.ring_for_capacity(device, capacity)?;
            readback_rings_by_class.push((capacity, Arc::clone(&ring)));
            Ok(ring)
        };
        for (output_idx, output) in request.output_bindings.iter().enumerate() {
            let readback_size = usize_to_u64(output.layout.copy_size, "output readback copy size")?;
            let readback_offset =
                usize_to_u64(output.layout.copy_offset, "output readback copy offset")?;
            let output_buffer = gpu_idx_by_binding
                .get(output.binding)
                .and_then(|idx| gpu_buffers.get(idx))
                .map(|(_, buf, _)| buf)
                .ok_or_else(|| {
                    BackendError::new(format!(
                        "GPU output buffer `{}` was not allocated. Fix: keep writable bindings synchronized during dispatch setup.",
                        output.name
                    ))
                })?;
            let ring = ring_for_size(readback_size)?;
            let ticket = ring.record_copy(
                device,
                encoder,
                output_buffer.buffer(),
                readback_offset,
                readback_size,
            )?;
            readback_buffers.push((Some(output_idx), SubmittedReadback::Ring { ring, ticket }));
        }
        if let Some(trap_info) = trap_info {
            let ring = ring_for_size(trap_readback_size)?;
            let trap_buffer = gpu_idx_by_binding
                .get(trap_info.binding)
                .and_then(|idx| gpu_buffers.get(idx))
                .map(|(_, buf, _)| buf)
                .ok_or_else(|| {
                    BackendError::new(
                        "GPU trap sidecar was not allocated. Fix: keep internal trap binding metadata synchronized during dispatch setup.",
                    )
                })?;
            let ticket =
                ring.record_copy(device, encoder, trap_buffer.buffer(), 0, trap_readback_size)?;
            readback_buffers.push((None, SubmittedReadback::Ring { ring, ticket }));
        }
    } else {
        for (output_idx, output) in request.output_bindings.iter().enumerate() {
            let readback_size = usize_to_u64(output.layout.copy_size, "output readback copy size")?;
            let readback_offset =
                usize_to_u64(output.layout.copy_offset, "output readback copy offset")?;
            let output_buffer = gpu_idx_by_binding
                .get(output.binding)
                .and_then(|idx| gpu_buffers.get(idx))
                .map(|(_, buf, _)| buf)
                .ok_or_else(|| {
                    BackendError::new(format!(
                        "GPU output buffer `{}` was not allocated. Fix: keep writable bindings synchronized during dispatch setup.",
                        output.name
                    ))
                })?;
            let readback_buffer = pool
                .acquire(
                    readback_size,
                    wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                )
                .map_err(pool_backend_error)?;
            encoder.copy_buffer_to_buffer(
                output_buffer.buffer(),
                readback_offset,
                readback_buffer.buffer(),
                0,
                readback_size,
            );
            readback_buffers.push((
                Some(output_idx),
                SubmittedReadback::Pooled {
                    buffer: readback_buffer,
                    mapped_range: 0..readback_size,
                },
            ));
        }
    }
    let Some(trap_info) = trap_info else {
        return Ok(readback_buffers);
    };
    if use_readback_rings {
        return Ok(readback_buffers);
    }
    let readback_buffer = pool
        .acquire(
            trap_readback_size,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        )
        .map_err(pool_backend_error)?;
    let trap_buffer = gpu_idx_by_binding
        .get(trap_info.binding)
        .and_then(|idx| gpu_buffers.get(idx))
        .map(|(_, buf, _)| buf)
        .ok_or_else(|| {
            BackendError::new(
                "GPU trap sidecar was not allocated. Fix: keep internal trap binding metadata synchronized during dispatch setup.",
            )
        })?;
    encoder.copy_buffer_to_buffer(
        trap_buffer.buffer(),
        0,
        readback_buffer.buffer(),
        0,
        trap_readback_size,
    );
    readback_buffers.push((
        None,
        SubmittedReadback::Pooled {
            buffer: readback_buffer,
            mapped_range: 0..4,
        },
    ));
    Ok(readback_buffers)
}
