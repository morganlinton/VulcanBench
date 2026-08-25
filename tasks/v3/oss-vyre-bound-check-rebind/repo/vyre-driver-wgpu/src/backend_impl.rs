//! vyre_driver::VyreBackend implementation and core WgpuBackend methods.

use crate::staging_reserve::{reserve_backend_vec, reserve_smallvec, reserve_vec};
use crate::{AdapterRecoveryTarget, DispatchArena, WgpuBackend};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use vyre_driver::persistent::PersistentThreadMode;
use vyre_driver::speculate::SpeculationMode;
use vyre_foundation::ir::Program;

fn empty_batch_result_slots<T>(
    len: usize,
) -> Result<Vec<Option<Result<T, vyre_driver::BackendError>>>, vyre_driver::BackendError> {
    let mut slots = Vec::new();
    reserve_vec(
        &mut slots,
        len,
        "WGPU backend",
        "batch result slot",
        "split the batch before dispatch",
    )?;
    slots.resize_with(len, || None);
    Ok(slots)
}

fn finalize_batch_results<T>(
    slots: Vec<Option<Result<T, vyre_driver::BackendError>>>,
    missing_slot_message: &'static str,
) -> Result<Vec<Result<T, vyre_driver::BackendError>>, vyre_driver::BackendError> {
    let mut results = Vec::new();
    reserve_vec(
        &mut results,
        slots.len(),
        "WGPU backend",
        "final batch result",
        "split the batch before dispatch",
    )?;
    for slot in slots {
        results.push(
            slot.unwrap_or_else(|| Err(vyre_driver::BackendError::new(missing_slot_message))),
        );
    }
    Ok(results)
}

fn elapsed_micros_u64(start: Instant, label: &str) -> Result<u64, vyre_driver::BackendError> {
    u64::try_from(start.elapsed().as_micros()).map_err(|source| {
        vyre_driver::BackendError::new(format!(
            "{label} elapsed time cannot fit u64 microseconds: {source}. Fix: split or timeout the dispatch before telemetry overflows."
        ))
    })
}

