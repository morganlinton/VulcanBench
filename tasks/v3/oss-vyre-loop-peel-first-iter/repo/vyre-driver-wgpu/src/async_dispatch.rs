use std::sync::Arc;
use std::time::{Duration, Instant};
use vyre_driver::validation::LaunchGeometryLimits;
use vyre_foundation::ir::Program;

use crate::{PredictedProgram, WgpuBackend};

enum WgpuPendingKind {
    Ready(Vec<Vec<u8>>),
    Readback(crate::engine::record_and_readback::WgpuPendingReadback),
}

pub(crate) struct WgpuPendingDispatch {
    kind: WgpuPendingKind,
    started: Instant,
    timeout: Option<Duration>,
    prefetch: Option<PipelinePrefetch>,
    launch_feedback: Option<WgpuLaunchFeedback>,
}

struct WgpuLaunchFeedback {
    program: Arc<Program>,
    config: vyre_driver::DispatchConfig,
    limits: LaunchGeometryLimits,
    element_count: u32,
    workgroup: [u32; 3],
}

impl vyre_driver::backend::private::Sealed for WgpuPendingDispatch {}

impl WgpuPendingDispatch {
    pub(crate) fn await_owned(
        self,
    ) -> Result<vyre_driver::OutputBuffers, vyre_driver::BackendError> {
        let Self {
            kind,
            started,
            timeout,
            prefetch,
            launch_feedback: _,
        } = self;
        run_prefetch(prefetch);
        let outputs = match kind {
            WgpuPendingKind::Ready(outputs) => outputs,
            WgpuPendingKind::Readback(pending) => match dispatch_deadline(started, timeout) {
                Some(deadline) => pending.await_result_until(deadline)?,
                None => pending.await_result()?,
            },
        };
        if let Some(deadline) = timeout {
            let elapsed = started.elapsed();
            if elapsed > deadline {
                return Err(vyre_driver::BackendError::new(format!(
                    "dispatch exceeded configured timeout: took {elapsed:?}, budget {deadline:?}. \
                     Fix: raise DispatchConfig.timeout or split the program into smaller chunks."
                )));
            }
        }
        Ok(outputs)
    }

    pub(crate) fn await_into(
        self,
        outputs: &mut vyre_driver::OutputBuffers,
    ) -> Result<(), vyre_driver::BackendError> {
        let Self {
            kind,
            started,
            timeout,
            prefetch,
            launch_feedback: _,
        } = self;
        run_prefetch(prefetch);
        match kind {
            WgpuPendingKind::Ready(ready) => {
                vyre_driver::backend::replace_output_buffers_preserving_slots(ready, outputs);
                Ok(())
            }
            WgpuPendingKind::Readback(pending) => match dispatch_deadline(started, timeout) {
                Some(deadline) => pending.await_into_until(outputs, deadline),
                None => pending.await_into(outputs),
            },
        }?;
        if let Some(deadline) = timeout {
            let elapsed = started.elapsed();
            if elapsed > deadline {
                return Err(vyre_driver::BackendError::new(format!(
                    "dispatch exceeded configured timeout: took {elapsed:?}, budget {deadline:?}. \
                     Fix: raise DispatchConfig.timeout or split the program into smaller chunks."
                )));
            }
        }
        Ok(())
    }

    pub(crate) fn await_mapped_outputs<F>(
        self,
        mut visitor: F,
    ) -> Result<(), vyre_driver::BackendError>
    where
        F: FnMut(usize, &[u8]) -> Result<(), vyre_driver::BackendError>,
    {
        let Self {
            kind,
            started,
            timeout,
            prefetch,
            launch_feedback: _,
        } = self;
        run_prefetch(prefetch);
        match kind {
            WgpuPendingKind::Ready(ready) => {
                for (index, output) in ready.iter().enumerate() {
                    visitor(index, output)?;
                }
                Ok(())
            }
            WgpuPendingKind::Readback(pending) => match dispatch_deadline(started, timeout) {
                Some(deadline) => pending.await_mapped_outputs_until(visitor, deadline),
                None => pending.await_mapped_outputs(visitor),
            },
        }?;
        Self::enforce_timeout(started, timeout)
    }

    fn is_ready_inner(&self) -> bool {
        match &self.kind {
            WgpuPendingKind::Ready(_) => true,
            WgpuPendingKind::Readback(pending) => pending.is_ready(),
        }
    }

