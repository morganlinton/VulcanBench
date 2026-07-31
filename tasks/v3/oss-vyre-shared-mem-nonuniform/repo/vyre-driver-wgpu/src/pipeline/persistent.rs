//! Persistent-buffer dispatch for compiled wgpu pipelines.

use std::sync::Arc;

use vyre_driver::BackendError;

use super::binding::{
    clear_outputs_for_bound, consumes_host_input, usage_for_binding, validate_handle,
};
use crate::buffer::{BindGroupCacheStats, GpuBufferHandle};
use crate::numeric::usize_to_u64;
use crate::pipeline::{element_size_bytes, BufferBindingInfo, WgpuPipeline};

/// One persistent dispatch record for batched queue submission.
pub struct DispatchItem<'a> {
    /// Input storage/uniform handles in declaration order.
    pub inputs: &'a [GpuBufferHandle],
    /// Output storage handles in declaration order.
    pub outputs: &'a [GpuBufferHandle],
    /// Optional params handle used for the first uniform/push binding.
    pub params: Option<&'a GpuBufferHandle>,
    /// Direct dispatch workgroup counts.
    pub workgroups: [u32; 3],
}

/// Borrowed persistent dispatch record for hot paths that already own
/// resident buffer handles elsewhere.
pub(crate) struct BorrowedDispatchItem<'a> {
    /// Input storage/uniform handles in declaration order.
    pub inputs: smallvec::SmallVec<[&'a GpuBufferHandle; 8]>,
    /// Output storage handles in declaration order.
    pub outputs: smallvec::SmallVec<[&'a GpuBufferHandle; 8]>,
    /// Optional params handle used for the first uniform/push binding.
    pub params: Option<&'a GpuBufferHandle>,
    /// Direct dispatch workgroup counts.
    pub workgroups: [u32; 3],
}

pub(crate) fn borrowed_handle_refs(
    handles: &[GpuBufferHandle],
) -> smallvec::SmallVec<[&GpuBufferHandle; 8]> {
    let mut refs = smallvec::SmallVec::<[&GpuBufferHandle; 8]>::with_capacity(handles.len());
    refs.extend(handles.iter());
    refs
}

pub(crate) fn copied_borrowed_handle_refs<'a>(
    handles: &[&'a GpuBufferHandle],
) -> smallvec::SmallVec<[&'a GpuBufferHandle; 8]> {
    let mut refs = smallvec::SmallVec::<[&'a GpuBufferHandle; 8]>::with_capacity(handles.len());
    refs.extend(handles.iter().copied());
    refs
}

impl WgpuPipeline {
    /// Dispatch using caller-owned GPU-resident buffers.
    ///
    /// This path performs no input, output, or bind-group allocation on cache
    /// hits. The caller owns terminal readback through
    /// [`GpuBufferHandle::readback`].
    ///
    /// # Errors
    ///
    /// Returns a backend error when the supplied handles do not satisfy the
    /// program's binding contract or command recording fails.
    pub fn dispatch_persistent(
        &self,
        inputs: &[GpuBufferHandle],
        outputs: &mut [GpuBufferHandle],
        params: Option<&GpuBufferHandle>,
        workgroups: [u32; 3],
    ) -> Result<(), BackendError> {
        let input_refs = borrowed_handle_refs(inputs);
        let output_refs = borrowed_handle_refs(outputs);
        self.dispatch_persistent_borrowed(
            input_refs.as_slice(),
            output_refs.as_slice(),
            params,
            workgroups,
        )
    }

    /// Dispatch using borrowed GPU-resident buffer handles.
    ///
    /// This is the zero-refcount-churn variant for resident hot paths such as
    /// the batched megakernel dispatcher. It records bindings directly from
    /// caller-owned handles instead of cloning [`GpuBufferHandle`] just to
    /// assemble temporary input/output slices.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the supplied handles do not satisfy the
    /// program's binding contract or command recording fails.
    pub fn dispatch_persistent_borrowed(
        &self,
        inputs: &[&GpuBufferHandle],
        outputs: &[&GpuBufferHandle],
        params: Option<&GpuBufferHandle>,
        workgroups: [u32; 3],
    ) -> Result<(), BackendError> {
        let item = BorrowedDispatchItem {
            inputs: copied_borrowed_handle_refs(inputs),
            outputs: copied_borrowed_handle_refs(outputs),
            params,
            workgroups,
        };
        self.dispatch_borrowed_persistent_batched(&[item])
    }

    /// Dispatch multiple persistent items in one queue submission.
    ///
    /// # Errors
    ///
    /// Returns a backend error when any item violates the binding contract or
    /// command recording fails.
    pub fn dispatch_persistent_batched(
        &self,
        items: &[DispatchItem<'_>],
    ) -> Result<(), BackendError> {
        if items.is_empty() {
            return Ok(());
        }
        let (device, queue) = &*self.device_queue;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("vyre persistent dispatch batch"),
        });
        for item in items {
            let input_refs = borrowed_handle_refs(item.inputs);
            let output_refs = borrowed_handle_refs(item.outputs);
            self.record_borrowed_persistent_item(
                device,
                &mut encoder,
                &BorrowedDispatchItem {
                    inputs: input_refs,
                    outputs: output_refs,
                    params: item.params,
                    workgroups: item.workgroups,
                },
            )?;
        }
        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Dispatch multiple borrowed persistent items in one queue submission.
    ///
    /// # Errors
    ///
    /// Returns a backend error when any item violates the binding contract or
    /// command recording fails.
    pub(crate) fn dispatch_borrowed_persistent_batched(
        &self,
        items: &[BorrowedDispatchItem<'_>],
    ) -> Result<(), BackendError> {
        if items.is_empty() {
            return Ok(());
        }
        let (device, queue) = &*self.device_queue;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("vyre borrowed persistent dispatch batch"),
        });
        for item in items {
            self.record_borrowed_persistent_item(device, &mut encoder, item)?;
        }
        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Return bind-group cache statistics for diagnostics and tests.
    #[must_use]
    pub fn bind_group_cache_stats(&self) -> BindGroupCacheStats {
        self.bind_group_cache.stats()
    }

    pub(crate) fn record_persistent_item(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        item: &DispatchItem<'_>,
    ) -> Result<(), BackendError> {
        let input_refs = borrowed_handle_refs(item.inputs);
        let output_refs = borrowed_handle_refs(item.outputs);
        self.record_borrowed_persistent_item(
            device,
            encoder,
            &BorrowedDispatchItem {
                inputs: input_refs,
                outputs: output_refs,
                params: item.params,
                workgroups: item.workgroups,
            },
        )
    }

    pub(crate) fn record_borrowed_persistent_item(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        item: &BorrowedDispatchItem<'_>,
    ) -> Result<(), BackendError> {
        self.record_borrowed_persistent_item_with_timestamps(device, encoder, item, None)
    }

    pub(crate) fn record_borrowed_persistent_item_with_timestamps(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        item: &BorrowedDispatchItem<'_>,
        timestamp_writes: Option<wgpu::ComputePassTimestampWrites<'_>>,
    ) -> Result<(), BackendError> {
        let bound = self.bound_borrowed_handles(item)?;
        let bind_groups = self.cached_bind_groups(device, &bound)?;
        self.clear_outputs(encoder, &bound)?;
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("vyre persistent compute"),
            timestamp_writes,
        });
        pass.set_pipeline(&self.pipeline);
        for (i, bg) in bind_groups.iter().enumerate() {
            let bind_group_index = u32::try_from(i).map_err(|_| {
                BackendError::new(
                    "persistent pipeline bind group index exceeds u32::MAX. Fix: reduce bind group fanout before WGPU dispatch.",
                )
            })?;
            pass.set_bind_group(bind_group_index, bg.as_ref(), &[]);
        }
        if let Some(indirect) = &self.indirect {
            let indirect_handle = bound
                .iter()
                .find(|(info, _)| info.name.as_ref() == indirect.count_buffer.as_str())
                .map(|(_, handle)| handle)
                .ok_or_else(|| {
                    BackendError::new(format!(
                        "indirect dispatch count buffer `{}` not bound in persistent dispatch. Fix: supply the declared buffer handle.",
                        indirect.count_buffer
                    ))
                })?;
            pass.dispatch_workgroups_indirect(indirect_handle.buffer(), indirect.count_offset);
        } else {
            pass.dispatch_workgroups(item.workgroups[0], item.workgroups[1], item.workgroups[2]);
        }
        Ok(())
    }

    pub(crate) fn legacy_handles_from_inputs(
        &self,
        inputs: &[&[u8]],
    ) -> Result<(Vec<GpuBufferHandle>, Vec<GpuBufferHandle>), BackendError> {
        let (_device, queue) = &*self.device_queue;
        let mut input_handles = Vec::new();
        vyre_driver::allocation::try_reserve_vec_to_capacity(
            &mut input_handles,
            self.buffer_bindings.len(),
        )
        .map_err(|source| {
            BackendError::new(format!(
                "persistent legacy input handle staging could not reserve {} handle slot(s): {source}. Fix: split input buffers before dispatch.",
                self.buffer_bindings.len()
            ))
        })?;
        let mut output_handles = Vec::new();
        vyre_driver::allocation::try_reserve_vec_to_capacity(
            &mut output_handles,
            self.buffer_bindings.len(),
        )
        .map_err(|source| {
            BackendError::new(format!(
                "persistent legacy output handle staging could not reserve {} handle slot(s): {source}. Fix: split output buffers before dispatch.",
                self.buffer_bindings.len()
            ))
        })?;
        // `inputs` is ordered like non-Shared `buffer_bindings`. Avoid building a
        // temporary `input_bindings` vec each call: advance a slot only for used entries.
        let mut input_slot: usize = 0;
        for info in self.buffer_bindings.iter() {
            if info.kind == vyre_foundation::ir::MemoryKind::Shared {
                continue;
            }
            let data = if info.internal_trap {
                None
            } else if !consumes_host_input(info) {
                None
            } else {
                let data = inputs.get(input_slot).copied();
                input_slot += 1;
                data
            };
            if info.is_output {
                let output = self.output_binding(info.binding)?;
                let output_bytes = output.word_count.checked_mul(4).ok_or_else(|| {
                    BackendError::new(format!(
                        "legacy persistent output `{}` size overflows usize. Fix: reduce its element count.",
                        output.name
                    ))
                })?;
                let output_bytes_u64 =
                    usize_to_u64(output_bytes, "persistent output allocation bytes")?;
                let handle = self
                    .persistent_pool
                    .acquire(output_bytes_u64, usage_for_binding(info)?)?;
                if info.preserve_input_contents {
                    if let Some(data) = data {
                        if data.len() > output_bytes {
                            return Err(BackendError::new(format!(
                                "persistent read-write output binding {} (`{}`) received {} host bytes but its output allocation is only {} bytes. Fix: preserve input contents only for read-write outputs whose host input size fits the declared output layout, or mark backend-owned live-outs so they do not consume host input.",
                                info.binding,
                                info.name,
                                data.len(),
                                output_bytes
                            )));
                        }
                        crate::buffer::write_padded(
                            queue,
                            handle.buffer(),
                            data,
                            output_bytes_u64,
                        )?;
                    } else {
                        crate::buffer::write_padded(queue, handle.buffer(), &[], output_bytes_u64)?;
                    }
                }
                output_handles.push(handle);
                continue;
            }
            let data = data.ok_or_else(|| {
                BackendError::new(format!(
                    "persistent input binding {} (`{}`) has no host input bytes. Fix: pass input slices matching non-output BufferDecl order; only internal traps and pure outputs may be backend-allocated empty.",
                    info.binding, info.name
                ))
            })?;
            let padded_size = usize_to_u64(
                binding_padded_size(info, Some(data))?,
                "persistent input bytes",
            )?;
            let handle = self
                .persistent_pool
                .acquire(padded_size, usage_for_binding(info)?)?;
            crate::buffer::write_padded(queue, handle.buffer(), data, padded_size)?;
            input_handles.push(handle);
        }
        Ok((input_handles, output_handles))
    }

    fn clear_outputs(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bound: &[(&BufferBindingInfo, &GpuBufferHandle)],
    ) -> Result<(), BackendError> {
        clear_outputs_for_bound("persistent", encoder, bound, |binding| {
            self.output_binding(binding).cloned()
        })
    }

    fn bound_borrowed_handles<'s, 'h>(
        &'s self,
        item: &BorrowedDispatchItem<'h>,
    ) -> Result<smallvec::SmallVec<[(&'s BufferBindingInfo, &'h GpuBufferHandle); 8]>, BackendError>
    {
        let mut input_index = 0usize;
        let mut output_index = 0usize;
        let mut params_used = false;
        let mut bound: smallvec::SmallVec<[(&'s BufferBindingInfo, &'h GpuBufferHandle); 8]> =
            smallvec::SmallVec::new();
        for info in self.buffer_bindings.iter() {
            if info.kind == vyre_foundation::ir::MemoryKind::Shared {
                continue;
            }
            let handle = if info.is_output {
                let handle = *item.outputs.get(output_index).ok_or_else(|| {
                    BackendError::new(format!(
                        "persistent dispatch missing output handle for binding {} (`{}`). Fix: pass one output handle per output BufferDecl.",
                        info.binding, info.name
                    ))
                })?;
                output_index += 1;
                handle
            } else if matches!(
                info.kind,
                vyre_foundation::ir::MemoryKind::Uniform | vyre_foundation::ir::MemoryKind::Push
            ) && item.params.is_some()
                && !params_used
            {
                params_used = true;
                let Some(params) = item.params else {
                    return Err(BackendError::new(
                        "persistent dispatch parameter handle disappeared after presence check. Fix: keep persistent handle items immutable during binding.",
                    ));
                };
                params
            } else {
                let handle = *item.inputs.get(input_index).ok_or_else(|| {
                    BackendError::new(format!(
                        "persistent dispatch missing input handle for binding {} (`{}`). Fix: pass non-output handles in BufferDecl order.",
                        info.binding, info.name
                    ))
                })?;
                input_index += 1;
                handle
            };
            validate_handle("persistent", info, handle)?;
            bound.push((info, handle));
        }
        validate_consumed_counts(
            item.inputs.len(),
            item.outputs.len(),
            input_index,
            output_index,
        )?;
        Ok(bound)
    }

    fn cached_bind_groups(
        &self,
        device: &wgpu::Device,
        bound: &[(&BufferBindingInfo, &GpuBufferHandle)],
    ) -> Result<Arc<[Arc<wgpu::BindGroup>]>, BackendError> {
        let mut grouped_bound: Vec<
            smallvec::SmallVec<[(&BufferBindingInfo, &GpuBufferHandle); 16]>,
        > = Vec::new();
        vyre_driver::allocation::try_reserve_vec_to_capacity(
            &mut grouped_bound,
            self.bind_group_layouts.len(),
        )
        .map_err(|source| {
            BackendError::new(format!(
                "persistent bind-group staging could not reserve {} group slot(s): {source}. Fix: split bind-group resources before caching.",
                self.bind_group_layouts.len()
            ))
        })?;
        grouped_bound.resize_with(self.bind_group_layouts.len(), smallvec::SmallVec::new);
        for (info, handle) in bound {
            let group = usize::try_from(info.group).map_err(|source| {
                BackendError::new(format!(
                    "persistent bind group {} cannot fit usize: {source}. Fix: keep group indices representable on this host.",
                    info.group
                ))
            })?;
            let Some(slot) = grouped_bound.get_mut(group) else {
                return Err(BackendError::new(format!(
                    "persistent binding {} (`{}`) targets group {}, but the pipeline only has {} bind-group layouts. Fix: keep reflection metadata synchronized with bind-group layouts.",
                    info.binding,
                    info.name,
                    info.group,
                    self.bind_group_layouts.len()
                )));
            };
            slot.push((*info, *handle));
        }
        let mut bind_groups = Vec::new();
        vyre_driver::allocation::try_reserve_vec_to_capacity(
            &mut bind_groups,
            self.bind_group_layouts.len(),
        )
        .map_err(|source| {
            BackendError::new(format!(
                "persistent bind-group cache result could not reserve {} group slot(s): {source}. Fix: split bind-group resources before caching.",
                self.bind_group_layouts.len()
            ))
        })?;
        for (group_index, layout) in self.bind_group_layouts.iter().enumerate() {
            let group_bound = &grouped_bound[group_index];
            let handle_id_capacity = group_bound.len().checked_mul(2).ok_or_else(|| {
                BackendError::new(
                    "persistent bind group handle-id count overflowed usize. Fix: split bind-group resources before caching.",
                )
            })?;
            let mut handle_ids: smallvec::SmallVec<[u64; 16]> = smallvec::SmallVec::new();
            vyre_foundation::allocation::try_reserve_smallvec_to_capacity(
                &mut handle_ids,
                handle_id_capacity,
            )
            .map_err(|source| {
                BackendError::new(format!(
                    "persistent bind-group handle-id cache key could not reserve {handle_id_capacity} word slot(s): {source}. Fix: split bind-group resources before caching."
                ))
            })?;
            let mut checked_bound: smallvec::SmallVec<
                [(&BufferBindingInfo, &GpuBufferHandle, u64); 16],
            > = smallvec::SmallVec::new();
            vyre_foundation::allocation::try_reserve_smallvec_to_capacity(
                &mut checked_bound,
                group_bound.len(),
            )
            .map_err(|source| {
                BackendError::new(format!(
                    "persistent bind-group checked binding staging could not reserve {} binding slot(s): {source}. Fix: split bind-group resources before caching.",
                    group_bound.len()
                ))
            })?;
            for (_, handle) in group_bound {
                handle_ids.push(handle.allocation_identity());
                let bind_size = padded_wgpu_u64(
                    handle.byte_len(),
                    "persistent bind-group cache key byte length",
                )?;
                handle_ids.push(bind_size);
            }
            for (info, handle) in group_bound {
                checked_bound.push((
                    info,
                    handle,
                    padded_wgpu_u64(handle.byte_len(), "persistent bind-group binding size")?,
                ));
            }
            let layout_id = Arc::as_ptr(layout).addr();
            let bg = self
                .bind_group_cache
                .get_or_create_by_ids(layout_id, handle_ids, || {
                    let mut entries =
                        smallvec::SmallVec::<[wgpu::BindGroupEntry<'_>; 16]>::with_capacity(
                            group_bound.len(),
                        );
                    entries.extend(checked_bound.iter().map(|(info, handle, bind_size)| {
                        wgpu::BindGroupEntry {
                            binding: info.binding,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: handle.buffer(),
                                offset: 0,
                                size: wgpu::BufferSize::new(*bind_size),
                            }),
                        }
                    }));
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("vyre persistent bind group"),
                        layout,
                        entries: &entries,
                    })
                });
            bind_groups.push(bg);
        }
        Ok(bind_groups.into())
    }
}