impl WgpuBackend {
    /// Adapter information selected for this backend instance.
    #[must_use]
    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        &self.adapter_info
    }

    /// Device limits for this backend instance.
    #[must_use]
    pub fn device_limits(&self) -> &wgpu::Limits {
        &self.device_limits
    }

    /// Acquire the backend, probing adapters and returning a structured error
    /// when no compatible GPU is found.
    pub fn acquire() -> Result<Self, vyre_driver::BackendError> {
        let ((device, queue), adapter_info, enabled_features) = crate::runtime::init_device()
            .map_err(|error| {
                let report = crate::runtime::device::adapter_probe_report();
                vyre_driver::BackendError::new(format!(
                    "no compatible GPU adapter found. Probed adapters: [{}].                  Missing features / limits: [{}]. Underlying error: {error}.                  Fix: install a compatible GPU driver and ensure a wgpu-supported backend                  (Vulkan, Metal, DX12) is available.",
                    report.probed.join(", "),
                    if report.missing.is_empty() {
                        "none".to_string()
                    } else {
                        report.missing.join(", ")
                    }
                ))
            })?;
        let recovery_target = AdapterRecoveryTarget::Identity(
            crate::runtime::device::AdapterIdentity::from_info(&adapter_info),
        );
        Self::from_device_queue(
            device,
            queue,
            adapter_info,
            enabled_features,
            recovery_target,
        )
    }

    /// Acquire a backend bound to a specific enumerable adapter index.
    pub fn acquire_adapter(index: usize) -> Result<Self, vyre_driver::BackendError> {
        let ((device, queue), adapter_info, enabled_features) =
            crate::runtime::device::init_device_for_adapter(index)
                .map_err(|error| vyre_driver::BackendError::new(error.to_string()))?;
        Self::from_device_queue(
            device,
            queue,
            adapter_info,
            enabled_features,
            AdapterRecoveryTarget::Index(index),
        )
    }

    fn from_device_queue(
        device: wgpu::Device,
        queue: wgpu::Queue,
        adapter_info: wgpu::AdapterInfo,
        enabled_features: crate::runtime::device::EnabledFeatures,
        recovery_target: AdapterRecoveryTarget,
    ) -> Result<Self, vyre_driver::BackendError> {
        let device_limits = device.limits();
        let adapter_name = Arc::<str>::from(adapter_info.name.as_str());
        let cache_tiers = vec![
            crate::runtime::cache::CacheTier::try_new("hot", 1 << 24)?,
            crate::runtime::cache::CacheTier::try_new("cold", 1 << 30)?,
        ];
        let persistent_pool = crate::buffer::BufferPool::with_tiering(
            device.clone(),
            queue.clone(),
            &vyre_driver::DispatchConfig::default(),
            cache_tiers,
        )?;
        let (pipeline_cache_entries, pipeline_cache_bytes) =
            vyre_driver::pipeline::pipeline_cache_limits_from_env();
        Ok(Self {
            adapter_name,
            adapter_info,
            device_limits,
            device_queue: Arc::new(arc_swap::ArcSwap::new(Arc::new((
                device.clone(),
                queue.clone(),
            )))),
            dispatch_arena: Arc::new(arc_swap::ArcSwap::from_pointee(DispatchArena::new(
                device.clone(),
                queue.clone(),
                &vyre_driver::DispatchConfig::default(),
            ))),
            persistent_pool: Arc::new(arc_swap::ArcSwap::new(Arc::new(persistent_pool))),
            pipeline_cache: Arc::new(
                crate::runtime::cache::pipeline::LruPipelineCache::with_limits(
                    pipeline_cache_entries,
                    pipeline_cache_bytes,
                ),
            ),
            wgsl_dispatch_pipeline_cache: Arc::new(dashmap::DashMap::with_hasher(
                BuildHasherDefault::<rustc_hash::FxHasher>::default(),
            )),
            resident_pipeline_cache: Arc::new(dashmap::DashMap::with_hasher(BuildHasherDefault::<
                rustc_hash::FxHasher,
            >::default(
            ))),
            validation_cache: Arc::new(vyre_driver::validation::ValidationCache::default()),
            shape_history: Arc::new(std::sync::Mutex::new(
                vyre_driver::shape_prediction::ShapeHistory::new(),
            )),
            predicted_programs: Arc::new(dashmap::DashMap::with_hasher(BuildHasherDefault::<
                rustc_hash::FxHasher,
            >::default())),
            bind_group_layout_cache: Arc::new(dashmap::DashMap::with_hasher(BuildHasherDefault::<
                rustc_hash::FxHasher,
            >::default(
            ))),
            resident_handles: Arc::new(dashmap::DashMap::with_hasher(BuildHasherDefault::<
                rustc_hash::FxHasher,
            >::default())),
            device_lost: Arc::new(AtomicBool::new(false)),
            enabled_features,
            recovery_target,
        })
    }

    pub(crate) fn current_device_queue(&self) -> Arc<(wgpu::Device, wgpu::Queue)> {
        self.device_queue.load_full()
    }

    /// Consumer-visible snapshot of the live wgpu device + queue.
    #[must_use]
    pub fn device_queue(&self) -> Arc<(wgpu::Device, wgpu::Queue)> {
        self.current_device_queue()
    }

    pub(crate) fn current_persistent_pool(&self) -> crate::buffer::BufferPool {
        self.persistent_pool.load_full().as_ref().clone()
    }

    fn resident_pipeline_cache_key(
        &self,
        program: &Program,
        config: &vyre_driver::DispatchConfig,
    ) -> Result<(u64, u64, usize), vyre_driver::BackendError> {
        let wire = program.to_wire().map_err(|source| {
            vyre_driver::BackendError::new(format!(
                "WGPU resident pipeline cache could not encode Program: {source}. Fix: validate the Program before resident dispatch."
            ))
        })?;
        let mut program_hasher = rustc_hash::FxHasher::default();
        program_hasher.write(&wire);
        let mut config_hasher = rustc_hash::FxHasher::default();
        config_hasher.write(format!("{config:?}").as_bytes());
        Ok((program_hasher.finish(), config_hasher.finish(), wire.len()))
    }

    pub(crate) fn compile_resident_pipeline_cached(
        &self,
        program: &Program,
        config: &vyre_driver::DispatchConfig,
    ) -> Result<Arc<crate::pipeline::WgpuPipeline>, vyre_driver::BackendError> {
        let key = self.resident_pipeline_cache_key(program, config)?;
        if let Some(hit) = self.resident_pipeline_cache.get(&key) {
            return Ok(hit.clone());
        }
        self.enforce_config_caps(config)?;
        self.validate_with_cache(program)?;
        let compiled = crate::pipeline::WgpuPipeline::compile_with_device_queue(
            program,
            config,
            self.adapter_info.clone(),
            self.enabled_features,
            self.current_device_queue(),
            self.dispatch_arena_snapshot(),
            self.current_persistent_pool(),
            self.pipeline_cache.clone(),
            self.bind_group_layout_cache.clone(),
        )?;
        match self.resident_pipeline_cache.entry(key) {
            dashmap::mapref::entry::Entry::Occupied(entry) => Ok(entry.get().clone()),
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(compiled.clone());
                Ok(compiled)
            }
        }
    }

    pub(crate) fn dispatch_arena_snapshot(&self) -> Arc<DispatchArena> {
        self.dispatch_arena.load_full()
    }

    pub(crate) fn validate_with_cache(
        &self,
        program: &Program,
    ) -> Result<(), vyre_driver::BackendError> {
        self.validation_cache.get_or_validate_backend(program, self)
    }

    /// Test-only hook that marks the backend device as lost and invalidates
    /// caches tied to the current device generation.
    pub fn force_device_lost(&self) -> Result<(), vyre_driver::BackendError> {
        self.device_lost.store(true, Ordering::Release);
        self.pipeline_cache.clear();
        self.wgsl_dispatch_pipeline_cache.clear();
        self.bind_group_layout_cache.clear();
        self.validation_cache.clear()?;
        let device_queue = self.device_queue.load_full();
        self.dispatch_arena.store(Arc::new(DispatchArena::new(
            device_queue.0.clone(),
            device_queue.1.clone(),
            &vyre_driver::DispatchConfig::default(),
        )));
        Ok(())
    }

    /// Invalidate compiled pipeline artifacts selected by a rule-impact mask.
    pub fn invalidate_impacted_pipeline_cache(
        &self,
        intervention_mask: &[u32],
        rule_adj: &[u32],
        state: &[u32],
        join_rules: &[u32],
        n: u32,
        max_iterations: u32,
        pipeline_lineage_cell: &[u32],
        pipeline_keys: &[[u8; 32]],
    ) -> Result<(), vyre_driver::BackendError> {
        let final_impact_mask = vyre_driver::cache_invalidation::impacted_entries(
            self,
            intervention_mask,
            rule_adj,
            state,
            join_rules,
            n,
            max_iterations,
            pipeline_lineage_cell,
        )
        .map_err(|error| vyre_driver::BackendError::new(error.to_string()))?;
        self.pipeline_cache
            .invalidate_impacted(&final_impact_mask, pipeline_keys);
        Ok(())
    }

    /// Convenience wrapper around [`Self::invalidate_impacted_pipeline_cache`]
    pub fn invalidate_pipeline_cache_for_changed_op(
        &self,
        changed_op_handle: u32,
        pipeline_lineage_cell: &[u32],
        pipeline_keys: &[[u8; 32]],
    ) -> Result<(), vyre_driver::BackendError> {
        let n = 1u32;
        let rule_adj = vec![1u32];
        let intervention_mask = vec![1u32];
        let state = vec![1u32];
        let join_rules = vec![1u32];
        let max_iterations = 1u32;
        let mut normalized_lineage_cell = Vec::with_capacity(pipeline_lineage_cell.len());
        normalized_lineage_cell.extend(pipeline_lineage_cell.iter().map(|&op| {
            if op == changed_op_handle {
                0
            } else {
                u32::MAX
            }
        }));
        self.invalidate_impacted_pipeline_cache(
            &intervention_mask,
            &rule_adj,
            &state,
            &join_rules,
            n,
            max_iterations,
            &normalized_lineage_cell,
            pipeline_keys,
        )
    }

    /// Invalidate disk-cached pipeline artifacts selected by a rule-impact mask.
    pub fn invalidate_impacted_disk_cache(
        &self,
        intervention_mask: &[u32],
        rule_adj: &[u32],
        state: &[u32],
        join_rules: &[u32],
        n: u32,
        max_iterations: u32,
        pipeline_lineage_cell: &[u32],
        cache_keys: &[String],
    ) -> Result<(), vyre_driver::BackendError> {
        crate::pipeline::disk_cache::invalidate_impacted(
            self,
            intervention_mask,
            rule_adj,
            state,
            join_rules,
            n,
            max_iterations,
            pipeline_lineage_cell,
            cache_keys,
        )
        .map_err(|e| vyre_driver::BackendError::new(e.to_string()))
    }

    /// Create the backend if a GPU adapter is available.
    #[must_use]
    #[inline]
    pub fn new() -> Result<Self, vyre_driver::BackendError> {
        Self::acquire().map_err(|e| vyre_driver::BackendError::new(e.to_string()))
    }

    /// Process-wide shared backend handle.
    pub fn shared() -> Result<Arc<Self>, vyre_driver::BackendError> {
        static SHARED: std::sync::OnceLock<Result<Arc<WgpuBackend>, String>> =
            std::sync::OnceLock::new();
        match SHARED.get_or_init(|| Self::new().map(Arc::new).map_err(|e| e.to_string())) {
            Ok(arc) => Ok(arc.clone()),
            Err(msg) => Err(vyre_driver::BackendError::new(msg.clone())),
        }
    }

    /// Dispatch borrowed inputs and visit each mapped output byte slice.
    pub fn dispatch_borrowed_for_each_mapped_output<F>(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &vyre_driver::DispatchConfig,
        visitor: F,
    ) -> Result<(), vyre_driver::BackendError>
    where
        F: FnMut(usize, &[u8]) -> Result<(), vyre_driver::BackendError>,
    {
        let _span = tracing::trace_span!(
            "vyre.dispatch_mapped_outputs",
            backend = "wgpu",
            inputs = inputs.len(),
            label = tracing::field::Empty,
        );
        let _enter = _span.enter();
        if let Some(label) = config.label.as_deref() {
            _span.record("label", label);
        }
        let start = Instant::now();
        self.dispatch_borrowed_async(program, inputs, config)?
            .await_mapped_outputs(visitor)?;
        tracing::trace!(
            target: "vyre.dispatch",
            elapsed_us = elapsed_micros_u64(start, "mapped-output dispatch")?,
            inputs = inputs.len(),
            "mapped-output dispatch completed"
        );
        Ok(())
    }

    /// Dispatch borrowed inputs and visit each mapped output as a typed POD slice.
    pub fn dispatch_borrowed_for_each_pod_output<T, F>(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &vyre_driver::DispatchConfig,
        mut visitor: F,
    ) -> Result<(), vyre_driver::BackendError>
    where
        T: bytemuck::Pod,
        F: FnMut(usize, &[T]) -> Result<(), vyre_driver::BackendError>,
    {
        self.dispatch_borrowed_for_each_mapped_output(program, inputs, config, |index, bytes| {
            let typed = bytemuck::try_cast_slice::<u8, T>(bytes).map_err(|error| {
                vyre_driver::BackendError::new(format!(
                    "mapped output #{index} cannot be viewed as {}: {error}. Fix: set output_byte_range to a length and offset aligned for the requested POD type.",
                    std::any::type_name::<T>()
                ))
            })?;
            visitor(index, typed)
        })
    }

    /// Enforce capability requirements declared in `config`.
    pub(crate) fn enforce_config_caps(
        &self,
        config: &vyre_driver::DispatchConfig,
    ) -> Result<(), vyre_driver::BackendError> {
        if matches!(config.speculation, Some(SpeculationMode::Force))
            && !<Self as vyre_driver::VyreBackend>::supports_speculation(self)
        {
            return Err(vyre_driver::BackendError::UnsupportedFeature {
                name: "speculative dispatch".to_string(),
                backend: <Self as vyre_driver::VyreBackend>::id(self).to_string(),
            });
        }
        if matches!(config.persistent_thread, Some(PersistentThreadMode::Force))
            && !<Self as vyre_driver::VyreBackend>::supports_persistent_thread_dispatch(self)
        {
            return Err(vyre_driver::BackendError::UnsupportedFeature {
                name: "persistent-thread dispatch".to_string(),
                backend: <Self as vyre_driver::VyreBackend>::id(self).to_string(),
            });
        }
        Ok(())
    }

    /// Dispatch a real prefilter/confirm scan through the adaptive speculative path.
    pub fn dispatch_speculative_prefilter_confirm<F>(
        &self,
        speculator: &vyre_driver::speculate::AdaptiveSpeculator,
        plan: vyre_driver::speculate::SpeculativeDispatchPlan<'_>,
        inputs: &[&[u8]],
        config: &vyre_driver::DispatchConfig,
        confirm_serial: F,
    ) -> Result<vyre_driver::speculate::SpeculativeDispatchOutcome, vyre_driver::BackendError>
    where
        F: FnMut(
            vyre_driver::OutputBuffers,
        ) -> Result<vyre_driver::OutputBuffers, vyre_driver::BackendError>,
    {
        vyre_driver::speculate::dispatch_prefilter_confirm(
            self,
            speculator,
            plan,
            inputs,
            config,
            confirm_serial,
        )
    }

    fn record_borrowed_batch_job(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &vyre_driver::DispatchConfig,
        started: Instant,
    ) -> Result<crate::engine::record_and_readback::RecordedDispatch, vyre_driver::BackendError>
    {
        self.enforce_config_caps(config)?;
        self.validate_with_cache(program)?;
        let pipeline = crate::pipeline::WgpuPipeline::compile_with_device_queue(
            program,
            config,
            self.adapter_info.clone(),
            self.enabled_features,
            self.current_device_queue(),
            self.dispatch_arena_snapshot(),
            self.current_persistent_pool(),
            self.pipeline_cache.clone(),
            self.bind_group_layout_cache.clone(),
        )?;
        if let Some(deadline) = config.timeout {
            let elapsed = started.elapsed();
            if elapsed > deadline {
                return Err(vyre_driver::BackendError::new(format!(
                    "batch dispatch cancelled before GPU submission: took {elapsed:?}, budget {deadline:?}.                      Fix: raise DispatchConfig.timeout or split the program into smaller chunks."
                )));
            }
        }
        let workgroup_count = pipeline.workgroups_for_dispatch(config)?;
        let dispatch_arena = self.dispatch_arena_snapshot();
        crate::engine::record_and_readback::record_dispatch_unsubmitted(
            crate::engine::record_and_readback::RecordAndReadback::for_dispatch(
                &pipeline,
                &dispatch_arena,
                inputs,
                workgroup_count,
                config,
                crate::async_dispatch::timestamp_profile_requested(config),
                crate::engine::record_and_readback::DispatchLabels {
                    bind_group: "vyre batch dispatch bind group",
                    encoder: "vyre batch dispatch",
                    compute: "vyre batch dispatch compute",
                },
            ),
        )
    }

    /// Dispatch a batch of borrowed `(Program, inputs, config)` triples.
    pub fn dispatch_borrowed_batch(
        &self,
        jobs: &[(&Program, &[&[u8]], &vyre_driver::DispatchConfig)],
    ) -> Result<
        Vec<Result<vyre_driver::OutputBuffers, vyre_driver::BackendError>>,
        vyre_driver::BackendError,
    > {
        let _span = tracing::trace_span!(
            "vyre.dispatch_borrowed_batch",
            backend = "wgpu",
            jobs = jobs.len(),
        );
        let _enter = _span.enter();

        let mut results = empty_batch_result_slots(jobs.len())?;
        let mut recorded = Vec::new();
        reserve_backend_vec(&mut recorded, jobs.len(), "recorded dispatch")?;
        let mut meta = Vec::new();
        reserve_backend_vec(&mut meta, jobs.len(), "batch dispatch metadata")?;
        for (index, (program, inputs, config)) in jobs.iter().enumerate() {
            let started = Instant::now();
            if program.is_explicit_noop() {
                results[index] = Some(Ok(Vec::new()));
                continue;
            }
            let command = self.record_borrowed_batch_job(program, inputs, config, started)?;
            recorded.push(command);
            meta.push((index, started, config.timeout));
        }

        let pending = crate::engine::record_and_readback::submit_recorded_batch(recorded)?;
        for ((index, started, timeout), result) in meta
            .into_iter()
            .zip(crate::engine::record_and_readback::WgpuPendingReadback::await_many_owned(pending))
        {
            results[index] = Some(result.and_then(|outputs| {
                if let Some(deadline) = timeout {
                    let elapsed = started.elapsed();
                    if elapsed > deadline {
                        return Err(vyre_driver::BackendError::new(format!(
                            "batch dispatch exceeded configured timeout: took {elapsed:?}, budget {deadline:?}.                              Fix: raise DispatchConfig.timeout or split the program into smaller chunks."
                        )));
                    }
                }
                Ok(outputs)
            }));
        }
        finalize_batch_results(
            results,
            "internal batch dispatch result slot was not filled. Fix: keep batch recording metadata synchronized.",
        )
    }

    /// Dispatch a borrowed batch and write each job's outputs into caller-owned per-job output buffers.
    pub fn dispatch_borrowed_batch_into(
        &self,
        jobs: &[(&Program, &[&[u8]], &vyre_driver::DispatchConfig)],
        outputs: &mut [vyre_driver::OutputBuffers],
    ) -> Result<Vec<Result<(), vyre_driver::BackendError>>, vyre_driver::BackendError> {
        if outputs.len() != jobs.len() {
            return Err(vyre_driver::BackendError::new(format!(
                "dispatch_borrowed_batch_into received {} output slots for {} jobs. Fix: pass exactly one OutputBuffers slot per job.",
                outputs.len(),
                jobs.len()
            )));
        }

        let _span = tracing::trace_span!(
            "vyre.dispatch_borrowed_batch_into",
            backend = "wgpu",
            jobs = jobs.len(),
        );
        let _enter = _span.enter();

        let mut results = empty_batch_result_slots(jobs.len())?;
        let mut recorded = Vec::new();
        reserve_backend_vec(&mut recorded, jobs.len(), "recorded dispatch")?;
        let mut meta = Vec::new();
        reserve_backend_vec(&mut meta, jobs.len(), "batch-into dispatch metadata")?;
        for (index, (program, inputs, config)) in jobs.iter().enumerate() {
            let started = Instant::now();
            if program.is_explicit_noop() {
                outputs[index].clear();
                results[index] = Some(Ok(()));
                continue;
            }
            let command = self.record_borrowed_batch_job(program, inputs, config, started)?;
            recorded.push(command);
            meta.push((index, started, config.timeout));
        }

        let pending = crate::engine::record_and_readback::submit_recorded_batch(recorded)?;
        let deadline =
            crate::engine::record_and_readback::WgpuPendingReadback::wait_for_many(&pending);
        for ((index, started, timeout), readback) in meta.into_iter().zip(pending) {
            results[index] = Some(
                readback
                    .collect_after_submission_wait(&mut outputs[index], deadline)
                    .and_then(|()| {
                        if let Some(deadline) = timeout {
                            let elapsed = started.elapsed();
                            if elapsed > deadline {
                                return Err(vyre_driver::BackendError::new(format!(
                                    "batch dispatch exceeded configured timeout: took {elapsed:?}, budget {deadline:?}.                                      Fix: raise DispatchConfig.timeout or split the program into smaller chunks."
                                )));
                            }
                        }
                        Ok(())
                    }),
            );
        }
        finalize_batch_results(
            results,
            "internal batch-into dispatch result slot was not filled. Fix: keep batch recording metadata synchronized.",
        )
    }

    /// Dispatch an owned batch of `(Program, inputs, config)` triples.
    pub fn dispatch_batch(
        &self,
        jobs: &[(
            vyre_foundation::ir::Program,
            Vec<Vec<u8>>,
            vyre_driver::DispatchConfig,
        )],
    ) -> Result<
        Vec<Result<vyre_driver::OutputBuffers, vyre_driver::BackendError>>,
        vyre_driver::BackendError,
    > {
        let mut borrowed_inputs = Vec::new();
        reserve_backend_vec(&mut borrowed_inputs, jobs.len(), "borrowed input batch")?;
        for (_, inputs, _) in jobs {
            let mut borrowed = smallvec::SmallVec::<[&[u8]; 8]>::new();
            reserve_smallvec(
                &mut borrowed,
                inputs.len(),
                "WGPU backend",
                "borrowed input slice reference",
                "split the batch job before dispatch",
            )?;
            borrowed.extend(inputs.iter().map(Vec::as_slice));
            borrowed_inputs.push(borrowed);
        }
        let mut borrowed_jobs = Vec::new();
        reserve_backend_vec(&mut borrowed_jobs, jobs.len(), "borrowed dispatch job")?;
        for ((program, _, config), inputs) in jobs.iter().zip(borrowed_inputs.iter()) {
            borrowed_jobs.push((program, inputs.as_slice(), config));
        }
        self.dispatch_borrowed_batch(&borrowed_jobs)
    }

    /// Compile a program into a host-ingress wgpu stream.
    #[allow(deprecated)]
    pub fn compile_streaming(
        &self,
        program: &vyre_foundation::ir::Program,
        config: vyre_driver::DispatchConfig,
    ) -> Result<crate::engine::streaming::HostIngressStream, vyre_driver::BackendError> {
        self.enforce_config_caps(&config)?;
        let pipeline = crate::pipeline::WgpuPipeline::compile_with_device_queue(
            program,
            &config,
            self.adapter_info.clone(),
            self.enabled_features,
            self.current_device_queue(),
            self.dispatch_arena_snapshot(),
            self.current_persistent_pool(),
            self.pipeline_cache.clone(),
            self.bind_group_layout_cache.clone(),
        )?;
        Ok(crate::engine::streaming::HostIngressStream::new(
            (*pipeline).clone(),
            config,
        ))
    }

    /// Compile a program into a persistent pipeline.
    pub fn compile_persistent(
        &self,
        program: &vyre_foundation::ir::Program,
        config: &vyre_driver::DispatchConfig,
    ) -> Result<Arc<crate::pipeline::WgpuPipeline>, vyre_driver::BackendError> {
        self.enforce_config_caps(config)?;
        crate::pipeline::WgpuPipeline::compile_with_device_queue(
            program,
            config,
            self.adapter_info.clone(),
            self.enabled_features,
            self.current_device_queue(),
            self.dispatch_arena_snapshot(),
            self.current_persistent_pool(),
            self.pipeline_cache.clone(),
            self.bind_group_layout_cache.clone(),
        )
    }
}

