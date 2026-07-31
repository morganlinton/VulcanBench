//! Persistent resource resolution for WGPU pipeline dispatch.
//!
//! This module maps public `Resource` inputs to live GPU buffer handles and
//! allocates backend-owned trap sidecars. Dispatch modules consume the resolved
//! handles; the parent pipeline module stays out of resource ownership policy.

use smallvec::SmallVec;
use vyre_driver::BackendError;
use vyre_lower::TRAP_SIDECAR_WORDS;

use crate::numeric::usize_to_u64;
use crate::pipeline::binding::{usage_for_binding, validate_handle};
use crate::pipeline::{BufferBindingInfo, WgpuPipeline};

pub(crate) struct ResolvedPersistentResources {
    pub(crate) inputs: SmallVec<[crate::buffer::GpuBufferHandle; 8]>,
    pub(crate) outputs: SmallVec<[crate::buffer::GpuBufferHandle; 8]>,
    pub(crate) output_resources: SmallVec<[vyre_driver::Resource; 8]>,
}

impl WgpuPipeline {
    pub(crate) fn resolve_persistent_resources(
        &self,
        resources: &[vyre_driver::Resource],
        queue: &wgpu::Queue,
    ) -> Result<ResolvedPersistentResources, BackendError> {
        self.resolve_persistent_resources_impl(resources, queue, false)
    }

    pub(crate) fn resolve_persistent_resources_for_resource_outputs(
        &self,
        resources: &[vyre_driver::Resource],
        queue: &wgpu::Queue,
    ) -> Result<ResolvedPersistentResources, BackendError> {
        self.resolve_persistent_resources_impl(resources, queue, true)
    }

    fn resolve_persistent_resources_impl(
        &self,
        resources: &[vyre_driver::Resource],
        queue: &wgpu::Queue,
        return_resource_outputs: bool,
    ) -> Result<ResolvedPersistentResources, BackendError> {
        let binding_capacity = self.buffer_bindings.len();
        let mut inputs =
            SmallVec::<[crate::buffer::GpuBufferHandle; 8]>::with_capacity(binding_capacity);
        let mut outputs =
            SmallVec::<[crate::buffer::GpuBufferHandle; 8]>::with_capacity(binding_capacity);
        let mut output_resources =
            SmallVec::<[vyre_driver::Resource; 8]>::with_capacity(self.output_bindings.len());
        let mut resource_index = 0usize;

        for info in self.buffer_bindings.iter() {
            if info.kind == vyre_foundation::ir::MemoryKind::Shared {
                continue;
            }
            if info.internal_trap {
                inputs.push(self.allocate_internal_trap_handle()?);
                continue;
            }
            let resource = resources.get(resource_index).ok_or_else(|| {
                BackendError::new(format!(
                    "persistent handle dispatch missing resource for binding {} (`{}`). Fix: pass one resource per public non-shared binding in BufferDecl order.",
                    info.binding, info.name
                ))
            })?;
            resource_index += 1;
            let handle = self.resolve_persistent_resource(info, resource, queue)?;
            if info.is_output {
                if return_resource_outputs {
                    match resource {
                        vyre_driver::Resource::Resident(id) => {
                            output_resources.push(vyre_driver::Resource::Resident(*id));
                        }
                        vyre_driver::Resource::Borrowed(_) => {
                            return Err(BackendError::new(format!(
                                "persistent resident-output dispatch cannot return borrowed output binding {} (`{}`). Fix: allocate a resident output buffer and pass Resource::Resident so the backend can skip host readback.",
                                info.binding, info.name
                            )));
                        }
                    }
                }
                outputs.push(handle);
            } else {
                inputs.push(handle);
            }
        }

        if resource_index != resources.len() {
            return Err(BackendError::new(format!(
                "persistent handle dispatch received {} resources but consumed {resource_index}. Fix: pass resources in public non-shared BufferDecl order without extra handles.",
                resources.len()
            )));
        }

        Ok(ResolvedPersistentResources {
            inputs,
            outputs,
            output_resources,
        })
    }

    fn resolve_persistent_resource(
        &self,
        info: &BufferBindingInfo,
        resource: &vyre_driver::Resource,
        queue: &wgpu::Queue,
    ) -> Result<crate::buffer::GpuBufferHandle, BackendError> {
        match resource {
            vyre_driver::Resource::Resident(id) => {
                let handle = crate::buffer::GpuBufferHandle::from_resident_id(*id).ok_or_else(|| {
                    BackendError::new(format!(
                        "resident buffer handle {id} for binding {} (`{}`) is not live. Fix: keep resident buffers alive until dispatch and never pass stale Resource::Resident ids.",
                        info.binding, info.name
                    ))
                })?;
                validate_handle("persistent", info, &handle)?;
                Ok(handle)
            }
            vyre_driver::Resource::Borrowed(bytes) => {
                let byte_len = self.persistent_resource_byte_len(info, Some(bytes.as_slice()))?;
                let byte_len_u64 = usize_to_u64(byte_len, "persistent resource bytes")?;
                let handle = self
                    .persistent_pool
                    .acquire(byte_len_u64, usage_for_binding(info)?)?;
                if !info.is_output || info.preserve_input_contents {
                    crate::buffer::write_padded(
                        queue,
                        handle.buffer(),
                        bytes,
                        handle.allocation_len(),
                    )?;
                }
                validate_handle("persistent", info, &handle)?;
                Ok(handle)
            }
        }
    }

    fn allocate_internal_trap_handle(
        &self,
    ) -> Result<crate::buffer::GpuBufferHandle, BackendError> {
        self.persistent_pool.acquire(
            u64::from(TRAP_SIDECAR_WORDS) * 4,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        )
    }

    fn persistent_resource_byte_len(
        &self,
        info: &BufferBindingInfo,
        data: Option<&[u8]>,
    ) -> Result<usize, BackendError> {
        if info.is_output {
            let output = self.output_binding(info.binding)?;
            let byte_len = output.word_count.checked_mul(4).ok_or_else(|| {
                BackendError::new(format!(
                    "persistent output `{}` size overflows usize. Fix: reduce its element count.",
                    output.name
                ))
            })?;
            if let Some(bytes) = data {
                if bytes.len() > byte_len {
                    return Err(BackendError::new(format!(
                        "persistent output `{}` received {} initialization bytes but declares only {byte_len}. Fix: resize the output BufferDecl or pass bytes matching the compiled output layout.",
                        output.name,
                        bytes.len()
                    )));
                }
            }
            return Ok(byte_len);
        }
        crate::pipeline::persistent::binding_padded_size(info, data)
    }
}