pub(crate) fn binding_padded_size(
    info: &BufferBindingInfo,
    data: Option<&[u8]>,
) -> Result<usize, BackendError> {
    let declared_size = if info.count > 0 {
        usize::try_from(info.count)
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
            })?
    } else {
        0
    };
    if let (declared, Some(bytes)) = (declared_size, data) {
        if declared > 0 && bytes.len() > declared {
            return Err(BackendError::new(format!(
                "buffer `{}` received {} input bytes but declares only {declared} bytes. Fix: either increase BufferDecl::count or pass bytes matching the static buffer contract.",
                info.name,
                bytes.len()
            )));
        }
    }
    let len = match (declared_size, data) {
        (d, Some(_)) if d > 0 => d,
        (d, None) if d > 0 => d,
        (0, Some(bytes)) => bytes.len(),
        (0, None) => 4,
        _ => return Err(BackendError::new(
            "binding_padded_size: unexpected (declared_size, data) combination. Fix: ensure buffer has either a declared count or input data.",
        )),
    };
    let len = padded_wgpu_usize(len, "persistent binding padded size")?;
    Ok(len)
}

fn padded_wgpu_u64(size: u64, label: &'static str) -> Result<u64, BackendError> {
    crate::numeric::align_up_u64(size, 4, label)
}