/// Converts caller-owned input buffers into a [`smallvec::SmallVec`] of borrowed slices.
///
/// The wgpu backend's `dispatch_async` routes through this helper so staging reads from the
/// caller's [`Vec`] allocations without cloning payload bytes - only slice references are
/// collected into the vector. With more than eight inputs the [`SmallVec`] spills to heap
/// storage while elements still alias the original buffers.
#[allow(clippy::needless_lifetimes)]
pub(crate) fn borrowed_slices_from_owned_inputs<'a>(
    inputs: &'a [Vec<u8>],
) -> smallvec::SmallVec<[&'a [u8]; 8]> {
    let mut borrowed = smallvec::SmallVec::<[&'a [u8]; 8]>::with_capacity(inputs.len());
    borrowed.extend(inputs.iter().map(Vec::as_slice));
    borrowed
}

impl vyre_driver::VyreBackend for WgpuBackend {
    fn id(&self) -> &'static str {
        "wgpu"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_ops(&self) -> &std::collections::HashSet<vyre_foundation::ir::OpId> {
        vyre_driver::backend::validation::default_supported_ops_with_trap()
    }

    fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        config: &vyre_driver::DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        let _span = tracing::trace_span!(
            "vyre.dispatch",
            backend = "wgpu",
            inputs = inputs.len(),
            label = tracing::field::Empty,
        );
        let _enter = _span.enter();
        if let Some(label) = config.label.as_deref() {
            _span.record("label", label);
        }
        let borrowed = borrowed_slices_from_owned_inputs(inputs);
        let start = Instant::now();
        let result = self
            .dispatch_borrowed_async(program, &borrowed, config)?
            .await_owned();
        tracing::trace!(
            target: "vyre.dispatch",
            elapsed_us = elapsed_micros_u64(start, "borrowed-path dispatch")?,
            inputs = inputs.len(),
            "dispatch completed (borrowed-path; clone-free)"
        );
        result
    }

    fn dispatch_borrowed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &vyre_driver::DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        let _span = tracing::trace_span!(
            "vyre.dispatch",
            backend = "wgpu",
            inputs = inputs.len(),
            label = tracing::field::Empty,
        );
        let _enter = _span.enter();
        if let Some(label) = config.label.as_deref() {
            _span.record("label", label);
        }
        let start = Instant::now();
        let result = self
            .dispatch_borrowed_async(program, inputs, config)?
            .await_owned();
        tracing::trace!(
            target: "vyre.dispatch",
            elapsed_us = elapsed_micros_u64(start, "dispatch")?,
            inputs = inputs.len(),
            "dispatch completed"
        );
        result
    }

    fn dispatch_borrowed_into(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &vyre_driver::DispatchConfig,
        outputs: &mut vyre_driver::OutputBuffers,
    ) -> Result<(), vyre_driver::BackendError> {
        let _span = tracing::trace_span!(
            "vyre.dispatch_into",
            backend = "wgpu",
            inputs = inputs.len(),
            label = tracing::field::Empty,
        );
        let _enter = _span.enter();
        if let Some(label) = config.label.as_deref() {
            _span.record("label", label);
        }
        if vyre_driver::grid_sync::contains_grid_sync(program)
            && !<Self as vyre_driver::VyreBackend>::supports_grid_sync(self)
        {
            return vyre_driver::grid_sync::dispatch_with_grid_sync_split_into(
                self, program, inputs, config, outputs,
            );
        }
        let start = Instant::now();
        self.dispatch_borrowed_async(program, inputs, config)?
            .await_into(outputs)?;
        tracing::trace!(
            target: "vyre.dispatch",
            elapsed_us = elapsed_micros_u64(start, "dispatch into caller-owned outputs")?,
            inputs = inputs.len(),
            "dispatch completed into caller-owned outputs"
        );
        Ok(())
    }

    fn dispatch_borrowed_timed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &vyre_driver::DispatchConfig,
    ) -> Result<vyre_driver::TimedDispatchResult, vyre_driver::BackendError> {
        let _span = tracing::trace_span!(
            "vyre.dispatch_timed",
            backend = "wgpu",
            inputs = inputs.len(),
            label = tracing::field::Empty,
        );
        let _enter = _span.enter();
        if let Some(label) = config.label.as_deref() {
            _span.record("label", label);
        }
        WgpuBackend::dispatch_borrowed_async_timed(self, program, inputs, config)?
            .await_timed_owned()
    }

    fn dispatch_async(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        config: &vyre_driver::DispatchConfig,
    ) -> Result<Box<dyn vyre_driver::backend::PendingDispatch>, vyre_driver::BackendError> {
        let _span = tracing::trace_span!(
            "vyre.dispatch_async",
            backend = "wgpu",
            inputs = inputs.len(),
            label = tracing::field::Empty,
        );
        let _enter = _span.enter();
        if let Some(label) = config.label.as_deref() {
            _span.record("label", label);
        }

        let borrowed = borrowed_slices_from_owned_inputs(inputs);
        Ok(Box::new(WgpuBackend::dispatch_borrowed_async(
            self, program, &borrowed, config,
        )?))
    }

    fn dispatch_borrowed_async(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &vyre_driver::DispatchConfig,
    ) -> Result<Box<dyn vyre_driver::backend::PendingDispatch>, vyre_driver::BackendError> {
        Ok(Box::new(WgpuBackend::dispatch_borrowed_async(
            self, program, inputs, config,
        )?))
    }

    fn compile_native(
        &self,
        program: &Program,
        config: &vyre_driver::DispatchConfig,
    ) -> Result<Option<std::sync::Arc<dyn vyre_driver::CompiledPipeline>>, vyre_driver::BackendError>
    {
        self.enforce_config_caps(config)?;
        self.validate_with_cache(program)?;
        let cached = crate::pipeline::WgpuPipeline::compile_with_device_queue(
            program,
            config,
            self.adapter_info.clone(),
            self.enabled_features,
            self.current_device_queue(),
            self.dispatch_arena_snapshot(),
            self.current_persistent_pool(),
            self.pipeline_cache.clone(),
            self.bind_group_layout_cache.clone(),
        )?;
        Ok(Some(cached))
    }

    fn allocate_device_buffer(
        &self,
        byte_len: usize,
    ) -> Result<Box<dyn vyre_driver::DeviceBuffer>, vyre_driver::BackendError> {
        self.allocate_wgpu_device_buffer(byte_len)
    }

    fn upload_device_buffer(
        &self,
        buffer: &mut dyn vyre_driver::DeviceBuffer,
        bytes: &[u8],
    ) -> Result<(), vyre_driver::BackendError> {
        self.upload_wgpu_device_buffer(buffer, bytes)
    }

    fn download_device_buffer(
        &self,
        buffer: &dyn vyre_driver::DeviceBuffer,
    ) -> Result<Vec<u8>, vyre_driver::BackendError> {
        self.download_wgpu_device_buffer(buffer)
    }

    fn free_device_buffer(
        &self,
        buffer: Box<dyn vyre_driver::DeviceBuffer>,
    ) -> Result<(), vyre_driver::BackendError> {
        self.free_wgpu_device_buffer(buffer)
    }

    fn allocate_resident(
        &self,
        byte_len: usize,
    ) -> Result<vyre_driver::Resource, vyre_driver::BackendError> {
        crate::resident_resource::allocate_resident(self, byte_len)
    }

    fn upload_resident(
        &self,
        resource: &vyre_driver::Resource,
        bytes: &[u8],
    ) -> Result<(), vyre_driver::BackendError> {
        crate::resident_upload::upload_resident(self, resource, bytes)
    }

    fn upload_resident_many(
        &self,
        uploads: &[(&vyre_driver::Resource, &[u8])],
    ) -> Result<(), vyre_driver::BackendError> {
        crate::resident_upload::upload_resident_many(self, uploads)
    }

    fn upload_resident_at(
        &self,
        resource: &vyre_driver::Resource,
        dst_offset_bytes: usize,
        bytes: &[u8],
    ) -> Result<(), vyre_driver::BackendError> {
        crate::resident_upload::upload_resident_at(self, resource, dst_offset_bytes, bytes)
    }

    fn upload_resident_at_many(
        &self,
        uploads: &[(&vyre_driver::Resource, usize, &[u8])],
    ) -> Result<(), vyre_driver::BackendError> {
        crate::resident_upload::upload_resident_at_many(self, uploads)
    }

    fn download_resident(
        &self,
        resource: &vyre_driver::Resource,
    ) -> Result<Vec<u8>, vyre_driver::BackendError> {
        crate::resident_download::download_resident(self, resource)
    }

    fn download_resident_into(
        &self,
        resource: &vyre_driver::Resource,
        out: &mut Vec<u8>,
    ) -> Result<(), vyre_driver::BackendError> {
        crate::resident_download::download_resident_into(self, resource, out)
    }

    fn download_resident_range(
        &self,
        resource: &vyre_driver::Resource,
        byte_offset: usize,
        byte_len: usize,
    ) -> Result<Vec<u8>, vyre_driver::BackendError> {
        crate::resident_download::download_resident_range(self, resource, byte_offset, byte_len)
    }

    fn download_resident_range_into(
        &self,
        resource: &vyre_driver::Resource,
        byte_offset: usize,
        byte_len: usize,
        out: &mut Vec<u8>,
    ) -> Result<(), vyre_driver::BackendError> {
        crate::resident_download::download_resident_range_into(
            self,
            resource,
            byte_offset,
            byte_len,
            out,
        )
    }

    fn download_resident_ranges_into(
        &self,
        ranges: &[(&vyre_driver::Resource, usize, usize)],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), vyre_driver::BackendError> {
        crate::resident_download::download_resident_ranges_into(self, ranges, outputs)
    }

    fn free_resident(
        &self,
        resource: vyre_driver::Resource,
    ) -> Result<(), vyre_driver::BackendError> {
        crate::resident_resource::free_resident(self, resource)
    }

    fn dispatch_resident_timed(
        &self,
        program: &Program,
        resources: &[vyre_driver::Resource],
        config: &vyre_driver::DispatchConfig,
    ) -> Result<vyre_driver::TimedDispatchResult, vyre_driver::BackendError> {
        crate::resident_dispatch::dispatch_resident_timed(self, program, resources, config)
    }

    fn dispatch_with_device_buffers(
        &self,
        program: &Program,
        inputs: &[&dyn vyre_driver::DeviceBuffer],
        outputs: &mut [&mut dyn vyre_driver::DeviceBuffer],
        config: &vyre_driver::DispatchConfig,
    ) -> Result<(), vyre_driver::BackendError> {
        // Validate all buffers were allocated by us so the downcast
        // below cannot fail mid-loop after partial side-effects.
        vyre_driver::validate_buffer_ownership(self.id(), inputs.iter().copied())?;
        vyre_driver::validate_buffer_ownership(
            self.id(),
            outputs
                .iter()
                .map(|b| &**b as &dyn vyre_driver::DeviceBuffer),
        )?;

        let resource_count = inputs.len().checked_add(outputs.len()).ok_or_else(|| {
            vyre_driver::BackendError::new(
                "resident dispatch resource count overflowed usize. Fix: split input/output resources before dispatch.",
            )
        })?;
        let mut resources =
            smallvec::SmallVec::<[vyre_driver::Resource; 8]>::with_capacity(resource_count);
        for buffer in inputs {
            let wgpu_buf = buffer
                .as_any()
                .downcast_ref::<crate::WgpuDeviceBuffer>()
                .ok_or_else(|| {
                    vyre_driver::BackendError::new(format!(
                        "Fix: dispatch_with_device_buffers expected WgpuDeviceBuffer inputs but got buffer owned by `{}`.",
                        buffer.backend_id()
                    ))
                })?;
            resources.push(vyre_driver::Resource::Resident(wgpu_buf.handle().id()));
        }
        for buffer in outputs.iter() {
            let backend_id = buffer.backend_id().to_string();
            let wgpu_buf = buffer
                .as_any()
                .downcast_ref::<crate::WgpuDeviceBuffer>()
                .ok_or_else(|| {
                    vyre_driver::BackendError::new(format!(
                        "Fix: dispatch_with_device_buffers expected WgpuDeviceBuffer outputs but got buffer owned by `{backend_id}`."
                    ))
                })?;
            resources.push(vyre_driver::Resource::Resident(wgpu_buf.handle().id()));
        }

        let pipeline = self
            .compile_native(program, config)?
            .ok_or_else(|| {
                vyre_driver::BackendError::new(
                    "Fix: WgpuBackend::compile_native unexpectedly returned None for dispatch_with_device_buffers.",
                )
            })?;
        let _outputs = pipeline.dispatch_persistent_handles(&resources, config)?;
        Ok(())
    }

    fn pipeline_cache_snapshot(&self) -> Option<vyre_driver::pipeline::PipelineCacheSnapshot> {
        Some(vyre_driver::pipeline::PipelineCacheSnapshot {
            hits: self.pipeline_cache.hits(),
            misses: self.pipeline_cache.misses(),
        })
    }

    fn supports_subgroup_ops(&self) -> bool {
        crate::capabilities::supports_subgroup_ops(&self.enabled_features)
    }

    fn supports_f16(&self) -> bool {
        false
    }

    fn supports_bf16(&self) -> bool {
        false
    }

    fn supports_tensor_cores(&self) -> bool {
        false
    }

    fn supports_async_compute(&self) -> bool {
        false
    }

    fn supports_indirect_dispatch(&self) -> bool {
        crate::capabilities::supports_indirect_dispatch(&self.adapter_info, &self.enabled_features)
    }

    fn supports_speculation(&self) -> bool {
        false
    }

    fn supports_persistent_thread_dispatch(&self) -> bool {
        false
    }

    fn is_distributed(&self) -> bool {
        false
    }

    fn max_workgroup_size(&self) -> [u32; 3] {
        self.enabled_features.max_workgroup_size
    }

    fn max_compute_workgroups_per_dimension(&self) -> u32 {
        self.device_limits.max_compute_workgroups_per_dimension
    }

    fn max_compute_invocations_per_workgroup(&self) -> u32 {
        self.device_limits.max_compute_invocations_per_workgroup
    }

    fn subgroup_size(&self) -> Option<u32> {
        crate::capabilities::supports_subgroup_ops(&self.enabled_features)
            .then_some(self.enabled_features.min_subgroup_size)
    }

    fn max_storage_buffer_bytes(&self) -> u64 {
        self.enabled_features.max_storage_buffer_binding_size
    }

    fn device_profile(&self) -> vyre_driver::DeviceProfile {
        WgpuBackend::device_profile(self)
    }

    fn flush(&self) -> Result<(), vyre_driver::BackendError> {
        let device_queue = self.current_device_queue();
        let submission = device_queue.1.submit(std::iter::empty());
        crate::runtime::device::poll_device_wait_for(&device_queue.0, submission)?;
        crate::pipeline::disk_cache::flush_disk_pipeline_cache()
    }

    fn device_lost(&self) -> bool {
        self.device_lost.load(Ordering::Acquire)
    }

    fn try_recover(&self) -> Result<(), vyre_driver::BackendError> {
        let ((device, queue), adapter_info, enabled) = match &self.recovery_target {
            AdapterRecoveryTarget::Index(index) => {
                crate::runtime::device::init_device_for_adapter(*index)
            }
            AdapterRecoveryTarget::Identity(identity) => {
                crate::runtime::device::init_device_for_adapter_identity(identity)
            }
        }
        .map_err(|error| vyre_driver::BackendError::new(error.to_string()))?;
        let device_limits = device.limits();
        let recovered_identity = crate::runtime::device::AdapterIdentity::from_info(&adapter_info);
        let original_identity =
            crate::runtime::device::AdapterIdentity::from_info(&self.adapter_info);
        if recovered_identity != original_identity {
            return Err(vyre_driver::BackendError::new(format!(
                "wgpu recovery selected a different adapter than the backend was constructed with. Original: {:?}; recovered: {:?}. Fix: construct a new backend for the new adapter instead of reusing device-local caches across adapter identities.",
                self.adapter_info, adapter_info
            )));
        }
        if device_limits != self.device_limits || enabled != self.enabled_features {
            return Err(vyre_driver::BackendError::new(
                "wgpu recovery selected the original adapter but feature or limit negotiation changed. Fix: construct a new backend so dispatch planning and pipeline caches are rebuilt against the new device contract.",
            ));
        }
        let cache_tiers = vec![
            crate::runtime::cache::CacheTier::try_new("hot", 1 << 24)?,
            crate::runtime::cache::CacheTier::try_new("cold", 1 << 30)?,
        ];
        let persistent_pool = crate::buffer::BufferPool::with_tiering(
            device.clone(),
            queue.clone(),
            &vyre_driver::DispatchConfig::default(),
            cache_tiers,
        )?;
        self.device_queue
            .store(Arc::new((device.clone(), queue.clone())));
        self.persistent_pool.store(Arc::new(persistent_pool));
        self.pipeline_cache.clear();
        self.wgsl_dispatch_pipeline_cache.clear();
        self.bind_group_layout_cache.clear();
        self.validation_cache.clear()?;
        self.dispatch_arena.store(Arc::new(DispatchArena::new(
            device.clone(),
            queue.clone(),
            &vyre_driver::DispatchConfig::default(),
        )));
        self.device_lost.store(false, Ordering::Release);

        Ok(())
    }
}