    fn enforce_timeout(
        started: Instant,
        timeout: Option<Duration>,
    ) -> Result<(), vyre_driver::BackendError> {
        if let Some(deadline) = timeout {
            let elapsed = started.elapsed();
            if elapsed > deadline {
                return Err(vyre_driver::BackendError::new(format!(
                    "dispatch exceeded configured timeout: took {elapsed:?}, budget {deadline:?}. \
                     Fix: raise DispatchConfig.timeout or split the program into smaller chunks."
                )));
            }
        }
        Ok(())
    }

    pub(crate) fn await_timed_owned(
        self,
    ) -> Result<vyre_driver::TimedDispatchResult, vyre_driver::BackendError> {
        let Self {
            kind,
            started,
            timeout,
            prefetch,
            launch_feedback,
        } = self;
        run_prefetch(prefetch);
        let (outputs, device_ns) = match kind {
            WgpuPendingKind::Ready(outputs) => (outputs, None),
            WgpuPendingKind::Readback(pending) => match dispatch_deadline(started, timeout) {
                Some(deadline) => pending.await_timed_result_until(deadline)?,
                None => pending.await_timed_result()?,
            },
        };
        if let (Some(feedback), Some(measured_device_ns)) = (launch_feedback, device_ns) {
            let _accepted = vyre_driver::launch::record_launch_measurement(
                &feedback.program,
                &feedback.config,
                feedback.limits,
                feedback.element_count,
                feedback.workgroup,
                measured_device_ns,
            );
        }
        Self::enforce_timeout(started, timeout)?;
        Ok(vyre_driver::TimedDispatchResult {
            outputs,
            wall_ns: elapsed_nanos_u64(started, "wgpu timed dispatch")?,
            device_ns,
            enqueue_ns: None,
            wait_ns: None,
        })
    }
}

fn elapsed_nanos_u64(start: Instant, label: &str) -> Result<u64, vyre_driver::BackendError> {
    u64::try_from(start.elapsed().as_nanos()).map_err(|source| {
        vyre_driver::BackendError::new(format!(
            "{label} elapsed time cannot fit u64 nanoseconds: {source}. Fix: split or timeout the dispatch before telemetry overflows."
        ))
    })
}

fn dispatch_deadline(started: Instant, timeout: Option<Duration>) -> Option<Instant> {
    timeout.and_then(|duration| started.checked_add(duration))
}

fn reject_unserviceable_timeout(
    timeout: Option<Duration>,
) -> Result<(), vyre_driver::BackendError> {
    if matches!(timeout, Some(timeout) if timeout <= Duration::from_millis(100)) {
        return Err(vyre_driver::BackendError::new(
            "dispatch cancelled before WGPU pipeline compilation because DispatchConfig.timeout is below the backend's serviceable queue/readback window. Fix: raise DispatchConfig.timeout or use an already compiled persistent pipeline.",
        ));
    }
    Ok(())
}

impl vyre_driver::PendingDispatch for WgpuPendingDispatch {
    fn is_ready(&self) -> bool {
        self.is_ready_inner()
    }

    fn await_result(self: Box<Self>) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        (*self).await_owned()
    }
}

impl WgpuBackend {
    /// GPU staging consumes these slices directly; backing memory must stay valid until the
    /// pending dispatch completes. The `VyreBackend::dispatch_async` implementation forwards here
    /// after collecting `Vec::as_slice` views into a `SmallVec` - no clone of input payloads.
    pub(crate) fn dispatch_borrowed_async(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &vyre_driver::DispatchConfig,
    ) -> Result<WgpuPendingDispatch, vyre_driver::BackendError> {
        let started = Instant::now();
        self.enforce_config_caps(config)?;
        self.validate_with_cache(program)?;
        if vyre_driver::grid_sync::contains_grid_sync(program)
            && !vyre_driver::VyreBackend::supports_grid_sync(self)
        {
            return Ok(WgpuPendingDispatch {
                kind: WgpuPendingKind::Ready(
                    vyre_driver::grid_sync::dispatch_with_grid_sync_split(
                        self, program, inputs, config,
                    )?,
                ),
                started,
                timeout: config.timeout,
                prefetch: None,
                launch_feedback: None,
            });
        }
        self.dispatch_borrowed_async_validated(program, inputs, config, started, false)
    }

