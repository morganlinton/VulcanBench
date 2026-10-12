//! Shared binding validation and output-clear helpers for wgpu pipelines.

use vyre_driver::BackendError;

use crate::buffer::GpuBufferHandle;
use crate::numeric::usize_to_u64;
use crate::pipeline::{element_size_bytes, BufferBindingInfo, OutputBindingLayout};

/// Return true when a binding consumes one caller-provided borrowed input slot.
///
/// Pure outputs are allocated by the backend and must not shift subsequent
/// inputs. Read/write live-outs with `preserve_input_contents` are both inputs
/// and outputs, so they intentionally consume one caller input slot.
pub(crate) fn consumes_host_input(info: &BufferBindingInfo) -> bool {
    info.kind != vyre_foundation::ir::MemoryKind::Shared
        && !info.internal_trap
        && (!info.is_output || info.preserve_input_contents)
}

/// Required wgpu usage flags for a compiled binding.
pub(crate) fn usage_for_binding(
    info: &BufferBindingInfo,
) -> Result<wgpu::BufferUsages, BackendError> {
    let _binding_contract = (&info.access, &info.hints);
    if info.internal_trap {
        return Ok(wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST);
    }
    if info.is_output {
        return Ok(wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::INDIRECT);
    }
    match info.kind {
        vyre_foundation::ir::MemoryKind::Readonly | vyre_foundation::ir::MemoryKind::Global => {
            Ok(wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::INDIRECT)
        }
        vyre_foundation::ir::MemoryKind::Uniform | vyre_foundation::ir::MemoryKind::Push => {
            Ok(wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST)
        }
        vyre_foundation::ir::MemoryKind::Shared => Err(BackendError::new(
            "shared memory reached wgpu binding validation. Fix: lower Shared memory into workgroup variables before dispatch.",
        )),
        vyre_foundation::ir::MemoryKind::Local => Err(BackendError::new(format!(
            "buffer `{}` reached wgpu allocation with MemoryKind::Local. Fix: lower Local regions into shader function variables before dispatch.",
            info.name
        ))),
        _ => Err(BackendError::new(format!(
            "buffer `{}` uses an unknown future MemoryKind in wgpu allocation. Fix: update vyre-wgpu before dispatching this Program.",
            info.name
        ))),
    }
}

/// Validate that a resident handle satisfies a binding's usage and size.
pub(crate) fn validate_handle(
    mode: &str,
    info: &BufferBindingInfo,
    handle: &GpuBufferHandle,
) -> Result<(), BackendError> {
    let required = usage_for_binding(info)?;
    if !handle.usage().contains(required) {
        return Err(BackendError::new(format!(
            "{mode} handle for binding {} (`{}`) has usage {:?} but requires {:?}. Fix: allocate the handle with the binding's required usage bits.",
            info.binding,
            info.name,
            handle.usage(),
            required
        )));
    }
    if info.count > 0 {
        let required_bytes = usize::try_from(info.count)
            .map_err(|_| {
                BackendError::new(format!(
                    "buffer `{}` element count cannot fit host usize. Fix: reduce buffer count.",
                    info.name
                ))
            })?
            .checked_mul(element_size_bytes(&info.element)?)
            .ok_or_else(|| {
                BackendError::new(format!(
                    "buffer `{}` declared size overflows usize. Fix: reduce buffer count.",
                    info.name
                ))
            })?;
        let required_bytes_u64 = usize_to_u64(required_bytes, "required binding bytes")?;
        if handle.allocation_len() < required_bytes_u64 {
            return Err(BackendError::new(format!(
                "{mode} handle for binding {} (`{}`) has {} bytes but requires {required_bytes}. Fix: allocate a larger GPU buffer.",
                info.binding,
                info.name,
                handle.allocation_len()
            )));
        }
    }
    Ok(())
}

/// Clear output buffers that do not preserve caller-provided input bytes.
pub(crate) fn clear_outputs_for_bound<F>(
    mode: &str,
    encoder: &mut wgpu::CommandEncoder,
    bound: &[(&BufferBindingInfo, &GpuBufferHandle)],
    mut output_binding: F,
) -> Result<(), BackendError>
where
    F: FnMut(u32) -> Result<OutputBindingLayout, BackendError>,
{
    for (info, handle) in bound {
        if !info.is_output || info.preserve_input_contents {
            continue;
        }
        let output = output_binding(info.binding)?;
        let clear_size = output.word_count.checked_mul(4).ok_or_else(|| {
            BackendError::new(format!(
                "{mode} output clear size overflows usize for `{}`. Fix: reduce its element count.",
                output.name
            ))
        })?;
        let clear_size_u64 = usize_to_u64(clear_size, "output clear bytes")?;
        if handle.allocation_len() < clear_size_u64 {
            return Err(BackendError::new(format!(
                "{mode} output buffer `{}` has {} bytes but dispatch requires {clear_size}. Fix: allocate the output handle with at least the compiled output size.",
                info.name,
                handle.allocation_len()
            )));
        }
        encoder.clear_buffer(handle.buffer(), 0, Some(clear_size_u64));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn info(
        is_output: bool,
        preserve_input_contents: bool,
        internal_trap: bool,
    ) -> BufferBindingInfo {
        BufferBindingInfo {
            group: 0,
            binding: 1,
            name: Arc::from("buf"),
            access: vyre_foundation::ir::BufferAccess::ReadWrite,
            kind: vyre_foundation::ir::MemoryKind::Global,
            hints: vyre_foundation::ir::MemoryHints::default(),
            element: vyre_foundation::ir::DataType::U32,
            count: 4,
            is_output,
            preserve_input_contents,
            internal_trap,
        }
    }

    #[test]
    fn pure_outputs_do_not_consume_host_input_slots() {
        assert!(!consumes_host_input(&info(true, false, false)));
    }

    #[test]
    fn preserved_live_outs_consume_host_input_slots() {
        assert!(consumes_host_input(&info(true, true, false)));
    }

    #[test]
    fn internal_traps_do_not_consume_host_input_slots() {
        assert!(!consumes_host_input(&info(false, false, true)));
    }
}