impl vyre_self_substrate::optimizer::dispatcher::OptimizerDispatcher for WgpuBackend {
    fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, vyre_self_substrate::optimizer::dispatcher::DispatchError> {
        let mut config = vyre_driver::DispatchConfig::default();
        config.grid_override = grid_override;
        vyre_driver::VyreBackend::dispatch(self, program, inputs, &config).map_err(|error| {
            vyre_self_substrate::optimizer::dispatcher::DispatchError::BackendError(
                error.to_string(),
            )
        })
    }
}

#[cfg(test)]
mod borrowed_slice_conversion_tests {
    use super::{
        borrowed_slices_from_owned_inputs, empty_batch_result_slots, finalize_batch_results,
    };

    #[test]
    fn dispatch_async_input_conversion_is_zero_copy_slice_refs() {
        let inputs = vec![vec![1u8, 2, 3], vec![4u8, 5]];
        let borrowed = borrowed_slices_from_owned_inputs(&inputs);
        assert_eq!(borrowed.len(), 2);
        assert_eq!(borrowed[0].as_ptr(), inputs[0].as_ptr());
        assert_eq!(borrowed[1].as_ptr(), inputs[1].as_ptr());
    }

    #[test]
    fn nine_inputs_spill_smallvec_but_slices_alias_vecs() {
        let inputs: Vec<Vec<u8>> = (0..9).map(|i| vec![i as u8]).collect();
        let borrowed = borrowed_slices_from_owned_inputs(&inputs);
        assert_eq!(borrowed.len(), 9);
        for i in 0..9 {
            assert_eq!(
                borrowed[i].as_ptr(),
                inputs[i].as_ptr(),
                "slice {i} must reference the corresponding Vec buffer"
            );
        }
    }

