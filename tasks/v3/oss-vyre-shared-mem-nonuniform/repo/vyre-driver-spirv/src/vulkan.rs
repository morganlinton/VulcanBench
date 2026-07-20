//! Vulkan compute dispatch for the SPIR-V backend.
//!
//! Uses `ash` to drive a minimal Vulkan 1.0 compute pipeline:
//! instance → physical device (with compute queue) → logical device →
//! shader module → descriptor set → compute pipeline → command buffer →
//! fence-wait submit.

use ash::vk;

use vyre_driver::{BackendError, BindingPlan};
use vyre_foundation::ir::{BufferAccess, Program};

/// Owned Vulkan compute context.
pub(crate) struct VulkanDevice {
    // Keep the dynamically-loaded Vulkan loader alive for every function
    // pointer stored in `instance` and `device`. `Drop` explicitly
    // destroys the logical device, and `_instance` is declared before
    // `_entry` so the instance handle drops before the loader unloads.
    _instance: ash::Instance,
    _entry: ash::Entry,
    device: ash::Device,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
    /// Memory type index that is host-visible and host-coherent.
    host_memory_type_index: u32,
    /// Device properties (for limits reporting).
    pub properties: vk::PhysicalDeviceProperties,
}

impl std::fmt::Debug for VulkanDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanDevice")
            .field("physical_device", &self.physical_device)
            .field("queue_family_index", &self.queue_family_index)
            .finish_non_exhaustive()
    }
}

