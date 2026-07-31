use super::binding_lookup::BindingLookup;
use super::{GpuBuffers, RecordAndReadback};
use vyre_driver::BackendError;

pub(super) fn record_buffer_clears(
    encoder: &mut wgpu::CommandEncoder,
    request: &RecordAndReadback<'_>,
    gpu_buffers: &GpuBuffers,
    gpu_idx_by_binding: &BindingLookup,
    clear_requests: &mut Vec<(u32, u64, u64)>,
) -> Result<(), BackendError> {
    for (binding, offset, size) in clear_requests.drain(..) {
        let (_, buf, _) = gpu_idx_by_binding
            .get(binding)
            .and_then(|idx| gpu_buffers.get(idx))
            .ok_or_else(|| {
                BackendError::new(format!(
                    "GPU buffer for binding {} missing during clear. Fix: internal invariant violation.",
                    binding
                ))
            })?;
        encoder.clear_buffer(buf.buffer(), offset, Some(size));
    }

    for output in request.output_bindings.iter() {
        let info = request
            .buffer_bindings
            .iter()
            .find(|info| info.binding == output.binding)
            .ok_or_else(|| {
                BackendError::new(format!(
                    "missing binding metadata for output `{}`. Fix: keep buffer_bindings synchronized with output_bindings.",
                    output.name
                ))
            })?;
        if info.preserve_input_contents {
            continue;
        }
        if let Some((_, buf, _)) = gpu_idx_by_binding
            .get(output.binding)
            .and_then(|idx| gpu_buffers.get(idx))
        {
            let clear_size = u64::try_from(output.word_count)
                .map_err(|source| {
                    BackendError::new(format!(
                        "output `{}` word count cannot fit u64 for clear_buffer: {source}. Fix: reduce its element count.",
                        output.name
                    ))
                })?
                .checked_mul(4)
                .ok_or_else(|| {
                BackendError::new(format!(
                    "clear_buffer size overflows u64 for output `{}`. Fix: reduce its element count.",
                    output.name
                ))
            })?;
            encoder.clear_buffer(buf.buffer(), 0, Some(clear_size));
        }
    }

    Ok(())
}