fn padded_wgpu_usize(size: usize, label: &'static str) -> Result<usize, BackendError> {
    crate::numeric::align_up_usize(size, 4, label)
}

fn validate_consumed_counts(
    input_len: usize,
    output_len: usize,
    input_index: usize,
    output_index: usize,
) -> Result<(), BackendError> {
    if input_index != input_len {
        return Err(BackendError::new(format!(
            "persistent dispatch received {} input handles but consumed {input_index}. Fix: pass handles matching non-output BufferDecl order.",
            input_len
        )));
    }
    if output_index != output_len {
        return Err(BackendError::new(format!(
            "persistent dispatch received {} output handles but consumed {output_index}. Fix: pass handles matching output BufferDecl order.",
            output_len
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding_info(count: u32) -> BufferBindingInfo {
        BufferBindingInfo {
            group: 0,
            binding: 0,
            name: Arc::from("input"),
            access: vyre_foundation::ir::BufferAccess::ReadOnly,
            kind: vyre_foundation::ir::MemoryKind::Readonly,
            hints: vyre_foundation::ir::MemoryHints::default(),
            element: vyre_foundation::ir::DataType::U32,
            count,
            is_output: false,
            preserve_input_contents: false,
            internal_trap: false,
        }
    }

    #[test]
    fn binding_padded_size_rejects_oversized_static_input() {
        let info = binding_info(4);
        let error = binding_padded_size(&info, Some(&[0u8; 20]))
            .expect_err("static buffer input larger than BufferDecl::count must fail");
        assert!(
            error
                .to_string()
                .contains("received 20 input bytes but declares only 16 bytes"),
            "{error}"
        );
    }

    #[test]
    fn binding_padded_size_accepts_runtime_sized_input() {
        let info = binding_info(0);
        let size = binding_padded_size(&info, Some(&[7u8; 20]))
            .expect("Fix: runtime input sizes; restore this invariant before continuing.");
        assert_eq!(size, 20);
    }
}