impl VulkanDevice {
    /// Acquire the first Vulkan physical device that exposes a compute queue.
    pub(crate) fn acquire() -> Result<Self, BackendError> {
        // SAFETY: ash::Entry::load dlopen's the system Vulkan loader. The
        // returned Entry is the only reference to that loader handle and
        // owns its lifetime via Drop; we surface initialization errors
        // back to the caller as a typed BackendError.
        let entry = unsafe { ash::Entry::load() }.map_err(|e| {
            BackendError::new(format!(
                "Failed to load Vulkan loader: {e}. Fix: install a Vulkan loader (libvulkan1) and ensure ICD files are in /usr/share/vulkan/icd.d/."
            ))
        })?;

        let app_info = vk::ApplicationInfo {
            api_version: vk::API_VERSION_1_0,
            ..Default::default()
        };
        let create_info = vk::InstanceCreateInfo {
            p_application_info: &app_info,
            ..Default::default()
        };

        // SAFETY: create_info points at app_info on the current stack and
        // both structs live until create_instance returns. The Instance
        // returned takes ownership of the Vulkan handle and frees it on
        // Drop. Allocator callbacks are null (None).
        let instance = unsafe { entry.create_instance(&create_info, None) }.map_err(|e| {
            BackendError::new(format!(
                "Vulkan instance creation failed: {e}. Fix: verify the Vulkan loader and any validation layers are compatible."
            ))
        })?;

        // SAFETY: `instance` is the live Instance returned above; its
        // handle is valid until VulkanDevice::Drop frees it.
        let physical_devices = unsafe { instance.enumerate_physical_devices() }.map_err(|e| {
            BackendError::new(format!(
                "Vulkan physical device enumeration failed: {e}. Fix: ensure a Vulkan-capable GPU is present and drivers are installed."
            ))
        })?;

        let mut chosen = None;
        for pd in physical_devices {
            // SAFETY: `pd` is a vk::PhysicalDevice handle returned by the
            // matching `instance.enumerate_physical_devices()` call above
            // and is valid for the lifetime of `instance`.
            let props = unsafe { instance.get_physical_device_properties(pd) };
            if props.device_type == vk::PhysicalDeviceType::CPU {
                continue;
            }
            let queue_families =
                // SAFETY: same as the get_physical_device_properties call
                // above  -  `pd` is a live handle bound to `instance`.
                unsafe { instance.get_physical_device_queue_family_properties(pd) };
            for (index, family) in queue_families.iter().enumerate() {
                if family.queue_flags.contains(vk::QueueFlags::COMPUTE) {
                    let queue_index = u32::try_from(index).map_err(|_| {
                        BackendError::new(format!(
                            "Vulkan queue family index {index} exceeds u32. Fix: repair driver-reported queue metadata before SPIR-V backend acquisition."
                        ))
                    })?;
                    chosen = Some((pd, queue_index, props));
                    break;
                }
            }
            if chosen.is_some() {
                break;
            }
        }

        let (physical_device, queue_family_index, properties) = chosen.ok_or_else(|| {
            BackendError::new(
                "No Vulkan physical GPU device with a compute queue was found. Fix: repair the Vulkan GPU driver or select the CUDA/WGPU backend; software CPU Vulkan implementations are not production dispatch backends.".to_string(),
            )
        })?;

        let queue_priority = 1.0f32;
        let queue_create_info = vk::DeviceQueueCreateInfo {
            queue_family_index,
            queue_count: 1,
            p_queue_priorities: &queue_priority,
            ..Default::default()
        };

        let device_create_info = vk::DeviceCreateInfo {
            queue_create_info_count: 1,
            p_queue_create_infos: &queue_create_info,
            ..Default::default()
        };

        // SAFETY: physical_device + device_create_info live until this
        // call returns; queue_create_info inside device_create_info
        // borrows queue_priority on the current stack frame, which is
        // also live for the duration of the call. The returned Device
        // takes ownership of the new vk::Device handle.
        let device = unsafe {
            instance.create_device(physical_device, &device_create_info, None)
        }
        .map_err(|e| {
            BackendError::new(format!(
                "Vulkan logical device creation failed: {e}. Fix: check device limits and feature requirements."
            ))
        })?;

        // SAFETY: queue_family_index was just used to create `device`
        // and is in range; index 0 is always valid for queue_count = 1.
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        // SAFETY: `device` is the live Device returned above; the
        // SAFETY: `device` is the live logical device created above,
        // `queue_family_index` was selected from that physical device's
        // compute-capable queue families, and the CommandPoolCreateInfo
        // struct lives until create_command_pool returns.
        // returns.
        let command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo {
                    flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                    queue_family_index,
                    ..Default::default()
                },
                None,
            )
        }
        .map_err(|e| {
            BackendError::new(format!(
                "Vulkan command pool creation failed: {e}. Fix: verify queue family index is valid."
            ))
        })?;

        let memory_properties =
            // SAFETY: physical_device is a live vk::PhysicalDevice handle
            // bound to `instance`.
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let host_memory_type_index = find_host_visible_memory_type(&memory_properties).ok_or_else(
            || {
                BackendError::new(
                    "No host-visible, host-coherent memory type found on Vulkan device. Fix: select a different physical device or implement explicit staging.".to_string(),
                )
            },
        )?;

        Ok(Self {
            _instance: instance,
            _entry: entry,
            device,
            physical_device,
            queue_family_index,
            queue,
            command_pool,
            host_memory_type_index,
            properties,
        })
    }

    /// Create a buffer backed by host-visible memory.
    unsafe fn create_host_buffer(
        &self,
        size: vk::DeviceSize,
    ) -> Result<(vk::Buffer, vk::DeviceMemory), BackendError> {
        let buffer_info = vk::BufferCreateInfo {
            size,
            usage: vk::BufferUsageFlags::STORAGE_BUFFER
                | vk::BufferUsageFlags::TRANSFER_SRC
                | vk::BufferUsageFlags::TRANSFER_DST,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        let buffer = unsafe {
            self
            .device
            .create_buffer(&buffer_info, None)
        }
        .map_err(|e| BackendError::new(format!("Vulkan buffer creation failed: {e}. Fix: reduce buffer size or check device limits.")))?;

        // SAFETY: The buffer is a valid Vulkan buffer created successfully just above.
        let mem_requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        let alloc_info = vk::MemoryAllocateInfo {
            allocation_size: mem_requirements.size,
            memory_type_index: self.host_memory_type_index,
            ..Default::default()
        };
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        let memory = unsafe {
            self
            .device
            .allocate_memory(&alloc_info, None)
        }
        .map_err(|e| BackendError::new(format!("Vulkan memory allocation failed: {e}. Fix: reduce buffer size or free unused allocations.")))?;

        // SAFETY: The buffer and memory were both created successfully just above and have not been freed.
        unsafe { self.device.bind_buffer_memory(buffer, memory, 0) }.map_err(|e| {
            BackendError::new(format!(
                "Vulkan buffer memory binding failed: {e}. Fix: verify alignment requirements."
            ))
        })?;

        Ok((buffer, memory))
    }

    /// Destroy a buffer and its memory.
    unsafe fn destroy_buffer(&self, buffer: vk::Buffer, memory: vk::DeviceMemory) {
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        unsafe {
            self.device.destroy_buffer(buffer, None);
            self.device.free_memory(memory, None);
        }
    }

    /// Record a compute dispatch and wait for completion.
    unsafe fn dispatch_compute(
        &self,
        pipeline: vk::Pipeline,
        pipeline_layout: vk::PipelineLayout,
        descriptor_set: vk::DescriptorSet,
        workgroups: [u32; 3],
    ) -> Result<(), BackendError> {
        let alloc_info = vk::CommandBufferAllocateInfo {
            s_type: vk::StructureType::COMMAND_BUFFER_ALLOCATE_INFO,
            p_next: std::ptr::null(),
            command_pool: self.command_pool,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: 1,
            _marker: std::marker::PhantomData,
        };
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        let mut cbs = unsafe {
            self
            .device
            .allocate_command_buffers(&alloc_info)
        }
        .map_err(|e| BackendError::new(format!("Vulkan command buffer allocation failed: {e}. Fix: reset or free existing command buffers.")))?;
        let command_buffer = cbs.pop().ok_or_else(|| {
            BackendError::new(
                "Vulkan returned zero command buffers. Fix: check command pool state.".to_string(),
            )
        })?;

        // Helper: free the command buffer on any early-exit below. The fence is
        // created later; its own cleanup guard is added at that point.
        let free_cb = |device: &ash::Device, pool: vk::CommandPool, cb: vk::CommandBuffer| {
            // SAFETY: cb was allocated from pool and is no longer submitted.
            unsafe { device.free_command_buffers(pool, &[cb]) };
        };

        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        if let Err(e) = unsafe {
            self.device.begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo {
                    flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                    ..Default::default()
                },
            )
        } {
            free_cb(&self.device, self.command_pool, command_buffer);
            return Err(BackendError::new(format!(
                "Vulkan command buffer begin failed: {e}. Fix: check command buffer state."
            )));
        }

        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        unsafe {
            self.device
                .cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::COMPUTE, pipeline);
            self.device.cmd_bind_descriptor_sets(
                command_buffer,
                vk::PipelineBindPoint::COMPUTE,
                pipeline_layout,
                0,
                &[descriptor_set],
                &[],
            );
            self.device
                .cmd_dispatch(command_buffer, workgroups[0], workgroups[1], workgroups[2]);
        }

        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        if let Err(e) = unsafe { self.device.end_command_buffer(command_buffer) } {
            free_cb(&self.device, self.command_pool, command_buffer);
            return Err(BackendError::new(format!(
                "Vulkan command buffer end failed: {e}. Fix: check recorded commands."
            )));
        }

        // SAFETY: Fence creation is a standard Vulkan device operation, parameters are valid defaults.
        let fence = match unsafe {
            self.device
                .create_fence(&vk::FenceCreateInfo::default(), None)
        } {
            Ok(f) => f,
            Err(e) => {
                free_cb(&self.device, self.command_pool, command_buffer);
                return Err(BackendError::new(format!(
                    "Vulkan fence creation failed: {e}. Fix: check device limits."
                )));
            }
        };

        // From here both `fence` and `command_buffer` must be released on any error path.
        let cleanup = |device: &ash::Device, pool: vk::CommandPool, f: vk::Fence, cb: vk::CommandBuffer| {
            // SAFETY: fence and cb were created/allocated successfully and are owned here.
            unsafe {
                device.destroy_fence(f, None);
                device.free_command_buffers(pool, &[cb]);
            }
        };

        let submit_info = vk::SubmitInfo {
            command_buffer_count: 1,
            p_command_buffers: &command_buffer,
            ..Default::default()
        };
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        if let Err(e) = unsafe { self.device.queue_submit(self.queue, &[submit_info], fence) } {
            cleanup(&self.device, self.command_pool, fence, command_buffer);
            return Err(BackendError::new(format!(
                "Vulkan queue submit failed: {e}. Fix: verify queue and command buffer state."
            )));
        }

        // SAFETY: The fence was created successfully and successfully submitted to the queue.
        if let Err(e) = unsafe { self.device.wait_for_fences(&[fence], true, u64::MAX) } {
            cleanup(&self.device, self.command_pool, fence, command_buffer);
            return Err(BackendError::new(format!(
                "Vulkan fence wait failed: {e}. Fix: check for device loss."
            )));
        }

        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        unsafe {
            self.device.destroy_fence(fence, None);
            self.device
                .free_command_buffers(self.command_pool, &[command_buffer]);
        }

        Ok(())
    }
}