    #[test]
    fn generated_batch_result_finalization_preserves_success_error_and_missing_slots() {
        for case in 0..4096usize {
            let len = (case % 19) + 1;
            let mut slots = empty_batch_result_slots::<usize>(len)
                .expect("Fix: generated WGPU batch result test must reserve slots");
            for slot in 0..len {
                match (slot + case) % 7 {
                    0 => {}
                    1 => {
                        slots[slot] = Some(Err(vyre_driver::BackendError::new(format!(
                            "generated-error-{case}-{slot}"
                        ))));
                    }
                    _ => {
                        slots[slot] = Some(Ok(case * 100 + slot));
                    }
                }
            }

            let finalized =
                finalize_batch_results(slots, "generated missing WGPU batch result slot")
                    .expect("Fix: generated WGPU batch finalization must reserve output results");
            assert_eq!(
                finalized.len(),
                len,
                "generated WGPU batch case {case} must preserve slot count"
            );
            for (slot, result) in finalized.into_iter().enumerate() {
                match (slot + case) % 7 {
                    0 => {
                        let error =
                            result.expect_err("Fix: missing generated batch slot must error");
                        assert!(
                            error
                                .to_string()
                                .contains("generated missing WGPU batch result slot"),
                            "Fix: missing generated batch slot must report the supplied invariant, got {error}"
                        );
                    }
                    1 => {
                        let error = result
                            .expect_err("Fix: explicit generated batch error must stay error");
                        assert!(
                            error
                                .to_string()
                                .contains(&format!("generated-error-{case}-{slot}")),
                            "Fix: explicit generated batch error must be preserved, got {error}"
                        );
                    }
                    _ => {
                        assert_eq!(
                            result.expect("Fix: generated batch success must stay success"),
                            case * 100 + slot
                        );
                    }
                }
            }
        }
    }
}
