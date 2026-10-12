//! Shared grid-sync wrapper for backends without native cooperative barriers.

use std::collections::HashSet;

use smallvec::SmallVec;
use vyre_foundation::ir::OpId;
use vyre_foundation::ir::Program;

use crate::backend::{
    BackendError, CompiledPipeline, DispatchConfig, OutputBuffers, PendingDispatch,
    ResidentDispatchStep, ResidentReadRange, Resource, TimedDispatchResult, VyreBackend,
};

pub(super) fn wrap_grid_sync_split(backend: Box<dyn VyreBackend>) -> Box<dyn VyreBackend> {
    Box::new(GridSyncSplitBackend { inner: backend })
}

struct GridSyncSplitBackend {
    inner: Box<dyn VyreBackend>,
}

impl super::super::private::Sealed for GridSyncSplitBackend {}

impl VyreBackend for GridSyncSplitBackend {
    fn id(&self) -> &'static str {
        self.inner.id()
    }

    fn version(&self) -> &'static str {
        self.inner.version()
    }

    fn supported_ops(&self) -> &HashSet<OpId> {
        self.inner.supported_ops()
    }

    fn allows_host_grid_sync_split(&self) -> bool {
        self.inner.allows_host_grid_sync_split()
    }

    fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        if self.should_split_grid_sync(program) {
            let borrowed = borrowed_inputs_from_owned(inputs)?;
            return crate::grid_sync::dispatch_with_grid_sync_split(
                self.inner.as_ref(),
                program,
                &borrowed,
                config,
            );
        }
        self.inner.dispatch(program, inputs, config)
    }

    fn dispatch_borrowed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        if self.should_split_grid_sync(program) {
            return crate::grid_sync::dispatch_with_grid_sync_split(
                self.inner.as_ref(),
                program,
                inputs,
                config,
            );
        }
        self.inner.dispatch_borrowed(program, inputs, config)
    }

    fn dispatch_borrowed_timed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        if self.should_split_grid_sync(program) {
            return crate::grid_sync::dispatch_with_grid_sync_split_timed(
                self.inner.as_ref(),
                program,
                inputs,
                config,
            );
        }
        self.inner.dispatch_borrowed_timed(program, inputs, config)
    }

    fn dispatch_borrowed_into(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        if self.should_split_grid_sync(program) {
            return crate::grid_sync::dispatch_with_grid_sync_split_into(
                self.inner.as_ref(),
                program,
                inputs,
                config,
                outputs,
            );
        }
        self.inner
            .dispatch_borrowed_into(program, inputs, config, outputs)
    }

    fn compile_native(
        &self,
        program: &Program,
        config: &DispatchConfig,
    ) -> Result<Option<std::sync::Arc<dyn CompiledPipeline>>, BackendError> {
        if self.should_split_grid_sync(program) {
            return Ok(None);
        }
        self.inner.compile_native(program, config)
    }

    fn pipeline_cache_snapshot(&self) -> Option<crate::pipeline::PipelineCacheSnapshot> {
        self.inner.pipeline_cache_snapshot()
    }

    fn backend_metric_snapshot(&self) -> Vec<(&'static str, u64)> {
        self.inner.backend_metric_snapshot()
    }

    fn allocate_resident(&self, byte_len: usize) -> Result<Resource, BackendError> {
        self.inner.allocate_resident(byte_len)
    }

    fn upload_resident(&self, resource: &Resource, bytes: &[u8]) -> Result<(), BackendError> {
        self.inner.upload_resident(resource, bytes)
    }

    fn upload_resident_many(&self, uploads: &[(&Resource, &[u8])]) -> Result<(), BackendError> {
        self.inner.upload_resident_many(uploads)
    }

    fn upload_resident_at(
        &self,
        resource: &Resource,
        dst_offset_bytes: usize,
        bytes: &[u8],
    ) -> Result<(), BackendError> {
        self.inner
            .upload_resident_at(resource, dst_offset_bytes, bytes)
    }

    fn upload_resident_at_many(
        &self,
        uploads: &[(&Resource, usize, &[u8])],
    ) -> Result<(), BackendError> {
        self.inner.upload_resident_at_many(uploads)
    }

    fn download_resident(&self, resource: &Resource) -> Result<Vec<u8>, BackendError> {
        self.inner.download_resident(resource)
    }

    fn download_resident_into(
        &self,
        resource: &Resource,
        out: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        self.inner.download_resident_into(resource, out)
    }

    fn download_resident_range(
        &self,
        resource: &Resource,
        byte_offset: usize,
        byte_len: usize,
    ) -> Result<Vec<u8>, BackendError> {
        self.inner
            .download_resident_range(resource, byte_offset, byte_len)
    }

    fn download_resident_range_into(
        &self,
        resource: &Resource,
        byte_offset: usize,
        byte_len: usize,
        out: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        self.inner
            .download_resident_range_into(resource, byte_offset, byte_len, out)
    }

    fn download_resident_ranges_into(
        &self,
        ranges: &[(&Resource, usize, usize)],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
        self.inner.download_resident_ranges_into(ranges, outputs)
    }

    fn free_resident(&self, resource: Resource) -> Result<(), BackendError> {
        self.inner.free_resident(resource)
    }

    fn dispatch_resident_timed(
        &self,
        program: &Program,
        resources: &[Resource],
        config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        if self.should_split_grid_sync(program) {
            return crate::grid_sync::dispatch_resident_with_grid_sync_split_timed(
                self.inner.as_ref(),
                program,
                resources,
                config,
            );
        }
        self.inner
            .dispatch_resident_timed(program, resources, config)
    }

    fn dispatch_resident_sequence_read_ranges_into(
        &self,
        steps: &[ResidentDispatchStep<'_>],
        read_ranges: &[ResidentReadRange<'_>],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
        if steps
            .iter()
            .any(|step| self.should_split_grid_sync(step.program))
        {
            for step in steps {
                let mut config = DispatchConfig::default();
                config.grid_override = step.grid_override;
                self.dispatch_resident_timed(step.program, step.resources, &config)?;
            }
            let ranges = read_ranges
                .iter()
                .map(|range| (range.resource, range.byte_offset, range.byte_len))
                .collect::<SmallVec<[_; 8]>>();
            return self.download_resident_ranges_into(&ranges, outputs);
        }
        self.inner
            .dispatch_resident_sequence_read_ranges_into(steps, read_ranges, outputs)
    }

    fn dispatch_resident_repeated_sequence_read_ranges_into(
        &self,
        prefix_steps: &[ResidentDispatchStep<'_>],
        repeated_steps: &[ResidentDispatchStep<'_>],
        repeat_count: u32,
        read_ranges: &[ResidentReadRange<'_>],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
        if prefix_steps
            .iter()
            .chain(repeated_steps)
            .any(|step| self.should_split_grid_sync(step.program))
        {
            for step in prefix_steps {
                let mut config = DispatchConfig::default();
                config.grid_override = step.grid_override;
                self.dispatch_resident_timed(step.program, step.resources, &config)?;
            }
            for _ in 0..repeat_count {
                for step in repeated_steps {
                    let mut config = DispatchConfig::default();
                    config.grid_override = step.grid_override;
                    self.dispatch_resident_timed(step.program, step.resources, &config)?;
                }
            }
            let ranges = read_ranges
                .iter()
                .map(|range| (range.resource, range.byte_offset, range.byte_len))
                .collect::<SmallVec<[_; 8]>>();
            return self.download_resident_ranges_into(&ranges, outputs);
        }
        self.inner
            .dispatch_resident_repeated_sequence_read_ranges_into(
                prefix_steps,
                repeated_steps,
                repeat_count,
                read_ranges,
                outputs,
            )
    }

    fn dispatch_async(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Box<dyn PendingDispatch>, BackendError> {
        if self.should_split_grid_sync(program) {
            let borrowed = borrowed_inputs_from_owned(inputs)?;
            let outputs = crate::grid_sync::dispatch_with_grid_sync_split(
                self.inner.as_ref(),
                program,
                &borrowed,
                config,
            )?;
            return Ok(Box::new(super::super::pending_dispatch::ReadyPending {
                outputs,
            }));
        }
        self.inner.dispatch_async(program, inputs, config)
    }

    fn dispatch_borrowed_async(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Box<dyn PendingDispatch>, BackendError> {
        if self.should_split_grid_sync(program) {
            let outputs = crate::grid_sync::dispatch_with_grid_sync_split(
                self.inner.as_ref(),
                program,
                inputs,
                config,
            )?;
            return Ok(Box::new(super::super::pending_dispatch::ReadyPending {
                outputs,
            }));
        }
        self.inner.dispatch_borrowed_async(program, inputs, config)
    }

    fn supports_subgroup_ops(&self) -> bool {
        self.inner.supports_subgroup_ops()
    }

    fn supports_f16(&self) -> bool {
        self.inner.supports_f16()
    }

    fn supports_bf16(&self) -> bool {
        self.inner.supports_bf16()
    }

    fn supports_tensor_cores(&self) -> bool {
        self.inner.supports_tensor_cores()
    }

    fn supports_async_compute(&self) -> bool {
        self.inner.supports_async_compute()
    }

    fn supports_indirect_dispatch(&self) -> bool {
        self.inner.supports_indirect_dispatch()
    }

    fn supports_speculation(&self) -> bool {
        self.inner.supports_speculation()
    }

    fn supports_persistent_thread_dispatch(&self) -> bool {
        self.inner.supports_persistent_thread_dispatch()
    }

    fn supports_grid_sync(&self) -> bool {
        self.inner.supports_grid_sync()
    }

    fn is_distributed(&self) -> bool {
        self.inner.is_distributed()
    }

    fn max_workgroup_size(&self) -> [u32; 3] {
        self.inner.max_workgroup_size()
    }

    fn max_compute_workgroups_per_dimension(&self) -> u32 {
        self.inner.max_compute_workgroups_per_dimension()
    }

    fn max_compute_invocations_per_workgroup(&self) -> u32 {
        self.inner.max_compute_invocations_per_workgroup()
    }

    fn subgroup_size(&self) -> Option<u32> {
        self.inner.subgroup_size()
    }

    fn max_storage_buffer_bytes(&self) -> u64 {
        self.inner.max_storage_buffer_bytes()
    }

    fn prepare(&self) -> Result<(), BackendError> {
        self.inner.prepare()
    }

    fn flush(&self) -> Result<(), BackendError> {
        self.inner.flush()
    }

    fn shutdown(&self) -> Result<(), BackendError> {
        self.inner.shutdown()
    }

    fn device_lost(&self) -> bool {
        self.inner.device_lost()
    }

    fn try_recover(&self) -> Result<(), BackendError> {
        self.inner.try_recover()
    }
}

impl GridSyncSplitBackend {
    fn should_split_grid_sync(&self, program: &Program) -> bool {
        crate::grid_sync::contains_grid_sync(program)
            && !self.inner.supports_grid_sync()
            && self.inner.allows_host_grid_sync_split()
    }
}

fn borrowed_inputs_from_owned(inputs: &[Vec<u8>]) -> Result<Vec<&[u8]>, BackendError> {
    let mut borrowed = Vec::new();
    if borrowed.capacity() < inputs.len() {
        borrowed
            .try_reserve_exact(inputs.len() - borrowed.len())
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: failed to reserve {} borrowed grid-sync input views for registry wrapper dispatch: {error}. Use borrowed dispatch directly or shard the host-side split.",
                    inputs.len()
                ),
            })?;
    }
    borrowed.extend(inputs.iter().map(Vec::as_slice));
    Ok(borrowed)
}

#[cfg(test)]
mod tests {
    use super::wrap_grid_sync_split;
    use crate::backend::registry::registered_backends;
    use crate::{
        BackendError, DispatchConfig, ResidentDispatchStep, ResidentReadRange, Resource,
        VyreBackend,
    };
    use smallvec::SmallVec;
    use std::sync::{Arc, Mutex};
    use vyre_foundation::ir::{BufferDecl, DataType, Node, Program};
    use vyre_foundation::memory_model::MemoryOrdering;

    #[test]
    fn registry_grid_sync_wrapper_uses_hardened_fallible_split_paths() {
        let source = include_str!("grid_sync_split.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: registry grid-sync split production source must precede tests");

        assert!(
            production.contains("fn borrowed_inputs_from_owned")
                && production.contains("try_reserve_exact")
                && production.contains("dispatch_with_grid_sync_split_timed"),
            "Fix: registry grid-sync wrapper must use fallible borrowed-input staging and the hardened shared timed split path."
        );
        assert!(
            !production.contains("SmallVec::with_capacity")
                && !production.contains(".as_nanos() as u64"),
            "Fix: registry grid-sync wrapper must not keep infallible input staging or lossy timing conversion."
        );
    }

    #[test]
    fn vyre_core_alone_sees_no_backends() {
        assert!(
            registered_backends().is_empty(),
            "vyre-core has no backend deps; registry must be empty here. \
             Fix: if a backend crate was added as a dependency, move this \
            assertion into that crate's test suite."
        );
    }

    #[derive(Default)]
    struct SegmentRecorder {
        calls: Mutex<Vec<(bool, Vec<Vec<u8>>)>>,
    }

    impl super::super::super::private::Sealed for SegmentRecorder {}

    impl VyreBackend for SegmentRecorder {
        fn id(&self) -> &'static str {
            "segment-recorder"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "owned dispatch should not run for split borrowed path. Fix: keep grid-sync split on the borrowed segment dispatcher.",
            ))
        }

        fn dispatch_borrowed(
            &self,
            program: &Program,
            inputs: &[&[u8]],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            let mut calls = self.calls.lock().map_err(BackendError::poisoned_lock)?;
            let has_grid_sync = crate::grid_sync::contains_grid_sync(program);
            let captured = inputs
                .iter()
                .map(|input| input.to_vec())
                .collect::<Vec<_>>();
            calls.push((has_grid_sync, captured));
            Ok(vec![vec![calls.len() as u8]])
        }
    }

    fn grid_sync_program() -> Program {
        Program::wrapped(
            vec![BufferDecl::read_write("state", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![
                Node::Return,
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                Node::Return,
            ],
        )
    }

    #[test]
    fn registered_backend_wrapper_splits_grid_sync_without_recursing() {
        let recorder = Arc::new(SegmentRecorder::default());
        let backend = wrap_grid_sync_split(Box::new(ArcBackend {
            inner: Arc::clone(&recorder),
        }));
        let inputs = [vec![0u8]];
        let borrowed: SmallVec<[&[u8]; 8]> = inputs.iter().map(Vec::as_slice).collect();

        let outputs = backend
            .dispatch_borrowed(&grid_sync_program(), &borrowed, &DispatchConfig::default())
            .expect("Fix: grid-sync split wrapper must dispatch every segment");

        assert_eq!(outputs, vec![vec![2]]);
        let calls = recorder
            .calls
            .lock()
            .expect("Fix: segment recorder mutex must not be poisoned");
        assert_eq!(calls.len(), 2);
        assert!(
            calls.iter().all(|(has_grid_sync, _)| !*has_grid_sync),
            "split segment dispatches must not contain GridSync barriers"
        );
        assert_eq!(calls[0].1, vec![vec![0]]);
        assert_eq!(
            calls[1].1,
            vec![vec![1]],
            "second segment must receive the first segment's ReadWrite output"
        );
    }

    struct NativeGridSyncProbe {
        calls: Mutex<usize>,
    }

    impl super::super::super::private::Sealed for NativeGridSyncProbe {}

    impl VyreBackend for NativeGridSyncProbe {
        fn id(&self) -> &'static str {
            "native-grid-sync-probe"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "owned dispatch should not run for this test. Fix: keep the borrowed path selected.",
            ))
        }

        fn dispatch_borrowed(
            &self,
            program: &Program,
            _inputs: &[&[u8]],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            assert!(
                crate::grid_sync::contains_grid_sync(program),
                "native grid-sync backends must receive the original unsplit Program"
            );
            *self.calls.lock().map_err(BackendError::poisoned_lock)? += 1;
            Ok(vec![vec![9]])
        }

        fn supports_grid_sync(&self) -> bool {
            true
        }
    }

    #[test]
    fn registered_backend_wrapper_preserves_native_grid_sync_dispatch() {
        let probe = Arc::new(NativeGridSyncProbe {
            calls: Mutex::new(0),
        });
        let backend = wrap_grid_sync_split(Box::new(ArcBackend {
            inner: Arc::clone(&probe),
        }));

        let outputs = backend
            .dispatch_borrowed(&grid_sync_program(), &[], &DispatchConfig::default())
            .expect("Fix: native grid-sync backend should receive original dispatch");

        assert_eq!(outputs, vec![vec![9]]);
        assert_eq!(
            *probe
                .calls
                .lock()
                .expect("Fix: native probe mutex must not be poisoned"),
            1
        );
    }

    struct ResidentUploadProbe {
        uploads: Mutex<Vec<(u64, usize, usize)>>,
    }

    impl super::super::super::private::Sealed for ResidentUploadProbe {}

    impl VyreBackend for ResidentUploadProbe {
        fn id(&self) -> &'static str {
            "resident-upload-probe"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "resident upload forwarding test must not dispatch programs.",
            ))
        }

        fn upload_resident_at_many(
            &self,
            uploads: &[(&Resource, usize, &[u8])],
        ) -> Result<(), BackendError> {
            let mut captured = self.uploads.lock().map_err(BackendError::poisoned_lock)?;
            for &(resource, offset, bytes) in uploads {
                let Resource::Resident(handle) = resource else {
                    return Err(BackendError::new(
                        "resident upload forwarding test expected resident handles.",
                    ));
                };
                captured.push((*handle, offset, bytes.len()));
            }
            Ok(())
        }
    }

    #[test]
    fn registered_backend_wrapper_forwards_ranged_resident_uploads() {
        let probe = Arc::new(ResidentUploadProbe {
            uploads: Mutex::new(Vec::new()),
        });
        let backend = wrap_grid_sync_split(Box::new(ArcBackend {
            inner: Arc::clone(&probe),
        }));

        backend
            .upload_resident_at_many(&[(&Resource::Resident(7), 12, &[1, 2, 3])])
            .expect("Fix: grid-sync split wrapper must forward resident ranged uploads");

        assert_eq!(
            probe
                .uploads
                .lock()
                .expect("Fix: resident upload probe mutex must not be poisoned")
                .as_slice(),
            &[(7, 12, 3)]
        );
    }

    struct ResidentSequenceProbe {
        calls: Mutex<Vec<(usize, usize, u32, usize)>>,
    }

    impl super::super::super::private::Sealed for ResidentSequenceProbe {}

    impl VyreBackend for ResidentSequenceProbe {
        fn id(&self) -> &'static str {
            "resident-sequence-probe"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "resident sequence forwarding test must not dispatch owned inputs.",
            ))
        }

        fn dispatch_resident_repeated_sequence_read_ranges_into(
            &self,
            prefix_steps: &[ResidentDispatchStep<'_>],
            repeated_steps: &[ResidentDispatchStep<'_>],
            repeat_count: u32,
            read_ranges: &[ResidentReadRange<'_>],
            outputs: &mut [&mut Vec<u8>],
        ) -> Result<(), BackendError> {
            self.calls
                .lock()
                .map_err(BackendError::poisoned_lock)?
                .push((
                    prefix_steps.len(),
                    repeated_steps.len(),
                    repeat_count,
                    read_ranges.len(),
                ));
            for (index, output) in outputs.iter_mut().enumerate() {
                output.clear();
                output.push(index as u8 + 10);
            }
            Ok(())
        }
    }

    #[test]
    fn registered_backend_wrapper_forwards_resident_repeated_sequences() {
        let probe = Arc::new(ResidentSequenceProbe {
            calls: Mutex::new(Vec::new()),
        });
        let backend = wrap_grid_sync_split(Box::new(ArcBackend {
            inner: Arc::clone(&probe),
        }));
        let program = Program::wrapped(Vec::new(), [1, 1, 1], Vec::new());
        let resources = [Resource::Resident(9)];
        let prefix_steps = [ResidentDispatchStep {
            program: &program,
            resources: &resources,
            grid_override: None,
            workgroup_override: None,
        }];
        let repeated_steps = [ResidentDispatchStep {
            program: &program,
            resources: &resources,
            grid_override: Some([3, 1, 1]),
            workgroup_override: None,
        }];
        let read_ranges = [
            ResidentReadRange {
                resource: &resources[0],
                byte_offset: 0,
                byte_len: 1,
            },
            ResidentReadRange {
                resource: &resources[0],
                byte_offset: 4,
                byte_len: 1,
            },
        ];
        let mut first = Vec::new();
        let mut second = Vec::new();

        backend
            .dispatch_resident_repeated_sequence_read_ranges_into(
                &prefix_steps,
                &repeated_steps,
                4,
                &read_ranges,
                &mut [&mut first, &mut second],
            )
            .expect("Fix: grid-sync split wrapper must forward resident repeated sequences");

        assert_eq!(first, vec![10]);
        assert_eq!(second, vec![11]);
        assert_eq!(
            probe
                .calls
                .lock()
                .expect("Fix: resident sequence probe mutex must not be poisoned")
                .as_slice(),
            &[(1, 1, 4, 2)]
        );
    }

    struct ArcBackend<T: VyreBackend + 'static> {
        inner: Arc<T>,
    }

    impl<T: VyreBackend + 'static> super::super::super::private::Sealed for ArcBackend<T> {}

    impl<T: VyreBackend + 'static> VyreBackend for ArcBackend<T> {
        fn id(&self) -> &'static str {
            self.inner.id()
        }

        fn dispatch(
            &self,
            program: &Program,
            inputs: &[Vec<u8>],
            config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            self.inner.dispatch(program, inputs, config)
        }

        fn dispatch_borrowed(
            &self,
            program: &Program,
            inputs: &[&[u8]],
            config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            self.inner.dispatch_borrowed(program, inputs, config)
        }

        fn upload_resident_at_many(
            &self,
            uploads: &[(&Resource, usize, &[u8])],
        ) -> Result<(), BackendError> {
            self.inner.upload_resident_at_many(uploads)
        }

        fn download_resident_ranges_into(
            &self,
            ranges: &[(&Resource, usize, usize)],
            outputs: &mut [&mut Vec<u8>],
        ) -> Result<(), BackendError> {
            self.inner.download_resident_ranges_into(ranges, outputs)
        }

        fn dispatch_resident_sequence_read_ranges_into(
            &self,
            steps: &[ResidentDispatchStep<'_>],
            read_ranges: &[ResidentReadRange<'_>],
            outputs: &mut [&mut Vec<u8>],
        ) -> Result<(), BackendError> {
            self.inner
                .dispatch_resident_sequence_read_ranges_into(steps, read_ranges, outputs)
        }

        fn dispatch_resident_repeated_sequence_read_ranges_into(
            &self,
            prefix_steps: &[ResidentDispatchStep<'_>],
            repeated_steps: &[ResidentDispatchStep<'_>],
            repeat_count: u32,
            read_ranges: &[ResidentReadRange<'_>],
            outputs: &mut [&mut Vec<u8>],
        ) -> Result<(), BackendError> {
            self.inner
                .dispatch_resident_repeated_sequence_read_ranges_into(
                    prefix_steps,
                    repeated_steps,
                    repeat_count,
                    read_ranges,
                    outputs,
                )
        }

        fn supports_grid_sync(&self) -> bool {
            self.inner.supports_grid_sync()
        }

        fn allows_host_grid_sync_split(&self) -> bool {
            self.inner.allows_host_grid_sync_split()
        }
    }

    struct GridSyncSplitOptOutProbe {
        calls: Mutex<usize>,
    }

    impl super::super::super::private::Sealed for GridSyncSplitOptOutProbe {}

    impl VyreBackend for GridSyncSplitOptOutProbe {
        fn id(&self) -> &'static str {
            "grid-sync-split-opt-out-probe"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "owned dispatch should not run for this test. Fix: keep the borrowed path selected.",
            ))
        }

        fn dispatch_borrowed(
            &self,
            program: &Program,
            _inputs: &[&[u8]],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            assert!(
                crate::grid_sync::contains_grid_sync(program),
                "split opt-out backends must receive the original GridSync program"
            );
            *self.calls.lock().map_err(BackendError::poisoned_lock)? += 1;
            Ok(vec![vec![13]])
        }

        fn allows_host_grid_sync_split(&self) -> bool {
            false
        }
    }

    #[test]
    fn registered_backend_wrapper_preserves_grid_sync_when_backend_opts_out_of_host_split() {
        let probe = Arc::new(GridSyncSplitOptOutProbe {
            calls: Mutex::new(0),
        });
        let backend = wrap_grid_sync_split(Box::new(ArcBackend {
            inner: Arc::clone(&probe),
        }));

        let outputs = backend
            .dispatch_borrowed(&grid_sync_program(), &[], &DispatchConfig::default())
            .expect("Fix: split opt-out backend must receive original dispatch");

        assert_eq!(outputs, vec![vec![13]]);
        assert_eq!(
            *probe
                .calls
                .lock()
                .expect("Fix: split opt-out probe mutex must not be poisoned"),
            1
        );
    }
}