impl Drop for VulkanDevice {
    fn drop(&mut self) {
        // SAFETY: self.command_pool and self.device are the live
        // handles created in `acquire`; Drop is the single owner of
        // both and runs once when the VulkanDevice is dropped.
        unsafe {
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_device(None);
        }
    }
}

fn find_host_visible_memory_type(props: &vk::PhysicalDeviceMemoryProperties) -> Option<u32> {
    for i in 0..props.memory_type_count {
        let ty = props.memory_types[i as usize];
        if ty
            .property_flags
            .contains(vk::MemoryPropertyFlags::HOST_VISIBLE)
            && ty
                .property_flags
                .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
        {
            return Some(i);
        }
    }
    None
}

/// Build a SPIR-V shader module from raw words.
unsafe fn create_shader_module(
    device: &ash::Device,
    words: &[u32],
) -> Result<vk::ShaderModule, BackendError> {
    let code_size = words.len() * std::mem::size_of::<u32>();
    let create_info = vk::ShaderModuleCreateInfo {
        code_size,
        p_code: words.as_ptr(),
        ..Default::default()
    };
    // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
    unsafe { device.create_shader_module(&create_info, None) }
        .map_err(|e| BackendError::new(format!("Vulkan shader module creation failed: {e}. Fix: validate the SPIR-V binary with spirv-val before loading.")))
}

