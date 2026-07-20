//! The frozen `VyreBackend` contract.

use std::sync::Arc;

use smallvec::SmallVec;
use vyre_foundation::ir::Program;

use crate::backend::{
    device_buffer::unsupported_device_buffer, private, BackendError, CompiledPipeline,
    DeviceBuffer, DispatchConfig, OutputBuffers, PendingDispatch, Resource, TimedDispatchResult,
};

/// One backend-resident program dispatch in an ordered sequence.
pub struct ResidentDispatchStep<'a> {
    /// Program to dispatch.
    pub program: &'a Program,
    /// Resident resources in binding order.
    pub resources: &'a [Resource],
    /// Optional CUDA/grid-style launch override.
    pub grid_override: Option<[u32; 3]>,
    /// Optional workgroup override. MUST be carried alongside `grid_override`:
    /// a caller that sizes its grid for a specific workgroup (grid =
    /// ceil(work / workgroup)) will under-cover the work if the step falls back
    /// to a different default workgroup. `None` keeps the backend's resolved
    /// default (correct only when `grid_override` is also `None`).
    pub workgroup_override: Option<[u32; 3]>,
}

/// One compact byte range to read from a backend-resident resource.
pub struct ResidentReadRange<'a> {
    /// Resident resource to read.
    pub resource: &'a Resource,
    /// Byte offset inside the resident resource.
    pub byte_offset: usize,
    /// Number of bytes to read.
    pub byte_len: usize,
}

/// Timing captured for an ordered resident dispatch sequence.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResidentSequenceTiming {
    /// Host-observed sequence duration including requested readbacks.
    pub wall_ns: u64,
    /// Device-observed elapsed dispatch time when the backend exposes timers.
    pub device_ns: Option<u64>,
    /// Host time spent enqueueing backend work before waiting.
    pub enqueue_ns: Option<u64>,
    /// Host time spent waiting for completion and collecting outputs.
    pub wait_ns: Option<u64>,
}

/// The frozen contract between vyre and every execution backend.
///
/// A backend is a pure function from a validated `Program` and input buffers
/// to output buffers. Implementations must be `Send + Sync`, deterministic
/// for identical inputs, and byte-identical to the CPU reference on success.
/// This trait is the keystone of the vyre abstraction thesis: frontends do
/// not know which backend runs their IR, and backends do not know which
/// frontend produced it.
///
/// # Examples
///
pub trait VyreBackend: private::Sealed + Send + Sync {
    /// Stable backend identifier used for logging, certificates, and adapter selection.
    ///
    /// The identifier must be unique among all backends linked into the
    /// current process. Conformance reports include this string so that
    /// consumers know exactly which implementation was certified.
    fn id(&self) -> &'static str;

