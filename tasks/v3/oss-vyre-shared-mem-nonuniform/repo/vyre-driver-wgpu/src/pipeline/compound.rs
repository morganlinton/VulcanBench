//! Compound command-buffer dispatch for pipeline mode (Innovation I.14).

use super::binding::usage_for_binding;
use crate::allocation::{reserve_smallvec_to_capacity, reserve_vec_to_capacity};
use crate::buffer::{GpuBufferHandle, StagingBufferPool};
use crate::numeric::usize_to_u64;
use crate::pipeline::{DispatchItem, OutputLayout, WgpuPipeline};
use smallvec::SmallVec;
use std::sync::mpsc::Receiver;
use vyre_driver::{BackendError, DispatchConfig, Resource};

#[derive(Clone, Copy)]
pub(crate) enum CompoundResource<'a> {
    Borrowed(&'a [u8]),
    Resident(u64),
}

impl<'a> From<&'a Resource> for CompoundResource<'a> {
    fn from(resource: &'a Resource) -> Self {
        match resource {
            Resource::Borrowed(bytes) => Self::Borrowed(bytes),
            Resource::Resident(id) => Self::Resident(*id),
        }
    }
}

impl WgpuPipeline {
    /// Batch several inputs for this same compiled program into one GPU
    /// submission.
    pub fn dispatch_coalesced(
        &self,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<Vec<u8>>>, BackendError> {
        let mut borrowed = SmallVec::<[&[u8]; 8]>::new();
        reserve_smallvec_to_capacity(
            &mut borrowed,
            inputs.len(),
            "same-pipeline compound dispatch",
            "borrowed input descriptor",
            "split the coalesced input batch before submission",
        )?;
        borrowed.extend(inputs.iter().map(Vec::as_slice));
        self.dispatch_coalesced_borrowed(&borrowed, config)
    }

    /// Batch several borrowed inputs for this same compiled program into one
    /// GPU submission.
    pub fn dispatch_coalesced_borrowed(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<Vec<u8>>>, BackendError> {
        self.dispatch_same_pipeline_borrowed(inputs, config)
    }

    /// Optimized substrate-neutral compound dispatch (V7-PERF-021).
    pub fn dispatch_compound_v2(
        requests: &[(&WgpuPipeline, Resource)],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<Vec<u8>>>, BackendError> {
        let mut borrowed_requests = SmallVec::<[(&WgpuPipeline, CompoundResource<'_>); 8]>::new();
        reserve_smallvec_to_capacity(
            &mut borrowed_requests,
            requests.len(),
            "compound dispatch",
            "borrowed request descriptor",
            "split the compound dispatch batch before submission",
        )?;
        borrowed_requests.extend(
            requests
                .iter()
                .map(|(pipeline, resource)| (*pipeline, CompoundResource::from(resource))),
        );
        Self::dispatch_compound_borrowed(&borrowed_requests, config)
    }

    pub(crate) fn dispatch_compound_borrowed(
        requests: &[(&WgpuPipeline, CompoundResource<'_>)],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<Vec<u8>>>, BackendError> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let (device, queue) = &*requests[0].0.device_queue;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("vyre compound dispatch v2"),
        });
        let mut live = SmallVec::<[PipelineDispatchReadback; 8]>::new();
        reserve_smallvec_to_capacity(
            &mut live,
            requests.len(),
            "compound dispatch",
            "live readback descriptor",
            "split the compound dispatch batch before submission",
        )?;
        for (pipeline, resource) in requests {
            if pipeline.device_queue.0 != *device {
                return Err(BackendError::new(
                    "cross-device compound dispatch is unsupported",
                ));
            }
            live.push(pipeline.record_compound_dispatch_v2(
                device,
                &mut encoder,
                resource,
                config,
            )?);
        }
        submit_and_collect_compound_readbacks(device, queue, encoder, live, config)
    }

    fn dispatch_same_pipeline_borrowed(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<Vec<u8>>>, BackendError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let (device, queue) = &*self.device_queue;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("vyre coalesced same-pipeline dispatch"),
        });
        let mut live = SmallVec::<[PipelineDispatchReadback; 8]>::new();
        reserve_smallvec_to_capacity(
            &mut live,
            inputs.len(),
            "same-pipeline compound dispatch",
            "live readback descriptor",
            "split the coalesced input batch before submission",
        )?;
        for input in inputs {
            live.push(self.record_compound_dispatch_v2(
                device,
                &mut encoder,
                &CompoundResource::Borrowed(input),
                config,
            )?);
        }
        submit_and_collect_compound_readbacks(device, queue, encoder, live, config)
    }

    fn record_compound_dispatch_v2(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        resource: &CompoundResource<'_>,
        config: &DispatchConfig,
    ) -> Result<PipelineDispatchReadback, BackendError> {
        let workgroups = self.workgroups_for_dispatch(config)?;

        let (input_handles, output_handles) = match resource {
            CompoundResource::Borrowed(bytes) => self.legacy_handles_from_inputs(&[bytes])?,
            CompoundResource::Resident(id) => self.handles_from_resident_resource(*id)?,
        };

        self.record_persistent_item(
            device,
            encoder,
            &DispatchItem {
                inputs: &input_handles,
                outputs: &output_handles,
                params: None,
                workgroups,
            },
        )?;

        let readback_size = usize_to_u64(self.output.copy_size, "compound readback bytes")?;
        let readback_usage = wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ;
        let readback_buffer = self
            .staging_pool
            .acquire(device, readback_size, readback_usage);

        let output = output_handles
            .first()
            .ok_or_else(|| BackendError::new("no output"))?;
        encoder.copy_buffer_to_buffer(
            output.buffer(),
            usize_to_u64(self.output.copy_offset, "compound output copy offset")?,
            &readback_buffer,
            0,
            readback_size,
        );

        Ok(PipelineDispatchReadback {
            readback_buffer,
            readback_size,
            readback_usage,
            output: self.output,
            staging_pool: self.staging_pool.clone(),
            _input_handles: input_handles,
            _output_handles: output_handles,
        })
    }

    fn handles_from_resident_resource(
        &self,
        id: u64,
    ) -> Result<(Vec<GpuBufferHandle>, Vec<GpuBufferHandle>), BackendError> {
        let input_count = self
            .buffer_bindings
            .iter()
            .filter(|info| info.kind != vyre_foundation::ir::MemoryKind::Shared && !info.is_output)
            .count();
        if input_count != 1 {
            return Err(BackendError::new(format!(
                "Resident Resource can bind exactly one non-output buffer, but this pipeline declares {input_count}. Fix: call dispatch_persistent with the full input handle list for multi-input resident dispatch."
            )));
        }
        let input = GpuBufferHandle::from_resident_id(id).ok_or_else(|| {
            BackendError::new(format!(
                "Resident Resource id {id} is not live in the wgpu resident registry. Fix: keep the GpuBufferHandle alive until compound dispatch completes."
            ))
        })?;
        let mut outputs = Vec::new();
        reserve_vec_to_capacity(
            &mut outputs,
            self.output_bindings.len(),
            "compound resident dispatch",
            "output handle",
            "split resident outputs before compound dispatch",
        )?;
        for info in self.buffer_bindings.iter() {
            if info.kind == vyre_foundation::ir::MemoryKind::Shared || !info.is_output {
                continue;
            }
            let output = self.output_binding(info.binding)?;
            let output_bytes = output.word_count.checked_mul(4).ok_or_else(|| {
                BackendError::new(format!(
                    "compound resident output `{}` size overflows usize. Fix: reduce its element count.",
                    output.name
                ))
            })?;
            outputs.push(self.persistent_pool.acquire(
                usize_to_u64(output_bytes, "compound output allocation bytes")?,
                usage_for_binding(info)?,
            )?);
        }
        Ok((vec![input], outputs))
    }
}

type CompoundPendingMap = (
    PipelineDispatchReadback,
    Receiver<Result<(), wgpu::BufferAsyncError>>,
);

fn submit_and_collect_compound_readbacks(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: wgpu::CommandEncoder,
    live: SmallVec<[PipelineDispatchReadback; 8]>,
    config: &DispatchConfig,
) -> Result<Vec<Vec<Vec<u8>>>, BackendError> {
    let submission = queue.submit(std::iter::once(encoder.finish()));
    let mut pending_maps = SmallVec::<[CompoundPendingMap; 8]>::new();
    reserve_smallvec_to_capacity(
        &mut pending_maps,
        live.len(),
        "compound dispatch",
        "pending readback map",
        "split the compound dispatch batch before submission",
    )?;
    for readback in live {
        pending_maps.push(readback.request_map());
    }
    crate::runtime::device::poll_device_wait_for(device, submission)?;
    let mut outputs = Vec::new();
    reserve_vec_to_capacity(
        &mut outputs,
        pending_maps.len(),
        "compound dispatch",
        "output set",
        "split the compound dispatch batch before readback collection",
    )?;
    for (resources, receiver) in pending_maps {
        outputs.push(resources.read_mapped(receiver)?);
    }
    enforce_compound_output_budget(config, &outputs)?;
    Ok(outputs)
}

fn enforce_compound_output_budget(
    config: &DispatchConfig,
    outputs: &[Vec<Vec<u8>>],
) -> Result<(), BackendError> {
    let Some(limit) = config.max_output_bytes else {
        return Ok(());
    };
    let actual = outputs.iter().try_fold(0usize, |sum, dispatch_outputs| {
        dispatch_outputs.iter().try_fold(sum, |inner_sum, output| {
            inner_sum.checked_add(output.len()).ok_or_else(|| {
                BackendError::new(
                    "compound readback size overflows usize. Fix: split the Program output before dispatch.",
                )
            })
        })
    })?;
    if actual > limit {
        return Err(BackendError::new(format!(
            "compound readback size {actual} exceeds DispatchConfig.max_output_bytes {limit}. Fix: narrow BufferDecl::output_byte_range or raise max_output_bytes."
        )));
    }
    Ok(())
}

struct PipelineDispatchReadback {
    readback_buffer: wgpu::Buffer,
    readback_size: u64,
    readback_usage: wgpu::BufferUsages,
    output: OutputLayout,
    staging_pool: StagingBufferPool,
    _input_handles: Vec<GpuBufferHandle>,
    _output_handles: Vec<GpuBufferHandle>,
}

impl PipelineDispatchReadback {
    fn request_map(self) -> (Self, Receiver<Result<(), wgpu::BufferAsyncError>>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        {
            let slice = self.readback_buffer.slice(..);
            slice.map_async(wgpu::MapMode::Read, move |res| {
                if let Err(error) = sender.send(res) {
                    tracing::error!(
                        ?error,
                        "compound pipeline readback map_async result was lost because the receiver dropped"
                    );
                }
            });
        }
        (self, receiver)
    }

    fn read_mapped(
        self,
        receiver: Receiver<Result<(), wgpu::BufferAsyncError>>,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let map_result = match receiver.recv() {
            Ok(result) => result,
            Err(_) => {
                return Err(BackendError::new(
                    "compound readback callback channel closed. Fix: ensure the wgpu device is polled until the submitted work completes.",
                ));
            }
        };
        if let Err(e) = map_result {
            return Err(BackendError::new(format!(
                "compound readback mapping failed: {e:?}. Fix: use MAP_READ and COPY_DST readback buffers."
            )));
        }

        let read_result = {
            let slice = self.readback_buffer.slice(..);
            let mapped = slice.get_mapped_range();
            let trim_start = self.output.trim_start;
            let read_size = self.output.read_size;
            let end = trim_start.checked_add(read_size).ok_or_else(|| {
                BackendError::new(format!(
                    "compound readback slice end overflows usize: trim_start={trim_start}, read_size={read_size}. Fix: verify OutputLayout before readback."
                ))
            })?;
            if end > mapped.len() {
                let mapped_len = mapped.len();
                Err(BackendError::new(format!(
                    "compound readback slice is out of bounds: trim_start={}, read_size={}, mapped_len={}. Fix: verify OutputLayout against the actual GPU readback buffer size.",
                    trim_start, read_size, mapped_len
                )))
            } else {
                Ok(mapped[trim_start..end].to_vec())
            }
        };
        self.release_readback_buffer();
        let res = read_result?;
        Ok(vec![res])
    }

    fn release_readback_buffer(self) {
        self.readback_buffer.unmap();
        self.staging_pool.release(
            self.readback_buffer,
            self.readback_size,
            self.readback_usage,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::enforce_compound_output_budget;
    use vyre_driver::DispatchConfig;

    fn generated_compound_outputs(case: usize) -> Vec<Vec<Vec<u8>>> {
        let dispatch_count = (case % 7) + 1;
        let mut outputs = Vec::new();
        outputs
            .try_reserve(dispatch_count)
            .expect("Fix: generated compound budget test must reserve dispatch outputs");
        for dispatch_idx in 0..dispatch_count {
            let output_count = ((case / (dispatch_idx + 1)) % 5) + 1;
            let mut dispatch_outputs = Vec::new();
            dispatch_outputs
                .try_reserve(output_count)
                .expect("Fix: generated compound budget test must reserve per-dispatch outputs");
            for output_idx in 0..output_count {
                let len = ((case * 31 + dispatch_idx * 17 + output_idx * 13) % 257) + 1;
                dispatch_outputs.push(vec![0u8; len]);
            }
            outputs.push(dispatch_outputs);
        }
        outputs
    }

    fn compound_output_bytes(outputs: &[Vec<Vec<u8>>]) -> usize {
        outputs
            .iter()
            .flat_map(|dispatch_outputs| dispatch_outputs.iter())
            .map(Vec::len)
            .sum()
    }

    #[test]
    fn generated_compound_output_budget_accepts_exact_total_and_rejects_one_byte_less() {
        for case in 0..4096 {
            let outputs = generated_compound_outputs(case);
            let exact_total = compound_output_bytes(&outputs);

            let mut exact_config = DispatchConfig::default();
            exact_config.max_output_bytes = Some(exact_total);
            enforce_compound_output_budget(&exact_config, &outputs)
                .expect("Fix: exact compound readback budget must be accepted");

            if exact_total == 0 {
                continue;
            }
            let mut too_small_config = DispatchConfig::default();
            too_small_config.max_output_bytes = Some(exact_total - 1);
            let error = enforce_compound_output_budget(&too_small_config, &outputs)
                .expect_err("Fix: compound readback budget one byte below total must reject");
            assert!(
                error.to_string().contains("compound readback size"),
                "compound budget rejection must identify readback size, got {error}"
            );
        }
    }
}
