//! `CompiledPipeline` implementation for WGPU pipeline dispatch.
//!
//! The parent `pipeline` module owns compilation and metadata assembly. This
//! module owns the trait entrypoints that turn caller inputs into persistent
//! GPU handles, execute the compiled compute pipeline, and read back outputs.

use std::time::{Duration, Instant};

use smallvec::SmallVec;
use vyre_driver::program_walks::enforce_actual_output_budget;
use vyre_driver::{
    resolve_fixpoint_iterations_usize, BackendError, CompiledPipeline, DispatchConfig,
    OutputBuffers, TimedDispatchResult,
};

use crate::engine::record_and_readback::timestamp::{collect_timestamp_profile, TimestampRecorder};
use crate::pipeline::output_slots::resize_vec_with;
use crate::pipeline::WgpuPipeline;
use crate::staging_reserve::{reserve_pipeline_vec, reserve_smallvec, reserve_vec};

impl CompiledPipeline for WgpuPipeline {
    fn dispatch_persistent_handles(
        &self,
        inputs: &[vyre_driver::Resource],
        config: &DispatchConfig,
    ) -> Result<OutputBuffers, BackendError> {
        let mut outputs = Vec::new();
        reserve_vec(
            &mut outputs,
            self.output_bindings.len(),
            "WGPU pipeline",
            "persistent dispatch output buffers",
            "split the dispatch batch before submission",
        )?;
        self.dispatch_persistent_handles_into(inputs, config, &mut outputs)?;
        enforce_actual_output_budget(config, outputs.as_slice())?;
        Ok(outputs)
    }

    fn dispatch_persistent_handles_into(
        &self,
        inputs: &[vyre_driver::Resource],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        self.enforce_static_output_budget(config)?;
        let (device, queue) = &*self.device_queue;
        let workgroup_count = self.workgroups_for_dispatch(config)?;
        let deadline = config
            .timeout
            .and_then(|timeout| Instant::now().checked_add(timeout));
        let resolved = self.resolve_persistent_resources(inputs, queue)?;
        let item = crate::pipeline::persistent::BorrowedDispatchItem {
            inputs: crate::pipeline::persistent::borrowed_handle_refs(&resolved.inputs),
            outputs: crate::pipeline::persistent::borrowed_handle_refs(&resolved.outputs),
            params: None,
            workgroups: workgroup_count,
        };
        self.dispatch_borrowed_persistent_batched(&[item])?;
        self.raise_if_trapped(&resolved.inputs, device, queue, deadline)?;
        self.readback_persistent_outputs(&resolved.outputs, deadline, outputs)?;
        enforce_actual_output_budget(config, outputs.as_slice())
    }

    fn dispatch_persistent_handles_timed(
        &self,
        inputs: &[vyre_driver::Resource],
        config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        self.enforce_static_output_budget(config)?;
        let started = Instant::now();
        let enqueue_started = Instant::now();
        let (device, queue) = &*self.device_queue;
        let deadline = config
            .timeout
            .and_then(|timeout| started.checked_add(timeout));
        let timestamp_deadline =
            deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(30));
        let resolved = self.resolve_persistent_resources(inputs, queue)?;
        let item = crate::pipeline::persistent::BorrowedDispatchItem {
            inputs: crate::pipeline::persistent::borrowed_handle_refs(&resolved.inputs),
            outputs: crate::pipeline::persistent::borrowed_handle_refs(&resolved.outputs),
            params: None,
            workgroups: self.workgroups_for_dispatch(config)?,
        };