    pub(crate) fn dispatch_borrowed_async_timed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &vyre_driver::DispatchConfig,
    ) -> Result<WgpuPendingDispatch, vyre_driver::BackendError> {
        let started = Instant::now();
        self.enforce_config_caps(config)?;
        self.validate_with_cache(program)?;
        if vyre_driver::grid_sync::contains_grid_sync(program)
            && !vyre_driver::VyreBackend::supports_grid_sync(self)
        {
            return Ok(WgpuPendingDispatch {
                kind: WgpuPendingKind::Ready(
                    vyre_driver::grid_sync::dispatch_with_grid_sync_split(
                        self, program, inputs, config,
                    )?,
                ),
                started,
                timeout: config.timeout,
                prefetch: None,
                launch_feedback: None,
            });
        }
        self.dispatch_borrowed_async_validated(program, inputs, config, started, true)
    }

    fn dispatch_borrowed_async_validated(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &vyre_driver::DispatchConfig,
        started: Instant,
        capture_timing: bool,
    ) -> Result<WgpuPendingDispatch, vyre_driver::BackendError> {
        if program.is_explicit_noop() {
            return Ok(WgpuPendingDispatch {
                kind: WgpuPendingKind::Ready(Vec::new()),
                started,
                timeout: config.timeout,
                prefetch: None,
                launch_feedback: None,
            });
        }
        reject_unserviceable_timeout(config.timeout)?;

        let dispatch_arena = self.dispatch_arena_snapshot();
        let pipeline = crate::pipeline::WgpuPipeline::compile_with_device_queue(
            program,
            config,
            self.adapter_info.clone(),
            self.enabled_features,
            self.current_device_queue(),
            Arc::clone(&dispatch_arena),
            self.current_persistent_pool(),
            self.pipeline_cache.clone(),
            self.bind_group_layout_cache.clone(),
        )?;

        if let Some(deadline) = config.timeout {
            let elapsed = started.elapsed();
            if elapsed > deadline {
                return Err(vyre_driver::BackendError::new(format!(
                    "dispatch cancelled after DispatchConfig.timeout before GPU submission: took {elapsed:?}, budget {deadline:?}. \
                     Fix: raise DispatchConfig.timeout or split the program into smaller chunks."
                )));
            }
        }

        let workgroup_count = pipeline.workgroups_for_dispatch(config)?;
        let launch_feedback = if capture_timing {
            Some(wgpu_launch_feedback(program, config, &pipeline)?)
        } else {
            None
        };
        let pending = crate::engine::record_and_readback::record_and_submit_async(
            crate::engine::record_and_readback::RecordAndReadback::for_dispatch(
                &pipeline,
                &dispatch_arena,
                inputs,
                workgroup_count,
                config,
                capture_timing || timestamp_profile_requested(config),
                crate::engine::record_and_readback::DispatchLabels {
                    bind_group: "vyre dispatch_async bind group",
                    encoder: "vyre dispatch_async",
                    compute: "vyre dispatch_async compute",
                },
            ),
        )?;
        Ok(WgpuPendingDispatch {
            kind: WgpuPendingKind::Readback(pending),
            started,
            timeout: config.timeout,
            prefetch: self.next_shape_prefetch(program, config)?,
            launch_feedback,
        })
    }
}

fn wgpu_launch_feedback(
    program: &Program,
    config: &vyre_driver::DispatchConfig,
    pipeline: &crate::pipeline::WgpuPipeline,
) -> Result<WgpuLaunchFeedback, vyre_driver::BackendError> {
    let limits = wgpu_launch_limits(&pipeline.device_queue.0);
    let element_count = u32::try_from(pipeline.output_word_count).map_err(|source| {
        vyre_driver::BackendError::new(format!(
            "WGPU launch feedback output word count {} cannot fit u32: {source}. Fix: split the dispatch before timed natural-gradient measurement.",
            pipeline.output_word_count
        ))
    })?;
    Ok(WgpuLaunchFeedback {
        program: Arc::new(program.clone()),
        config: config.clone(),
        limits,
        element_count,
        workgroup: pipeline.workgroup_shape,
    })
}