    /// Backend implementation version string used for certificates and
    /// regression tracking.
    ///
    /// The default returns `"unspecified"`. Concrete backends should
    /// override this with their crate version (e.g. `"0.4.0"`) so that
    /// certificates can detect backend upgrades that may require re-cert.
    fn version(&self) -> &'static str {
        "unspecified"
    }

    /// Operation ids this backend can execute without further lowering.
    fn supported_ops(&self) -> &std::collections::HashSet<vyre_foundation::ir::OpId> {
        use crate::backend::validation::default_supported_ops;
        default_supported_ops()
    }

    // Raw backend shader text is a concrete-driver implementation
    // detail, not part of the substrate-neutral `VyreBackend`
    // contract.

    /// Executes the program with the given input buffers and returns the output buffers.
    ///
    /// On success the returned bytes must match the pure-Rust reference
    /// implementation bit-for-bit. On failure the backend must return a
    /// [`BackendError`] whose message contains an actionable `Fix: ` hint.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use vyre::{Program, VyreBackend, DispatchConfig};
    ///
    /// # fn example(backend: &dyn VyreBackend, program: &Program) -> Result<Vec<Vec<u8>>, vyre::BackendError> {
    /// let inputs = vec![vec![1u8, 2, 3]];
    /// let config = DispatchConfig::default();
    /// backend.dispatch(program, &inputs, &config)
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete dispatch.
    /// The error message always includes a `Fix: ` remediation section.
    fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError>;

    /// Executes the program with borrowed input buffers.
    ///
    /// Backends may override this method to avoid staging borrowed bytes into
    /// owned `Vec<u8>` buffers. The default is non-breaking: it performs one
    /// owned vector allocation for the call and delegates to
    /// [`VyreBackend::dispatch`].
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete dispatch.
    fn dispatch_borrowed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let owned =
            crate::backend::clone_borrowed_inputs_for_dispatch(inputs, "backend input staging")?;
        let outputs = self.dispatch(program, &owned, config)?;
        crate::observability::record_dispatch_io(inputs, &outputs);
        Ok(outputs)
    }

    /// Executes a borrowed-input dispatch and returns backend-owned timing.
    ///
    /// The default records host wall time and delegates to
    /// [`VyreBackend::dispatch_borrowed`]. Device-specific backends override
    /// this only inside their driver crates so benchmark crates never import
    /// vendor APIs directly.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete dispatch.
    fn dispatch_borrowed_timed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        let started = std::time::Instant::now();
        let outputs = self.dispatch_borrowed(program, inputs, config)?;
        Ok(TimedDispatchResult {
            outputs,
            wall_ns: crate::backend::checked_elapsed_wall_ns(started, "backend borrowed dispatch")?,
            device_ns: None,
            enqueue_ns: None,
            wait_ns: None,
        })
    }

    /// Executes the program with borrowed input buffers and writes outputs into
    /// caller-owned storage.
    ///
    /// Backends may override this method to reuse output buffers across
    /// dispatches. The default preserves the existing dispatch contract and
    /// copies returned bytes into existing output slots where possible.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete dispatch.
    fn dispatch_borrowed_into(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        let result = self.dispatch_borrowed(program, inputs, config)?;
        let stats = crate::backend::dispatch_result::replace_output_buffers_preserving_slots_with_memory_stats(
            result,
            outputs,
        );
        crate::observability::record_output_replacement_stats(stats);
        Ok(())
    }

    /// Allocate a backend-resident buffer and return a stable resource handle.
    ///
    /// Backends that support resident resources override this method so callers
    /// can keep hot inputs on the device without importing a concrete driver
    /// crate. The returned [`Resource`] is only meaningful to the backend that
    /// produced it.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot allocate a resident
    /// resource of the requested size.
    fn allocate_resident(&self, _byte_len: usize) -> Result<Resource, BackendError> {
        Err(BackendError::UnsupportedFeature {
            name: "resident buffer allocation".to_string(),
            backend: self.id().to_string(),
        })
    }

    /// Upload bytes into a backend-resident resource.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the resource is not owned by this backend
    /// or the byte length does not match the resident allocation.
    fn upload_resident(&self, _resource: &Resource, _bytes: &[u8]) -> Result<(), BackendError> {
        Err(BackendError::UnsupportedFeature {
            name: "resident buffer upload".to_string(),
            backend: self.id().to_string(),
        })
    }

    /// Upload several backend-resident resources as one logical staging
    /// operation.
    ///
    /// Backends that support resident graph/dataflow hot paths should override
    /// this with a native batched transfer. The default fails loudly instead
    /// of looping over [`VyreBackend::upload_resident`], because a hidden
    /// per-buffer synchronization loop destroys the performance contract.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot batch resident uploads
    /// or when any resource/byte length is invalid.
    fn upload_resident_many(&self, _uploads: &[(&Resource, &[u8])]) -> Result<(), BackendError> {
        Err(BackendError::UnsupportedFeature {
            name: "resident buffer batch upload".to_string(),
            backend: self.id().to_string(),
        })
    }

    /// Upload bytes into a subrange of a backend-resident resource.
    ///
    /// This is the hot-loop path for reusable resident slots whose capacity is
    /// larger than the current logical payload. Backends must not require the
    /// upload length to equal the allocation length.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when ranged upload is unsupported, the resource
    /// is not owned by this backend, or the destination range is out of bounds.
    fn upload_resident_at(
        &self,
        _resource: &Resource,
        _dst_offset_bytes: usize,
        _bytes: &[u8],
    ) -> Result<(), BackendError> {
        Err(BackendError::UnsupportedFeature {
            name: "resident buffer ranged upload".to_string(),
            backend: self.id().to_string(),
        })
    }

    /// Upload several resident subranges as one logical staging operation.
    ///
    /// The default fails loudly instead of looping over
    /// [`VyreBackend::upload_resident_at`], because hidden per-range
    /// synchronization breaks resident hot-loop performance.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when ranged batch upload is unsupported or when
    /// any resource/range is invalid.
    fn upload_resident_at_many(
        &self,
        _uploads: &[(&Resource, usize, &[u8])],
    ) -> Result<(), BackendError> {
        Err(BackendError::UnsupportedFeature {
            name: "resident buffer ranged batch upload".to_string(),
            backend: self.id().to_string(),
        })
    }

    /// Download a backend-resident resource into a new host buffer.
    ///
    /// Prefer [`VyreBackend::download_resident_into`] in hot loops so repeated
    /// validation does not allocate a fresh `Vec` for every readback.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot download resident
    /// resources or when `resource` is not owned by this backend.
    fn download_resident(&self, resource: &Resource) -> Result<Vec<u8>, BackendError> {
        let mut bytes = Vec::new();
        self.download_resident_into(resource, &mut bytes)?;
        Ok(bytes)
    }

    /// Download a backend-resident resource into caller-owned storage.
    ///
    /// Implementations must clear and reuse `out`; hidden compatibility
    /// allocation defeats resident hot-loop validation.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when resident download is unsupported or when
    /// `resource` is not owned by this backend.
    fn download_resident_into(
        &self,
        _resource: &Resource,
        _out: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        Err(BackendError::UnsupportedFeature {
            name: "resident buffer download".to_string(),
            backend: self.id().to_string(),
        })
    }

    /// Download a byte range from a backend-resident resource into a new host
    /// buffer.
    ///
    /// Prefer [`VyreBackend::download_resident_range_into`] in hot loops.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when resident ranged download is unsupported,
    /// the range is invalid, or `resource` is not owned by this backend.
    fn download_resident_range(
        &self,
        resource: &Resource,
        byte_offset: usize,
        byte_len: usize,
    ) -> Result<Vec<u8>, BackendError> {
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(byte_len).map_err(|error| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: resident ranged download could not reserve {byte_len} output byte(s): {error}. Split the readback range before dispatch."
                ),
            }
        })?;
        self.download_resident_range_into(resource, byte_offset, byte_len, &mut bytes)?;
        Ok(bytes)
    }

    /// Download a byte range from a backend-resident resource into
    /// caller-owned storage.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when resident ranged download is unsupported,
    /// the range is invalid, or `resource` is not owned by this backend.
    fn download_resident_range_into(
        &self,
        _resource: &Resource,
        _byte_offset: usize,
        _byte_len: usize,
        _out: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        Err(BackendError::UnsupportedFeature {
            name: "resident buffer ranged download".to_string(),
            backend: self.id().to_string(),
        })
    }

    /// Download several byte ranges from backend-resident resources into
    /// caller-owned storage as one logical readback operation.
    ///
    /// Backends with a real multi-readback path should override this to issue
    /// all copies behind one backend synchronization boundary.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when counts do not match, any range is invalid,
    /// or resident ranged download is unsupported.
    fn download_resident_ranges_into(
        &self,
        ranges: &[(&Resource, usize, usize)],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
        if ranges.len() != outputs.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: resident ranged batch download expected matching range/output counts but got {} range(s) and {} output(s).",
                    ranges.len(),
                    outputs.len()
                ),
            });
        }
        for ((resource, byte_offset, byte_len), output) in ranges.iter().zip(outputs.iter_mut()) {
            self.download_resident_range_into(resource, *byte_offset, *byte_len, output)?;
        }
        Ok(())
    }

    /// Free a backend-resident resource previously returned by
    /// [`VyreBackend::allocate_resident`].
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the resource is unknown or still in use.
    fn free_resident(&self, _resource: Resource) -> Result<(), BackendError> {
        Err(BackendError::UnsupportedFeature {
            name: "resident buffer free".to_string(),
            backend: self.id().to_string(),
        })
    }

    /// Dispatch using backend-resident resources and return backend-owned
    /// timing.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend does not support resident
    /// dispatch or any resource is invalid for the program.
    fn dispatch_resident_timed(
        &self,
        _program: &Program,
        _resources: &[Resource],
        _config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        Err(BackendError::UnsupportedFeature {
            name: "resident timed dispatch".to_string(),
            backend: self.id().to_string(),
        })
    }

    /// Dispatch an ordered sequence of resident-buffer programs and read
    /// selected resident byte ranges into caller-owned storage.
    ///
    /// The default preserves correctness by dispatching each step through
    /// [`VyreBackend::dispatch_resident_timed`] and then calling
    /// [`VyreBackend::download_resident_ranges_into`]. CUDA overrides this to
    /// enqueue the whole dependent chain plus D2H readbacks on one stream and
    /// pay one host synchronization point.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when any step fails, when readback ranges are
    /// invalid, or when the backend cannot perform resident dispatch/readback.
    fn dispatch_resident_sequence_read_ranges_into(
        &self,
        steps: &[ResidentDispatchStep<'_>],
        read_ranges: &[ResidentReadRange<'_>],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
        for step in steps {
            let mut config = DispatchConfig::default();
            config.grid_override = step.grid_override;
            self.dispatch_resident_timed(step.program, step.resources, &config)?;
        }
        let ranges = read_ranges
            .iter()
            .map(|range| (range.resource, range.byte_offset, range.byte_len))
            .collect::<SmallVec<[_; 8]>>();
        self.download_resident_ranges_into(&ranges, outputs)
    }

    /// Timed variant of
    /// [`VyreBackend::dispatch_resident_sequence_read_ranges_into`].
    ///
    /// The default preserves correctness by summing each step's resident
    /// dispatch timing and then downloading the requested ranges. Backends with
    /// a fused resident sequence path should override this to keep their
    /// optimized stream/queue behavior while exposing device timing.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when any step fails, when readback ranges are
    /// invalid, or when resident sequence timing overflows.
    fn dispatch_resident_sequence_read_ranges_timed_into(
        &self,
        steps: &[ResidentDispatchStep<'_>],
        read_ranges: &[ResidentReadRange<'_>],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<ResidentSequenceTiming, BackendError> {
        let started = std::time::Instant::now();
        let mut device_ns = Some(0_u64);
        let mut enqueue_ns = Some(0_u64);
        let mut wait_ns = Some(0_u64);
        for step in steps {
            let mut config = DispatchConfig::default();
            config.grid_override = step.grid_override;
            let timed = self.dispatch_resident_timed(step.program, step.resources, &config)?;
            device_ns = sum_optional_timing(device_ns, timed.device_ns, "device timing")?;
            enqueue_ns = sum_optional_timing(enqueue_ns, timed.enqueue_ns, "enqueue timing")?;
            wait_ns = sum_optional_timing(wait_ns, timed.wait_ns, "wait timing")?;
        }
        let ranges = read_ranges
            .iter()
            .map(|range| (range.resource, range.byte_offset, range.byte_len))
            .collect::<SmallVec<[_; 8]>>();
        self.download_resident_ranges_into(&ranges, outputs)?;
        Ok(ResidentSequenceTiming {
            wall_ns: elapsed_resident_sequence_wall_ns(started)?,
            device_ns,
            enqueue_ns,
            wait_ns,
        })
    }

    /// Dispatch a resident prefix, repeat a resident sub-sequence, and read
    /// selected resident byte ranges into caller-owned storage.
    ///
    /// This is the fixed-point hot-path contract: dataflow clients can express
    /// a repeated kernel group without allocating one host sequence entry per
    /// iteration. Backends that understand repetition should override this to
    /// keep launch preparation and parameter upload sublinear in
    /// `repeat_count`.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when any step fails, when readback ranges are
    /// invalid, or when the backend cannot perform resident dispatch/readback.
    fn dispatch_resident_repeated_sequence_read_ranges_into(
        &self,
        prefix_steps: &[ResidentDispatchStep<'_>],
        repeated_steps: &[ResidentDispatchStep<'_>],
        repeat_count: u32,
        read_ranges: &[ResidentReadRange<'_>],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
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
        self.download_resident_ranges_into(&ranges, outputs)
    }

    /// Optional pre-compilation hook for the pipeline-mode API.
    ///
    /// Default returns `Ok(None)`  -  the framework wraps in a passthrough
    /// pipeline whose `dispatch` calls back into [`VyreBackend::dispatch`]
    /// every time. Backends that genuinely cache compiled state (compute
    /// pipeline, bind-group layout, lowered shader text) override this and
    /// return `Ok(Some(...))` so repeated dispatches skip the compilation
    /// overhead.
    ///
    /// The returned pipeline MUST be bit-identical to repeated
    /// `dispatch(program, inputs, config)` for the program it was compiled
    /// from. The cache key is the backend's responsibility  -  the framework
    /// does not deduplicate compile calls.
    ///
    /// Implementing this method is the P-6 contract from
    /// `docs/audits/ROADMAP_PERFORMANCE.md`: "compile target-text + pipeline +
    /// bind-group-layout once; dispatch repeatedly with different inputs."
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete the
    /// pre-compilation. Callers should treat this as fatal for the program
    /// (the program will not dispatch successfully via any path).
    fn compile_native(
        &self,
        _program: &Program,
        _config: &DispatchConfig,
    ) -> Result<Option<Arc<dyn CompiledPipeline>>, BackendError> {
        Ok(None)
    }

    /// Optional pre-compilation hook for callers that already own a shared
    /// program allocation.
    ///
    /// Backends that store the program inside the compiled pipeline should
    /// override this method and keep the supplied [`Arc<Program>`] instead of
    /// cloning the IR. The default preserves the older borrowed-program hook
    /// for backends that only inspect the program while compiling.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when backend-native compilation fails.
    fn compile_native_shared(
        &self,
        program: Arc<Program>,
        config: &DispatchConfig,
    ) -> Result<Option<Arc<dyn CompiledPipeline>>, BackendError> {
        self.compile_native(&program, config)
    }

    /// Optional compiled-pipeline cache counters for compile telemetry.
    ///
    /// Return `None` unless the backend can report real cache hits and misses.
    fn pipeline_cache_snapshot(&self) -> Option<crate::pipeline::PipelineCacheSnapshot> {
        None
    }

    /// Optional backend-specific numeric telemetry for release evidence.
    ///
    /// Return an empty vector unless the backend can report real counters.
    /// Metric names must be stable ASCII identifiers suitable for JSON and
    /// Prometheus-style export.
    fn backend_metric_snapshot(&self) -> Vec<(&'static str, u64)> {
        Vec::new()
    }

    /// Non-blocking dispatch primitive.
    ///
    /// Returns a [`PendingDispatch`] handle immediately; the caller
    /// polls via [`PendingDispatch::is_ready`] and consumes the result
    /// via [`PendingDispatch::await_result`]. Backends that genuinely
    /// pipeline dispatches override this so N concurrent dispatches
    /// do not serialize on the host.
    ///
    /// Default: run the synchronous [`VyreBackend::dispatch`] path and
    /// wrap the result in a trivially-ready handle. This keeps every
    /// backend useful from the async API without forcing an async
    /// rewrite.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if the dispatch cannot start. Errors
    /// that surface only during GPU execution come back through
    /// [`PendingDispatch::await_result`], not from this call.
    fn dispatch_async(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Box<dyn PendingDispatch>, BackendError> {
        let outputs = self.dispatch(program, inputs, config)?;
        Ok(Box::new(crate::backend::pending_dispatch::ReadyPending {
            outputs,
        }))
    }

    /// Non-blocking dispatch with borrowed input buffers.
    ///
    /// Backends that record GPU commands synchronously before returning can
    /// override this to avoid cloning input buffers just to create a pending
    /// handle. The returned [`PendingDispatch`] must not borrow from `inputs`.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if the dispatch cannot start.
    fn dispatch_borrowed_async(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Box<dyn PendingDispatch>, BackendError> {
        let outputs = self.dispatch_borrowed(program, inputs, config)?;
        Ok(Box::new(crate::backend::pending_dispatch::ReadyPending {
            outputs,
        }))
    }

    // ---------------------------------------------------------------
    // Capability queries (all default to conservative "no" / minimal).
    //
    // These are the stable capability surface. Additional backends implement
    // this trait by default-inheriting every capability below and OVERRIDING
    // only the ones where they are more capable than the conservative floor.
    // This means adding a backend is strictly additive  -  no existing
    // backend impl has to change when a new capability query is added.
    //
    // Backends MUST report HONESTLY. Returning `true` from a capability
    // query is a promise the lowering path emits the corresponding
    // intrinsic and the adapter supports it. "Supported but broken" is a
    // LAW 9 evasion (see AGENTS.md). If the feature bit is set on the
    // device but the lowering emits a slower emulation sequence, the
    // answer is `false` until the native lowering catches up.
    // ---------------------------------------------------------------

    /// Whether this backend's lowering path emits subgroup / wave
    /// intrinsics AND the current adapter exposes them.
    ///
    /// Default: `false` (conservative  -  assumes no native subgroup lowering).
    #[must_use]
    fn supports_subgroup_ops(&self) -> bool {
        false
    }

    /// Whether this backend lowers IEEE 754 binary16 (`DataType::F16`)
    /// natively rather than emulating through `f32`.
    ///
    /// Default: `false`.
    #[must_use]
    fn supports_f16(&self) -> bool {
        false
    }

    /// Whether this backend lowers bfloat16 (`DataType::BF16`) natively.
    ///
    /// Default: `false`.
    #[must_use]
    fn supports_bf16(&self) -> bool {
        false
    }

    /// Whether this backend emits tensor-core / matrix-engine intrinsics
    /// for supported tensor shapes.
    ///
    /// Default: `false`.
    #[must_use]
    fn supports_tensor_cores(&self) -> bool {
        false
    }

    /// Whether this backend overlaps copies and compute via independent
    /// queues or async engines.
    ///
    /// Default: `false` (host serializes copy ↔ compute).
    #[must_use]
    fn supports_async_compute(&self) -> bool {
        false
    }

    /// Whether this backend supports indirect dispatch
    /// (`Node::IndirectDispatch`).
    ///
    /// Default: `false`.
    #[must_use]
    fn supports_indirect_dispatch(&self) -> bool {
        false
    }

    /// Whether this backend supports speculative dispatch  -  a fused
    /// prefilter + confirmer kernel with commit-gated output and a
    /// counter tail read back by the host.
    ///
    /// Default: `false`.
    #[must_use]
    fn supports_speculation(&self) -> bool {
        false
    }

    /// Whether this backend supports device-side persistent-thread
    /// dispatch (a long-running kernel that polls a work queue).
    ///
    /// Default: `false`.
    #[must_use]
    fn supports_persistent_thread_dispatch(&self) -> bool {
        false
    }

    /// Whether this backend can satisfy `Node::Barrier { ordering:
    /// MemoryOrdering::GridSync }` inside a single dispatch  -  i.e.
    /// every thread in the entire grid waits at the barrier and
    /// every prior write is globally visible afterwards. Backends
    /// that lack a native grid barrier (workgroup-only fences) must
    /// return `false`; registration-based dispatch may lower a
    /// `GridSync` barrier to a host-orchestrated kernel split only
    /// when [`VyreBackend::allows_host_grid_sync_split`] also returns
    /// `true`.
    ///
    /// Backends with cooperative whole-grid launch support can return
    /// `true`; backends limited to workgroup-local synchronization return
    /// `false` until the target exposes a compatible grid-barrier primitive.
    ///
    /// Default: `false`.
    #[must_use]
    fn supports_grid_sync(&self) -> bool {
        false
    }

    /// Whether a native cooperative grid-sync launch of `program` with these
    /// `inputs` and `config` can be made fully resident on this device.
    ///
    /// [`VyreBackend::supports_grid_sync`] reports that native lowering is
    /// *available*; this reports whether it *fits* for a specific dispatch. A
    /// cooperative launch requires every block co-resident, so a grid whose
    /// block count exceeds the device's cooperative residency cannot run
    /// natively and must route to the resident-fixpoint or host-split path.
    /// Orchestrators call this to choose the native route only when it fits,
    /// avoiding a wasted allocate/upload that would otherwise end in
    /// [`crate::ErrorCode::CooperativeResidencyExceeded`].
    ///
    /// Default: `Ok(false)` (no native cooperative launch). Backends that lower
    /// grid sync override this with the real residency check. Returns `Ok(false)`
    /// — not an error — when the program carries no grid-sync barrier, since
    /// there is then nothing to launch cooperatively.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if the launch geometry cannot be computed for
    /// the program/inputs (a structurally invalid dispatch).
    fn cooperative_grid_sync_fits(
        &self,
        _program: &Program,
        _inputs: &[&[u8]],
        _config: &DispatchConfig,
    ) -> Result<bool, BackendError> {
        Ok(false)
    }

    /// Whether the shared registry wrapper may emulate whole-grid
    /// synchronization for this backend by splitting one program into
    /// multiple host-dispatched kernels.
    ///
    /// This exists separately from [`VyreBackend::supports_grid_sync`]
    /// because a backend can intentionally reject hidden host
    /// orchestration while native cooperative-grid lowering is absent.
    /// CUDA uses that policy in the release path so missing native
    /// grid-barrier lowering is surfaced as an unsupported feature
    /// instead of silently becoming a slower multi-launch path.
    ///
    /// Default: `true` to preserve existing behavior for simple
    /// backends that intentionally rely on shared split lowering.
    #[must_use]
    fn allows_host_grid_sync_split(&self) -> bool {
        true
    }

    /// Whether this backend implements the resident half of the contract
    /// (`allocate_resident` / `upload_resident` / `dispatch_resident_timed` /
    /// `dispatch_resident_repeated_sequence_read_ranges_into` /
    /// `download_resident_*` / `free_resident`) well enough to run a
    /// device-resident dispatch sequence.
    ///
    /// Consumers use this to choose
    /// [`crate::grid_sync::dispatch_resident_grid_sync_fixpoint_into`] (which
    /// keeps live buffers device-resident across every grid-sync segment and
    /// fixpoint pass) over the host-orchestrated
    /// [`crate::grid_sync::dispatch_with_grid_sync_split_into`] (which
    /// round-trips every live buffer host↔device between segments). Both are
    /// correct; the choice is a performance route on a probed capability, not
    /// a silent failure fallback.
    ///
    /// Default: `false`. Backends that implement resident dispatch override
    /// this to `true`.
    #[must_use]
    fn supports_resident_dispatch(&self) -> bool {
        false
    }

    /// Whether this backend partitions a program across more than one
    /// physical device / node.
    ///
    /// Default: `false` (single-device execution).
    #[must_use]
    fn is_distributed(&self) -> bool {
        false
    }

    /// Whether this backend lowers distributed collective communication
    /// nodes (`AllReduce`, `AllGather`, `ReduceScatter`, `Broadcast`).
    ///
    /// This is intentionally separate from [`VyreBackend::is_distributed`]:
    /// a backend may partition work across devices without yet exposing a
    /// correct collective transport/lowering stack. Default: `false`.
    #[must_use]
    fn supports_distributed_collectives(&self) -> bool {
        false
    }

    /// Maximum supported workgroup size per axis `[x, y, z]`.
    ///
    /// Default: `[1, 1, 1]` (scalar dispatch  -  a backend that has not
    /// reported a real limit cannot be trusted to execute parallel
    /// workgroups).
    #[must_use]
    fn max_workgroup_size(&self) -> [u32; 3] {
        [1, 1, 1]
    }

    /// Maximum number of compute workgroups the backend can launch in one
    /// dispatch dimension.
    ///
    /// Default: `1`, which is safe for scalar/reference backends but must be
    /// overridden by real GPU backends so schedulers do not under-launch.
    #[must_use]
    fn max_compute_workgroups_per_dimension(&self) -> u32 {
        1
    }

    /// Maximum total invocations allowed in a single workgroup.
    ///
    /// Default derives from [`max_workgroup_size`](Self::max_workgroup_size)
    /// and fails loudly if a backend reports an unrepresentable product.
    #[must_use]
    fn max_compute_invocations_per_workgroup(&self) -> u32 {
        let [x, y, z] = self.max_workgroup_size();
        let invocations = u128::from(x) * u128::from(y) * u128::from(z);
        u32::try_from(invocations).unwrap_or(u32::MAX)
    }

    /// Native subgroup size for the backing device when the backend
    /// knows it. Returning
    /// `None` tells the dispatch planner the backend can't report a
    /// subgroup width  -  the planner falls back to `max_workgroup_size`
    /// for its sizing heuristic.
    ///
    /// I.6  -  adaptive workgroup sizing reads this capability to pick
    /// a workgroup multiple of the subgroup so threads don't straddle
    /// subgroups. Typical devices expose 16, 32, or 64 lanes.
    #[must_use]
    fn subgroup_size(&self) -> Option<u32> {
        None
    }

    /// Maximum size in bytes of a single storage buffer the backend
    /// accepts. `0` means the backend has not reported a limit, not
    /// "unlimited".
    ///
    /// Default: `0`.
    #[must_use]
    fn max_storage_buffer_bytes(&self) -> u64 {
        0
    }

    /// Unified backend-neutral device profile.
    ///
    /// Shared planner code should prefer this single profile over reading
    /// individual capability methods one by one. Concrete backends may
    /// override it when they can report richer device facts such as shared
    /// memory size or native lowering-strategy features.
    #[must_use]
    fn device_profile(&self) -> crate::DeviceProfile {
        let max_workgroup_size = self.max_workgroup_size();
        crate::DeviceProfile {
            backend: self.id(),
            supports_subgroup_ops: self.supports_subgroup_ops(),
            supports_indirect_dispatch: self.supports_indirect_dispatch(),
            supports_distributed_collectives: self.supports_distributed_collectives(),
            supports_specialization_constants: false,
            supports_f16: self.supports_f16(),
            supports_bf16: self.supports_bf16(),
            supports_trap_propagation: false,
            supports_tensor_cores: self.supports_tensor_cores(),
            has_mul_high: false,
            has_dual_issue_fp32_int32: false,
            has_subgroup_shuffle: self.supports_subgroup_ops(),
            has_shared_memory: false,
            max_native_int_width: 32,
            max_workgroup_size,
            max_invocations_per_workgroup: self.max_compute_invocations_per_workgroup(),
            max_shared_memory_bytes: 0,
            max_storage_buffer_binding_size: self.max_storage_buffer_bytes(),
            subgroup_size: self.subgroup_size().unwrap_or(0),
            compute_units: 0,
            regs_per_thread_max: 0,
            l1_cache_bytes: 0,
            l2_cache_bytes: 0,
            mem_bw_gbps: 0,
            timing_quality: crate::DeviceTimingQuality::HostOnly,
            supports_device_timestamps: false,
            supports_hardware_counters: false,
            ideal_unroll_depth: 0,
            ideal_vector_pack_bits: 0,
            ideal_workgroup_tile: [0, 0, 0],
            shared_memory_bank_count: 0,
            shared_memory_bank_width_bytes: 0,
        }
    }

    // ---------------------------------------------------------------
    // Lifecycle hooks (defaulted, override as needed).
    //
    // These let a backend warm caches, flush pending work, recover from
    // device loss, or tear down cleanly. Every hook defaults to a
    // no-op-or-structured-error, so existing impls do not have to add
    // any code.
    // ---------------------------------------------------------------

    /// Pre-dispatch warmup. Called before the first dispatch on a new
    /// program so the backend can warm caches, compile ahead-of-time, or
    /// acquire a device handle without paying that cost on the hot path.
    ///
    /// Default: no-op `Ok(())`.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if warmup cannot complete.
    fn prepare(&self) -> Result<(), BackendError> {
        Ok(())
    }

    /// Flush any queued work to the device and wait for it to complete.
    ///
    /// Useful before tearing down a context or before reading back data
    /// that was produced by the last asynchronous dispatch.
    ///
    /// Default: no-op `Ok(())`  -  backends that do not queue work
    /// implicitly satisfy flush.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] on device failure.
    fn flush(&self) -> Result<(), BackendError> {
        Ok(())
    }

    /// Release device resources held by this backend. After `shutdown`
    /// returns the backend is in an unspecified state and may not be
    /// used for further dispatches.
    ///
    /// Default: no-op `Ok(())`.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] on device failure during teardown.
    fn shutdown(&self) -> Result<(), BackendError> {
        Ok(())
    }

    /// Probe whether the underlying device has been lost since the last
    /// successful dispatch.
    ///
    /// Default: `false` (assume healthy  -  backends that have no
    /// device-loss story do not need to probe).
    #[must_use]
    fn device_lost(&self) -> bool {
        false
    }

    /// Attempt to recover from device loss by reacquiring the underlying
    /// device and invalidating pipeline caches.
    ///
    /// Default: returns an `UnsupportedFeature` error  -  recovery must be
    /// opt-in, because a backend that silently re-acquires without
    /// notifying the caller is a correctness hazard.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError::UnsupportedFeature`] by default. Backends
    /// that implement recovery return any error encountered during
    /// re-acquisition.
    fn try_recover(&self) -> Result<(), BackendError> {
        Err(BackendError::UnsupportedFeature {
            name: "device recovery".to_string(),
            backend: self.id().to_string(),
        })
    }

    /// Allocate a backend-owned device buffer of `byte_len` bytes.
    ///
    /// The returned [`DeviceBuffer`] handle is only meaningful to the
    /// backend that produced it. Backends that have not opted in return
    /// [`BackendError::UnsupportedFeature`] with the
    /// `DEVICE_BUFFER_FEATURE` name; production callers that require
    /// resident-buffer performance must treat that as a hard capability
    /// failure rather than silently routing through host `Vec<u8>`
    /// dispatch. Real device backends (cuda/wgpu/spirv) override this to
    /// wrap their concrete handle (for example, a vendor device allocation,
    /// vulkan buffer) in a `DeviceBuffer` impl.
    ///
    /// See `crate::backend::device_buffer` for the substrate.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError::UnsupportedFeature`] when the backend
    /// has not yet implemented persistent device-buffer allocation.
    fn allocate_device_buffer(
        &self,
        _byte_len: usize,
    ) -> Result<Box<dyn DeviceBuffer>, BackendError> {
        Err(unsupported_device_buffer(self.id()))
    }

    /// Upload host bytes into a previously-allocated device buffer.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the buffer was not allocated by this
    /// backend, the byte length does not match the allocation, or the
    /// backend has not opted in to device-buffer dispatch.
    fn upload_device_buffer(
        &self,
        _buffer: &mut dyn DeviceBuffer,
        _bytes: &[u8],
    ) -> Result<(), BackendError> {
        Err(unsupported_device_buffer(self.id()))
    }

    /// Download bytes from a device buffer back to a host `Vec<u8>`.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the buffer was not allocated by this
    /// backend or the backend has not opted in to device-buffer dispatch.
    fn download_device_buffer(&self, _buffer: &dyn DeviceBuffer) -> Result<Vec<u8>, BackendError> {
        Err(unsupported_device_buffer(self.id()))
    }

    /// Free a device buffer previously returned by
    /// [`Self::allocate_device_buffer`]. Explicit-free is required
    /// because the substrate does not assume reference-counted backend
    /// handles; consumers are responsible for calling this when done.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the buffer was not allocated by this
    /// backend or the underlying free fails.
    fn free_device_buffer(&self, _buffer: Box<dyn DeviceBuffer>) -> Result<(), BackendError> {
        Err(unsupported_device_buffer(self.id()))
    }

    /// Dispatch a Program with backend-owned device buffers as inputs and
    /// outputs.
    ///
    /// Backends that have implemented [`Self::allocate_device_buffer`]
    /// override this method to bind their concrete buffer type without
    /// the host upload/download round trip the legacy `dispatch` API
    /// requires. Backends that have not opted in return
    /// [`BackendError::UnsupportedFeature`]. Callers that choose this
    /// API are asking for resident-buffer execution; falling back to
    /// host-buffer dispatch would hide the copy cost and violate the
    /// performance contract.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError::UnsupportedFeature`] by default. Real
    /// implementations return any error encountered during dispatch.
    fn dispatch_with_device_buffers(
        &self,
        _program: &Program,
        _inputs: &[&dyn DeviceBuffer],
        _outputs: &mut [&mut dyn DeviceBuffer],
        _config: &DispatchConfig,
    ) -> Result<(), BackendError> {
        Err(unsupported_device_buffer(self.id()))
    }
}

fn elapsed_resident_sequence_wall_ns(started: std::time::Instant) -> Result<u64, BackendError> {
    u64::try_from(started.elapsed().as_nanos()).map_err(|error| BackendError::InvalidProgram {
        fix: format!(
            "Fix: resident sequence wall timing cannot fit u64 nanoseconds: {error}. Split telemetry windows or report per-step timing."
        ),
    })
}

fn sum_optional_timing(
    accumulator: Option<u64>,
    next: Option<u64>,
    field: &'static str,
) -> Result<Option<u64>, BackendError> {
    match (accumulator, next) {
        (Some(left), Some(right)) => Ok(Some(left.checked_add(right).ok_or_else(|| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: resident sequence {field} overflowed u64 nanoseconds. Split telemetry windows or report per-step timing instead of silently clamping."
                ),
            }
        })?)),
        _ => Ok(None),
    }
}

#[cfg(test)]

mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TelemetryBackend;

    impl private::Sealed for TelemetryBackend {}

    impl VyreBackend for TelemetryBackend {
        fn id(&self) -> &'static str {
            "telemetry-test"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Ok(vec![vec![1, 2], vec![3, 4]])
        }
    }

    struct SequenceTimingBackend {
        dispatches: AtomicUsize,
    }

    impl private::Sealed for SequenceTimingBackend {}

    impl VyreBackend for SequenceTimingBackend {
        fn id(&self) -> &'static str {
            "sequence-timing-test"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Ok(Vec::new())
        }

        fn dispatch_resident_timed(
            &self,
            _program: &Program,
            _resources: &[Resource],
            config: &DispatchConfig,
        ) -> Result<TimedDispatchResult, BackendError> {
            let index = self.dispatches.fetch_add(1, Ordering::SeqCst) as u64;
            assert_eq!(
                config.grid_override,
                Some([index as u32 + 1, 1, 1]),
                "Fix: default resident sequence timing must preserve each step's grid override."
            );
            Ok(TimedDispatchResult {
                outputs: Vec::new(),
                wall_ns: 10 + index,
                device_ns: Some(7 + index),
                enqueue_ns: Some(3 + index),
                wait_ns: Some(4 + index),
            })
        }

        fn download_resident_ranges_into(
            &self,
            ranges: &[(&Resource, usize, usize)],
            outputs: &mut [&mut Vec<u8>],
        ) -> Result<(), BackendError> {
            assert_eq!(ranges.len(), outputs.len());
            for ((resource, offset, len), output) in ranges.iter().zip(outputs.iter_mut()) {
                let Resource::Resident(id) = resource else {
                    panic!("Fix: default timed resident sequence test expects resident resources.");
                };
                output.clear();
                output.extend_from_slice(&id.to_le_bytes());
                output.extend_from_slice(&(*offset as u64).to_le_bytes());
                output.extend_from_slice(&(*len as u64).to_le_bytes());
            }
            Ok(())
        }
    }

    #[test]
    fn default_borrowed_into_dispatch_records_runtime_telemetry() {
        let _guard = crate::observability::audit_events_test_lock();
        let before = crate::observability::snapshot_dispatch_telemetry();
        let backend = TelemetryBackend;
        let mut outputs = vec![Vec::with_capacity(4), Vec::with_capacity(1)];

        backend
            .dispatch_borrowed_into(
                &Program::empty(),
                &[&[9, 8, 7]],
                &DispatchConfig::default(),
                &mut outputs,
            )
            .expect("Fix: default borrowed-into dispatch must succeed");

        let telemetry = crate::observability::snapshot_dispatch_telemetry();
        assert!(telemetry.launches >= before.launches + 1);
        assert!(telemetry.input_bytes >= before.input_bytes + 3);
        assert!(telemetry.output_bytes >= before.output_bytes + 4);
        assert!(telemetry.output_slots >= before.output_slots + 2);
        assert!(telemetry.output_slots_reused >= before.output_slots_reused + 1);
        assert!(telemetry.output_slots_moved >= before.output_slots_moved + 1);
        assert!(telemetry.output_slots_appended >= before.output_slots_appended);
    }

    #[test]
    fn default_resident_sequence_timing_sums_step_device_times_and_reads_ranges() {
        let backend = SequenceTimingBackend {
            dispatches: AtomicUsize::new(0),
        };
        let program = Program::empty();
        let first_resources = [Resource::Resident(11)];
        let second_resources = [Resource::Resident(22)];
        let steps = [
            ResidentDispatchStep {
                program: &program,
                resources: &first_resources,
                grid_override: Some([1, 1, 1]),
                workgroup_override: None,
            },
            ResidentDispatchStep {
                program: &program,
                resources: &second_resources,
                grid_override: Some([2, 1, 1]),
                workgroup_override: None,
            },
        ];
        let read_resource = Resource::Resident(33);
        let reads = [ResidentReadRange {
            resource: &read_resource,
            byte_offset: 4,
            byte_len: 8,
        }];
        let mut output = Vec::new();

        let timing = backend
            .dispatch_resident_sequence_read_ranges_timed_into(&steps, &reads, &mut [&mut output])
            .expect("Fix: default timed resident sequence must execute and read ranges.");

        assert_eq!(backend.dispatches.load(Ordering::SeqCst), 2);
        assert_eq!(timing.device_ns, Some(15));
        assert_eq!(timing.enqueue_ns, Some(7));
        assert_eq!(timing.wait_ns, Some(9));
        assert!(timing.wall_ns > 0);
        assert_eq!(output.len(), 24);
        assert_eq!(u64::from_le_bytes(output[0..8].try_into().unwrap()), 33);
        assert_eq!(u64::from_le_bytes(output[8..16].try_into().unwrap()), 4);
        assert_eq!(u64::from_le_bytes(output[16..24].try_into().unwrap()), 8);
    }

    #[test]
    fn default_dispatch_paths_use_shared_fallible_staging_and_checked_timing() {
        let backend_source = include_str!("vyre_backend.rs");
        let compiled_source = include_str!("compiled_pipeline.rs");
        let module_source = include_str!("../backend.rs");

        assert!(
            module_source.contains("fn clone_borrowed_inputs_for_dispatch")
                && module_source.contains("fn reserve_batch_output_slots")
                && module_source.contains("fn checked_elapsed_wall_ns"),
            "Fix: backend defaults must share one fallible staging and checked timing contract."
        );
        for source in [backend_source, compiled_source] {
            let production = source
                .split("#[cfg(test)]")
                .next()
                .expect("Fix: backend source must contain production section before tests");
            assert!(
                production.contains("clone_borrowed_inputs_for_dispatch")
                    && production.contains("checked_elapsed_wall_ns")
                    && !production.contains(".as_nanos() as u64")
                    && !production.contains("inputs.iter().map(|input| (*input).to_vec()).collect()"),
                "Fix: inherited backend dispatch defaults must avoid infallible borrowed-input collection and lossy wall timing."
            );
        }
        let compiled_production = compiled_source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: compiled pipeline source must contain production section before tests");
        assert!(
            compiled_production.contains("reserved_batch_output_slots")
                && !compiled_production.contains("Vec::with_capacity(batches.len())"),
            "Fix: compiled-pipeline batch defaults must construct output slots through shared fallible staging."
        );
    }
}