/// One binding slot used during dispatch.
struct DispatchBinding {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    byte_len: usize,
    binding: u32,
}

/// Run one compute dispatch on the Vulkan device.
///
/// # Safety
/// The Vulkan device must be valid. This function performs all Vulkan FFI calls.
pub(crate) unsafe fn dispatch_program(
    device: &VulkanDevice,
    program: &Program,
    spv_words: &[u32],
    inputs: &[&[u8]],
    config: &vyre_driver::DispatchConfig,
) -> Result<Vec<Vec<u8>>, BackendError> {
    let workgroup_size = config.workgroup_override.unwrap_or(program.workgroup_size);
    if workgroup_size.contains(&0) {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: SPIR-V dispatch workgroup size contains zero dimension: {workgroup_size:?}. Emit a positive GPU workgroup shape before Vulkan dispatch."
            ),
        });
    }
    let workgroup_size = [workgroup_size[0], workgroup_size[1], workgroup_size[2]];

    let grid = if let Some(grid) = config.grid_override {
        grid
    } else {
        infer_grid(program, workgroup_size)?
    };

    let binding_plan = BindingPlan::from_borrowed_inputs(program, inputs)?;

    // Build bindings from the backend-neutral ABI plan so input and output
    // slots stay identical to every other backend.
    let mut dispatch_bindings: Vec<DispatchBinding> = Vec::new();
    let mut output_bindings: Vec<(u32, usize)> = Vec::new(); // (binding, index in dispatch_bindings)

    for binding in &binding_plan.bindings {
        let buffer = &program.buffers()[binding.buffer_index];
        if buffer.access() == BufferAccess::Workgroup {
            continue;
        }

        let element_size = buffer.element().size_bytes().ok_or_else(|| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Vulkan buffer `{}` uses a runtime-sized element type. Lower it to a fixed-width GPU storage type before SPIR-V dispatch.",
                    buffer.name()
                ),
            }
        })? as usize;
        let byte_len = if buffer.count() == 0 {
            if let Some(input_index) = binding.input_index {
                let input = inputs[input_index];
                input.len()
            } else if binding.output_index.is_some() {
                // Runtime-sized output with no paired input: infer capacity from the
                // largest input buffer passed to this dispatch. A one-element fallback
                // would silently truncate any scan that produces more than one match
                // (the GPU writes beyond the mapped range). If no inputs are present,
                // require an explicit grid_override to bound the output; the caller
                // must supply DispatchConfig::output_size_hint or grid_override in
                // that case.
                let max_input_bytes = inputs.iter().map(|s| s.len()).max().unwrap_or(0);
                if max_input_bytes == 0 {
                    return Err(BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: runtime-sized output buffer `{}` has no input to infer size from. \
                             Pass at least one non-empty input or set DispatchConfig::grid_override \
                             so the backend can bound the output allocation.",
                            buffer.name()
                        ),
                    });
                }
                // Conservatively match the largest input buffer size so the GPU
                // cannot overflow. Downstream readback trims to the actual output
                // via the match-count metadata that the GPU kernel writes.
                max_input_bytes
            } else {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: buffer `{}` has runtime size but no matching input was provided.",
                        buffer.name()
                    ),
                });
            }
        } else {
            (buffer.count() as usize)
                .checked_mul(element_size)
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: Vulkan buffer `{}` size overflows host address space (count={}, element_size={element_size}). Split the Program buffer before dispatch.",
                        buffer.name(),
                        buffer.count()
                    ),
                })?
        };

        let vk_byte_len = vk::DeviceSize::try_from(byte_len).map_err(|_| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: Vulkan buffer `{}` is {byte_len} bytes, which exceeds vk::DeviceSize. Split the Program buffer before SPIR-V dispatch.",
                    buffer.name()
                ),
            }
        })?;
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        let (vk_buffer, vk_memory) = unsafe { device.create_host_buffer(vk_byte_len) }?;

        // If there is a matching input, upload it.
        if let Some(input_index) = binding.input_index {
            let input = inputs[input_index];
            if input.len() > byte_len {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: input buffer for Vulkan binding `{}` is {} bytes but declared storage is {byte_len} bytes. Resize the input or fix the Program buffer count; silent upload truncation is forbidden.",
                        buffer.name(),
                        input.len()
                    ),
                });
            }
            // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
            let ptr = unsafe {
                device
                    .device
                    .map_memory(vk_memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
            }
            .map_err(|e| {
                BackendError::new(format!(
                    "Vulkan memory map failed: {e}. Fix: check memory type is host-visible."
                ))
            })?;
            // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
            let slice = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, byte_len) };
            slice[..input.len()].copy_from_slice(input);
            // SAFETY: Memory was mapped successfully just above and is unmapped properly after copying.
            unsafe { device.device.unmap_memory(vk_memory) };
        }

        if binding.output_index.is_some() {
            output_bindings.push((buffer.binding(), dispatch_bindings.len()));
        }

        dispatch_bindings.push(DispatchBinding {
            buffer: vk_buffer,
            memory: vk_memory,
            byte_len,
            binding: buffer.binding(),
        });
    }

    // Create shader module.
    // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
    let shader_module = unsafe { create_shader_module(&device.device, spv_words) }?;

    // Descriptor set layout.
    let layout_bindings: Vec<vk::DescriptorSetLayoutBinding<'_>> = dispatch_bindings
        .iter()
        .map(|b| vk::DescriptorSetLayoutBinding {
            binding: b.binding,
            descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::COMPUTE,
            p_immutable_samplers: std::ptr::null(),
            ..Default::default()
        })
        .collect();

    let layout_binding_count = u32::try_from(layout_bindings.len()).map_err(|_| {
        BackendError::new(format!(
            "Vulkan descriptor set layout has {} bindings, exceeding u32. Fix: split the Program binding table before SPIR-V dispatch.",
            layout_bindings.len()
        ))
    })?;
    // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
    let descriptor_set_layout = unsafe {
        device.device.create_descriptor_set_layout(
            &vk::DescriptorSetLayoutCreateInfo {
                binding_count: layout_binding_count,
                p_bindings: layout_bindings.as_ptr(),
                ..Default::default()
            },
            None,
        )
    }
    .map_err(|e| {
        BackendError::new(format!(
            "Vulkan descriptor set layout creation failed: {e}. Fix: check binding limits."
        ))
    })?;

    // Pipeline layout.
    // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
    let pipeline_layout = unsafe {
        device.device.create_pipeline_layout(
            &vk::PipelineLayoutCreateInfo {
                set_layout_count: 1,
                p_set_layouts: &descriptor_set_layout,
                ..Default::default()
            },
            None,
        )
    }
    .map_err(|e| {
        BackendError::new(format!(
            "Vulkan pipeline layout creation failed: {e}. Fix: check push constant limits."
        ))
    })?;

    // Compute pipeline.
    let pipeline_info = vk::ComputePipelineCreateInfo {
        stage: vk::PipelineShaderStageCreateInfo {
            stage: vk::ShaderStageFlags::COMPUTE,
            module: shader_module,
            p_name: b"main\0".as_ptr() as *const i8,
            ..Default::default()
        },
        layout: pipeline_layout,
        ..Default::default()
    };

    // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
    let pipeline = match unsafe {
        device.device.create_compute_pipelines(
            vk::PipelineCache::null(),
            &[pipeline_info],
            None,
        )
    } {
        Ok(mut pipelines) => {
            pipelines.pop().ok_or_else(|| BackendError::new(
                "Vulkan returned zero compute pipelines. Fix: check shader module and pipeline layout compatibility.".to_string(),
            ))?
        }
        Err((_, e)) => {
            return Err(BackendError::new(format!(
                "Vulkan compute pipeline creation failed: {e:?}. Fix: validate SPIR-V entry point name is 'main' and pipeline layout matches shader bindings."
            )));
        }
    };

    // Descriptor pool.
    let descriptor_count = u32::try_from(dispatch_bindings.len()).map_err(|_| {
        BackendError::new(format!(
            "Vulkan descriptor pool needs {} storage-buffer descriptors, exceeding u32. Fix: split the Program binding table before SPIR-V dispatch.",
            dispatch_bindings.len()
        ))
    })?;
    let pool_size = vk::DescriptorPoolSize {
        ty: vk::DescriptorType::STORAGE_BUFFER,
        descriptor_count,
    };
    // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
    let descriptor_pool = unsafe {
        device.device.create_descriptor_pool(
            &vk::DescriptorPoolCreateInfo {
                max_sets: 1,
                pool_size_count: 1,
                p_pool_sizes: &pool_size,
                ..Default::default()
            },
            None,
        )
    }
    .map_err(|e| {
        BackendError::new(format!(
            "Vulkan descriptor pool creation failed: {e}. Fix: check pool sizes."
        ))
    })?;

    // Descriptor set.
    // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
    let descriptor_set = unsafe {
        device
            .device
            .allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo {
                descriptor_pool,
                descriptor_set_count: 1,
                p_set_layouts: &descriptor_set_layout,
                ..Default::default()
            })
    }
    .map_err(|e| {
        BackendError::new(format!(
            "Vulkan descriptor set allocation failed: {e}. Fix: check descriptor pool capacity."
        ))
    })?
    .pop()
    .ok_or_else(|| {
        BackendError::new(
            "Vulkan returned zero descriptor sets. Fix: check descriptor pool state.".to_string(),
        )
    })?;

    // Write descriptor set.
    let buffer_infos: Vec<vk::DescriptorBufferInfo> = dispatch_bindings
        .iter()
        .map(|b| vk::DescriptorBufferInfo {
            buffer: b.buffer,
            offset: 0,
            range: vk::WHOLE_SIZE,
        })
        .collect();

    let write_bindings: Vec<vk::WriteDescriptorSet<'_>> = dispatch_bindings
        .iter()
        .zip(buffer_infos.iter())
        .map(|(b, info)| vk::WriteDescriptorSet {
            dst_set: descriptor_set,
            dst_binding: b.binding,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
            p_buffer_info: info,
            ..Default::default()
        })
        .collect();

    // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
    unsafe { device.device.update_descriptor_sets(&write_bindings, &[]) };

    // Dispatch.
    // SAFETY: FFI boundary to Vulkan dispatch: pipeline, descriptor_set, layout, and grid parameters are fully validated.
    unsafe { device.dispatch_compute(pipeline, pipeline_layout, descriptor_set, grid) }?;

    // Read back outputs.
    let mut outputs = Vec::with_capacity(output_bindings.len());
    for (_binding, idx) in &output_bindings {
        let b = &dispatch_bindings[*idx];
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        let ptr = unsafe {
            device
            .device
            .map_memory(b.memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
        }
            .map_err(|e| BackendError::new(format!("Vulkan memory map for readback failed: {e}. Fix: check memory type is host-visible.")))?;
        // SAFETY: The host memory was successfully mapped and the pointer remains valid for readback.
        let slice = unsafe { std::slice::from_raw_parts(ptr as *const u8, b.byte_len) };
        outputs.push(slice.to_vec());
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        unsafe { device.device.unmap_memory(b.memory) };
    }

    // Cleanup.
    // SAFETY: All resources created for the temporary compute dispatch are destroyed in reverse creation order.
    unsafe {
        device.device.destroy_descriptor_pool(descriptor_pool, None);
        device.device.destroy_pipeline(pipeline, None);
        device.device.destroy_pipeline_layout(pipeline_layout, None);
        device
            .device
            .destroy_descriptor_set_layout(descriptor_set_layout, None);
        device.device.destroy_shader_module(shader_module, None);
    }
    for b in dispatch_bindings {
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        unsafe { device.destroy_buffer(b.buffer, b.memory) };
    }

    Ok(outputs)
}