        let timestamp_recorder =
            TimestampRecorder::new(device, queue, &self.persistent_pool, true, 0)?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("vyre timed persistent dispatch"),
        });
        let timestamp_writes =
            timestamp_recorder
                .as_ref()
                .map(|recorder| wgpu::ComputePassTimestampWrites {
                    query_set: &recorder.query_set,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                });
        self.record_borrowed_persistent_item_with_timestamps(
            device,
            &mut encoder,
            &item,
            timestamp_writes,
        )?;
        if let Some(recorder) = &timestamp_recorder {
            encoder.write_timestamp(&recorder.query_set, 2);
            encoder.write_timestamp(&recorder.query_set, 3);
            recorder.resolve(&mut encoder)?;
        }
        queue.submit(std::iter::once(encoder.finish()));
        let timestamp_profile = timestamp_recorder
            .map(TimestampRecorder::map_async)
            .transpose()?;
        let enqueue_ns = checked_elapsed_ns(enqueue_started, "WGPU persistent enqueue")?;

        let wait_started = Instant::now();
        self.raise_if_trapped(&resolved.inputs, device, queue, deadline)?;
        let mut outputs = Vec::new();
        self.readback_persistent_outputs(&resolved.outputs, deadline, &mut outputs)?;
        enforce_actual_output_budget(config, outputs.as_slice())?;
        let device_ns = collect_timestamp_profile(timestamp_profile, timestamp_deadline)?
            .map(|profile| profile.dispatch_ns);
        let wait_ns = checked_elapsed_ns(wait_started, "WGPU persistent wait")?;

        Ok(TimedDispatchResult {
            outputs,
            wall_ns: checked_elapsed_ns(started, "WGPU persistent timed dispatch")?,
            device_ns,
            enqueue_ns: Some(enqueue_ns),
            wait_ns: Some(wait_ns),
        })
    }

    fn dispatch_persistent_resource_outputs(
        &self,
        inputs: &[vyre_driver::Resource],
        config: &DispatchConfig,
    ) -> Result<Vec<vyre_driver::Resource>, BackendError> {
        self.enforce_static_output_budget(config)?;
        let (device, queue) = &*self.device_queue;
        let resolved = self.resolve_persistent_resources_for_resource_outputs(inputs, queue)?;
        let item = crate::pipeline::persistent::BorrowedDispatchItem {
            inputs: crate::pipeline::persistent::borrowed_handle_refs(&resolved.inputs),
            outputs: crate::pipeline::persistent::borrowed_handle_refs(&resolved.outputs),
            params: None,
            workgroups: self.workgroups_for_dispatch(config)?,
        };
        self.dispatch_borrowed_persistent_batched(&[item])?;
        let deadline = config
            .timeout
            .and_then(|timeout| Instant::now().checked_add(timeout));
        self.raise_if_trapped(&resolved.inputs, device, queue, deadline)?;
        Ok(resolved.output_resources.into_iter().collect())
    }

    fn dispatch_persistent_handles_batched(
        &self,
        batches: &[&[vyre_driver::Resource]],
        config: &DispatchConfig,
    ) -> Result<Vec<OutputBuffers>, BackendError> {
        let mut outputs = Vec::new();
        reserve_vec(
            &mut outputs,
            batches.len(),
            "WGPU pipeline",
            "persistent batched dispatch output sets",
            "split the dispatch batch before submission",
        )?;
        self.dispatch_persistent_handles_batched_into(batches, config, &mut outputs)?;
        Ok(outputs)
    }

    fn dispatch_persistent_handles_batched_into(
        &self,
        batches: &[&[vyre_driver::Resource]],
        config: &DispatchConfig,
        batch_outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        if batches.is_empty() {
            batch_outputs.clear();
            return Ok(());
        }
        self.enforce_static_output_budget(config)?;
        let (device, queue) = &*self.device_queue;
        let workgroup_count = self.workgroups_for_dispatch(config)?;
        let deadline = config
            .timeout
            .and_then(|timeout| Instant::now().checked_add(timeout));

        let mut resolved = SmallVec::<[_; 8]>::new();
        reserve_smallvec(
            &mut resolved,
            batches.len(),
            "persistent batched dispatch",
            "resolved resource set",
            "split the persistent dispatch batch before submission",
        )?;
        for batch in batches {
            resolved.push(self.resolve_persistent_resources(batch, queue)?);
        }

        let mut items =
            SmallVec::<[crate::pipeline::persistent::BorrowedDispatchItem<'_>; 8]>::new();
        reserve_smallvec(
            &mut items,
            resolved.len(),
            "persistent batched dispatch",
            "command item",
            "split the persistent dispatch batch before submission",
        )?;
        for item in resolved.iter() {
            items.push(crate::pipeline::persistent::BorrowedDispatchItem {
                inputs: crate::pipeline::persistent::borrowed_handle_refs(&item.inputs),
                outputs: crate::pipeline::persistent::borrowed_handle_refs(&item.outputs),
                params: None,
                workgroups: workgroup_count,
            });
        }

        self.dispatch_borrowed_persistent_batched(&items)?;

        resize_vec_with(
            batch_outputs,
            resolved.len(),
            Vec::new,
            "persistent batched dispatch output slots",
        )?;
        for (item, outputs) in resolved.iter().zip(batch_outputs.iter_mut()) {
            self.raise_if_trapped(&item.inputs, device, queue, deadline)?;
            self.readback_persistent_outputs(&item.outputs, deadline, outputs)?;
            enforce_actual_output_budget(config, outputs.as_slice())?;
        }

        Ok(())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn dispatch(
        &self,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let borrowed = vyre_driver::borrowed_input_slices(inputs, "wgpu compiled borrowed input")?;
        self.dispatch_borrowed(&borrowed, config)
    }

    fn dispatch_borrowed(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let mut outputs = Vec::new();
        reserve_pipeline_vec(
            &mut outputs,
            self.output_bindings.len(),
            "borrowed dispatch output buffers",
        )?;
        self.dispatch_borrowed_into(inputs, config, &mut outputs)?;
        Ok(outputs)
    }

    fn dispatch_borrowed_timed(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        self.enforce_static_output_budget(config)?;
        let started = Instant::now();
        let enqueue_started = Instant::now();
        let iterations = resolve_fixpoint_iterations_usize(config, "WGPU")?;
        let iterations = u32::try_from(iterations).map_err(|source| {
            BackendError::new(format!(
                "WGPU compiled borrowed timed dispatch iteration count cannot fit u32: {source}. Fix: split fixpoint replay before command recording."
            ))
        })?;
        let workgroup_count = self.workgroups_for_dispatch(config)?;
        let pending = crate::engine::record_and_readback::record_and_submit_async(
            crate::engine::record_and_readback::RecordAndReadback {
                device_queue: &self.device_queue,
                pool: &self.persistent_pool,
                readback_rings: None,
                pipeline: &self.pipeline,
                bind_group_layouts: &self.bind_group_layouts,
                bind_group_cache: Some(self.bind_group_cache.as_ref()),
                buffer_bindings: &self.buffer_bindings,
                inputs,
                output_bindings: &self.output_bindings,
                trap_tags: &self.trap_tags,
                workgroup_count,
                indirect: self.indirect.as_ref(),
                labels: crate::engine::record_and_readback::DispatchLabels {
                    bind_group: "vyre compiled timed bind group",
                    encoder: "vyre compiled timed dispatch",
                    compute: "vyre compiled timed compute",
                },
                iterations,
                timestamp_profile: true,
            },
        )?;
        let enqueue_ns = checked_elapsed_ns(enqueue_started, "WGPU compiled timed enqueue")?;

        let wait_started = Instant::now();
        let deadline = config
            .timeout
            .and_then(|timeout| started.checked_add(timeout));
        let (outputs, device_ns) = match deadline {
            Some(deadline) => pending.await_timed_result_until(deadline)?,
            None => pending.await_timed_result()?,
        };
        enforce_actual_output_budget(config, outputs.as_slice())?;
        let wait_ns = checked_elapsed_ns(wait_started, "WGPU compiled timed wait")?;

        Ok(TimedDispatchResult {
            outputs,
            wall_ns: checked_elapsed_ns(started, "WGPU compiled timed dispatch")?,
            device_ns,
            enqueue_ns: Some(enqueue_ns),
            wait_ns: Some(wait_ns),
        })
    }

    fn dispatch_borrowed_batched(
        &self,
        batches: &[&[&[u8]]],
        config: &DispatchConfig,
    ) -> Result<Vec<OutputBuffers>, BackendError> {
        let mut outputs = Vec::new();
        reserve_pipeline_vec(
            &mut outputs,
            batches.len(),
            "borrowed batched dispatch output sets",
        )?;
        self.dispatch_borrowed_batched_into(batches, config, &mut outputs)?;
        Ok(outputs)
    }

    fn dispatch_borrowed_batched_into(
        &self,
        batches: &[&[&[u8]]],
        config: &DispatchConfig,
        batch_outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        if batches.is_empty() {
            batch_outputs.clear();
            return Ok(());
        }
        self.enforce_static_output_budget(config)?;
        let deadline = config
            .timeout
            .and_then(|timeout| Instant::now().checked_add(timeout));
        let workgroup_count = self.workgroups_for_dispatch(config)?;

        let mut resolved = SmallVec::<[_; 8]>::new();
        reserve_smallvec(
            &mut resolved,
            batches.len(),
            "borrowed batched dispatch",
            "resolved handle set",
            "split the borrowed dispatch batch before submission",
        )?;
        for inputs in batches {
            resolved.push(self.legacy_handles_from_inputs(inputs)?);
        }

        let mut items =
            SmallVec::<[crate::pipeline::persistent::BorrowedDispatchItem<'_>; 8]>::new();
        reserve_smallvec(
            &mut items,
            resolved.len(),
            "borrowed batched dispatch",
            "command item",
            "split the borrowed dispatch batch before submission",
        )?;
        for (inputs, outputs) in resolved.iter() {
            items.push(crate::pipeline::persistent::BorrowedDispatchItem {
                inputs: crate::pipeline::persistent::borrowed_handle_refs(inputs),
                outputs: crate::pipeline::persistent::borrowed_handle_refs(outputs),
                params: None,
                workgroups: workgroup_count,
            });
        }

        let max_iters = resolve_fixpoint_iterations_usize(config, "WGPU")?;
        for _ in 0..max_iters {
            self.dispatch_borrowed_persistent_batched(&items)?;
        }

        let (device, queue) = &*self.device_queue;
        resize_vec_with(
            batch_outputs,
            resolved.len(),
            Vec::new,
            "borrowed batched dispatch output slots",
        )?;
        for ((inputs, outputs), item_outputs) in resolved.iter().zip(batch_outputs.iter_mut()) {
            self.raise_if_trapped(inputs, device, queue, deadline)?;
            self.readback_persistent_outputs(outputs, deadline, item_outputs)?;
            enforce_actual_output_budget(config, item_outputs.as_slice())?;
        }
        Ok(())
    }

    fn dispatch_borrowed_into(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        self.enforce_static_output_budget(config)?;
        let deadline = config
            .timeout
            .and_then(|timeout| Instant::now().checked_add(timeout));
        let workgroup_count = self.workgroups_for_dispatch(config)?;

        let (input_handles, mut output_handles) = self.legacy_handles_from_inputs(inputs)?;
        let max_iters = resolve_fixpoint_iterations_usize(config, "WGPU")?;
        for _iter in 0..max_iters {
            self.dispatch_persistent(&input_handles, &mut output_handles, None, workgroup_count)?;
        }
        if max_iters > 1 {
            tracing::trace!(
                target: "vyre.dispatch.fixpoint",
                iters = max_iters,
                substrate_path = "persistent_pipeline_fixpoint_loop",
                "persistent pipeline fixpoint loop ran",
            );
        }
        let (device, queue) = &*self.device_queue;
        self.raise_if_trapped(&input_handles, device, queue, deadline)?;
        resize_vec_with(
            outputs,
            output_handles.len(),
            Vec::new,
            "borrowed dispatch output slots",
        )?;
        for ((handle, output), bytes) in output_handles
            .iter()
            .zip(self.output_bindings.iter())
            .zip(outputs.iter_mut())
        {
            crate::pipeline::output_readback::read_trimmed_output(
                handle,
                output,
                device,
                &self.staging_pool,
                queue,
                "persistent pipeline output",
                deadline,
                bytes,
            )?;
        }
        enforce_actual_output_budget(config, outputs.as_slice())?;
        Ok(())
    }
}

fn checked_elapsed_ns(started: Instant, label: &'static str) -> Result<u64, BackendError> {
    u64::try_from(started.elapsed().as_nanos()).map_err(|source| {
        BackendError::new(format!(
            "{label} elapsed time cannot fit u64 nanoseconds: {source}. Fix: split or timeout the dispatch before telemetry overflows."
        ))
    })
}

#[cfg(test)]
mod tests {
    use vyre_driver::{resolve_fixpoint_iterations_usize, DispatchConfig};

    #[test]
    fn generated_fixpoint_iteration_count_uses_driver_policy() {
        let default_config = DispatchConfig::default();
        assert_eq!(
            resolve_fixpoint_iterations_usize(&default_config, "WGPU")
                .expect("Fix: default fixpoint count fits"),
            1
        );

        let mut zero_config = DispatchConfig::default();
        zero_config.fixpoint_iterations = Some(0);
        assert!(
            resolve_fixpoint_iterations_usize(&zero_config, "WGPU").is_err(),
            "Fix: WGPU must use the driver-owned policy and reject explicit zero fixpoint iterations."
        );

        for iterations in 1..4096u32 {
            let mut config = DispatchConfig::default();
            config.fixpoint_iterations = Some(iterations);
            assert_eq!(
                resolve_fixpoint_iterations_usize(&config, "WGPU")
                    .expect("Fix: generated fixpoint count should fit usize"),
                iterations as usize
            );
        }
    }
}