fn wgpu_launch_limits(device: &wgpu::Device) -> LaunchGeometryLimits {
    let limits = device.limits();
    LaunchGeometryLimits {
        backend: "wgpu",
        max_threads_per_block: limits.max_compute_invocations_per_workgroup,
        max_block_dim: [
            limits.max_compute_workgroup_size_x,
            limits.max_compute_workgroup_size_y,
            limits.max_compute_workgroup_size_z,
        ],
        max_grid_dim: [limits.max_compute_workgroups_per_dimension; 3],
    }
}

struct PipelinePrefetch {
    backend: WgpuBackend,
    program: Arc<Program>,
    config: vyre_driver::DispatchConfig,
}

impl PipelinePrefetch {
    fn run(self) {
        if let Err(error) = crate::pipeline::WgpuPipeline::compile_with_device_queue(
            &self.program,
            &self.config,
            self.backend.adapter_info.clone(),
            self.backend.enabled_features,
            self.backend.current_device_queue(),
            self.backend.dispatch_arena_snapshot(),
            self.backend.current_persistent_pool(),
            self.backend.pipeline_cache.clone(),
            self.backend.bind_group_layout_cache.clone(),
        ) {
            tracing::debug!(
                target: "vyre.wgpu.pipeline.prefetch",
                error = %error,
                "predicted pipeline prefetch failed"
            );
        }
    }
}

fn run_prefetch(prefetch: Option<PipelinePrefetch>) {
    if let Some(prefetch) = prefetch {
        prefetch.run();
    }
}

impl WgpuBackend {
    fn next_shape_prefetch(
        &self,
        program: &Program,
        config: &vyre_driver::DispatchConfig,
    ) -> Result<Option<PipelinePrefetch>, vyre_driver::BackendError> {
        let fingerprint = vyre_driver::program_vsa_fingerprint_words(program);
        self.predicted_programs
            .entry(fingerprint)
            .and_modify(|cached| cached.config = config.clone())
            .or_insert_with(|| PredictedProgram {
                program: Arc::new(program.clone()),
                config: config.clone(),
            });

        let predicted = {
            let mut history = self.shape_history.lock().map_err(|_| {
                vyre_driver::BackendError::new(
                    "wgpu shape-prediction history lock was poisoned. Fix: abort the current backend instance and reacquire the GPU backend.",
                )
            })?;
            history.record(fingerprint);
            self.predicted_programs
                .retain(|candidate, _| history.contains(candidate));
            history.predict_next()
        };

        let Some(predicted) = predicted else {
            return Ok(None);
        };
        Ok(self
            .predicted_programs
            .get(&predicted)
            .map(|cached| PipelinePrefetch {
                backend: self.clone(),
                program: Arc::clone(&cached.program),
                config: cached.config.clone(),
            }))
    }
}

pub(crate) fn timestamp_profile_requested(config: &vyre_driver::DispatchConfig) -> bool {
    matches!(
        config.profile.as_deref(),
        Some("gpu-timestamps" | "wgpu-timestamps" | "timestamps")
    ) || std::env::var_os("VYRE_WGPU_TIMESTAMPS").is_some()
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn ready_pending_await_into_preserves_output_slots() {
        let mut outputs = vec![Vec::with_capacity(16), Vec::with_capacity(16)];
        outputs[0].extend_from_slice(&[1]);
        outputs[1].extend_from_slice(&[2]);
        let outer = outputs.as_ptr();
        let first_slot = outputs[0].as_ptr();
        let second_slot = outputs[1].as_ptr();

        let pending = WgpuPendingDispatch {
            kind: WgpuPendingKind::Ready(vec![vec![9, 9, 9], vec![8, 8]]),
            started: Instant::now(),
            timeout: None,
            prefetch: None,
            launch_feedback: None,
        };

        pending
            .await_into(&mut outputs)
            .expect("Fix: ready pending dispatch should move bytes into caller outputs");

        assert_eq!(outputs, vec![vec![9, 9, 9], vec![8, 8]]);
        assert_eq!(
            outputs.as_ptr(),
            outer,
            "Fix: ready pending dispatch must preserve caller-owned output vector storage."
        );
        assert_eq!(
            outputs[0].as_ptr(),
            first_slot,
            "Fix: ready pending dispatch must reuse output slot 0 allocation."
        );
        assert_eq!(
            outputs[1].as_ptr(),
            second_slot,
            "Fix: ready pending dispatch must reuse output slot 1 allocation."
        );
    }
}