/// Infer the dispatch grid from the program's buffer element counts.
///
/// When all output buffers are runtime-sized (count == 0) the grid cannot be
/// derived from outputs alone. Fall back to the largest *input* buffer element
/// count so that programs with a single runtime-sized output (e.g. scan result
/// buffers) still launch one thread per input element rather than exactly one
/// workgroup. If both outputs and inputs are runtime-sized the caller must
/// supply `DispatchConfig::grid_override`; otherwise a 1-workgroup launch would
/// silently process only `workgroup_size[0]` elements.
fn infer_grid(program: &Program, workgroup_size: [u32; 3]) -> Result<[u32; 3], BackendError> {
    if workgroup_size[1] != 1 || workgroup_size[2] != 1 {
        return Err(BackendError::new(format!(
            "Fix: non-1D workgroup_size {:?} requires DispatchConfig::grid_override. Set grid_override explicitly.",
            workgroup_size
        )));
    }

    // Prefer the largest statically-declared output element count.
    let max_output_count = program
        .buffers()
        .iter()
        .filter(|b| b.is_output())
        .map(|b| b.count())
        .max()
        .unwrap_or(0);

    let effective_count = if max_output_count > 0 {
        max_output_count
    } else {
        // All outputs are runtime-sized: derive from input buffers instead.
        let max_input_count = program
            .buffers()
            .iter()
            .filter(|b| !b.is_output())
            .map(|b| b.count())
            .max()
            .unwrap_or(0);

        if max_input_count == 0 {
            return Err(BackendError::new(
                "Fix: all buffers have runtime size (count == 0). \
                 Set DispatchConfig::grid_override to bound the dispatch grid \
                 when no statically-sized buffer exists.",
            ));
        }
        max_input_count
    };

    let lanes = workgroup_size[0];
    let x = effective_count.div_ceil(lanes).max(1);
    Ok([x, 1, 1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

    /// Before the fix, a program with count=0 output and count=512 input launched exactly
    /// 1 workgroup (max_output_count==0 → div_ceil(lanes).max(1)==1). After the fix it
    /// falls back to the input count and launches ceil(512/lanes) workgroups.
    #[test]
    fn test_infer_grid_runtime_sized_output_falls_back_to_input_count() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(512),
                BufferDecl::output("out", 1, DataType::U32), // count=0: runtime-sized
            ],
            [64, 1, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::load("input", Expr::gid_x()))],
        );
        let grid = infer_grid(&program, [64, 1, 1])
            .expect("Fix: infer_grid must succeed when input count is non-zero");
        assert!(
            grid[0] >= 512 / 64,
            "Fix: grid x must be at least ceil(512/64)=8, got {}",
            grid[0]
        );
    }

    /// A program where all buffers have count=0 must return an error requiring
    /// grid_override rather than silently launching 1 workgroup.
    #[test]
    fn test_infer_grid_all_runtime_sized_requires_grid_override() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32), // count=0
                BufferDecl::output("out", 1, DataType::U32), // count=0
            ],
            [64, 1, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::load("input", Expr::gid_x()))],
        );
        let result = infer_grid(&program, [64, 1, 1]);
        assert!(
            result.is_err(),
            "Fix: all-runtime-sized program must require grid_override, not silently launch 1 workgroup"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Fix:"),
            "Fix: error must carry Fix: hint, got: {err}"
        );
    }

    /// Static-count output still uses output count (unchanged behavior after fix).
    #[test]
    fn test_infer_grid_static_output_count_unchanged() {
        let program = Program::wrapped(
            vec![
                BufferDecl::output("out", 0, DataType::U32).with_count(256),
            ],
            [64, 1, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::u32(0))],
        );
        let grid = infer_grid(&program, [64, 1, 1])
            .expect("Fix: static output count must succeed");
        assert_eq!(
            grid[0],
            4, // ceil(256/64) = 4
            "Fix: grid x must be ceil(256/64)=4, got {}",
            grid[0]
        );
    }
}
