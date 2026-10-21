//! Dispatcher trait  -  the seam between the self-hosted optimizer and
//! a backend that can actually run vyre Programs.
//!
//! The optimizer encodes the user's Program into ProgramGraph buffers,
//! builds a vyre Program that does the analysis (e.g. `persistent_bfs`),
//! and asks an `OptimizerDispatcher` to run that analysis Program. The
//! returned bytes drive the rewrite.
//!
//! `vyre-self-substrate` cannot depend on a concrete backend  -  it sits
//! below the driver layer. The trait inverts that dependency: the
//! orchestrator code stays in self-substrate, and a backend crate
//! (e.g. `vyre-driver-wgpu` or a runtime wrapper) provides the impl.
//!
//! Test code in this crate uses `oracle::CpuOracleDispatcher` so the
//! encoder can be proven sound against the existing primitive oracles
//! before any GPU backend is wired. The CPU oracle is gated to tests
//! only  -  it is never on a production code path.

use vyre_foundation::ir::Program;

/// One resident-buffer kernel launch in an ordered optimizer sequence.
pub struct ResidentDispatchStep<'a> {
    /// Program to launch.
    pub program: &'a Program,
    /// Resident handle ids in canonical buffer binding order.
    pub handle_ids: &'a [u64],
    /// Optional launch grid override.
    pub grid_override: Option<[u32; 3]>,
}

/// One byte range to read from a resident buffer after an ordered sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentReadRange {
    /// Resident handle id.
    pub handle_id: u64,
    /// First byte to read from the device buffer.
    pub byte_offset: usize,
    /// Number of meaningful bytes to transfer.
    pub byte_len: usize,
}

/// Resident handles for immutable payloads that may stay device-resident
/// across optimizer calls.
///
/// `retained_by_dispatcher` means the dispatcher owns the handles after the
/// caller is done with the current launch sequence. Call
/// [`OptimizerDispatcher::release_resident_static_uploads`] instead of
/// `free_resident` so CUDA can keep read-only graph/arena buffers hot while
/// portable dispatchers free them immediately.
#[derive(Debug)]
pub struct ResidentStaticBufferSet {
    /// Resident handle ids in the same order as the payload slice passed to
    /// `acquire_resident_static_uploads`.
    pub handles: Vec<u64>,
    /// True when the handles were already resident and no host upload was paid.
    pub cache_hit: bool,
    /// True when the dispatcher retained ownership for future reuse.
    pub retained_by_dispatcher: bool,
}

/// Errors a dispatcher may surface. Concrete backends compose their
/// own error types into this; the orchestrator only needs the
/// boundary message.
#[derive(Debug)]
pub enum DispatchError {
    /// The dispatcher rejected the Program. The string carries the
    /// backend's actionable message (must contain `Fix:`).
    Rejected(String),
    /// Input arity or shape did not match the Program's declared
    /// buffer set. Hard error  -  not retryable.
    BadInputs(String),
    /// Backend raised an internal error. Same shape as `Rejected` but
    /// the cause is in the backend, not the Program.
    BackendError(String),
}

impl std::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(msg) => write!(f, "dispatcher rejected program: {msg}"),
            Self::BadInputs(msg) => write!(f, "dispatcher input mismatch: {msg}"),
            Self::BackendError(msg) => write!(f, "dispatcher backend error: {msg}"),
        }
    }
}

impl std::error::Error for DispatchError {}

/// Run a vyre Program with byte inputs, return byte outputs in the
/// Program's declared output order.
///
/// This is the canonical dispatch boundary. Production impls go
/// through `vyre-driver-wgpu` or `vyre-driver-cuda`; test impls use
/// CPU oracles (gated to test-only builds).
pub trait OptimizerDispatcher {
    /// Dispatch `program` with the given byte inputs (one `Vec<u8>`
    /// per declared input buffer in canonical buffer order). Returns
    /// the declared outputs in the same canonical order.
    ///
    /// `grid_override` lets parallel kernels dispatch enough
    /// workgroups to cover their input. `None` means "use the
    /// backend's default grid" (typically `[1, 1, 1]`), which is what
    /// sequential single-thread Programs want. Parallel passes
    /// compute `Some([ceil(work/wg_x), 1, 1])` based on the input
    /// size and their declared workgroup_size.
    fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError>;

    /// Whether this dispatcher supports the persistent-resident path.
    /// Default: false. CUDA backend overrides to true. The orchestrator
    /// uses this to decide whether to take the persistent fast-path
    /// (encode arena once → upload once → dispatch many → readback once)
    /// or use the non-resident per-call GPU dispatch path.
    fn supports_persistent(&self) -> bool {
        false
    }

    /// Device/lowering feature bits that affect reusable plan identity.
    ///
    /// Backends with feature-dependent lowering must override this so
    /// self-substrate plan caches cannot replay a Program shape prepared for a
    /// different hardware/lowering capability set. Test-only and reference
    /// dispatchers keep the zero default because they do not specialize plans by
    /// device.
    fn device_feature_cache_key(&self) -> u64 {
        0
    }

    /// Allocate a backend-resident buffer. Returns an opaque u64
    /// handle. Callers must `free_resident` to release.
    fn alloc_resident(&self, _byte_len: usize) -> Result<u64, DispatchError> {
        Err(DispatchError::Rejected(
            "Fix: this dispatcher does not implement the persistent path; \
             use `dispatch` instead, or wire the resident-buffer methods."
                .to_string(),
        ))
    }

    /// Allocate a logical group of resident buffers and roll back partial state
    /// if any allocation fails.
    fn alloc_resident_many(&self, byte_lens: &[usize]) -> Result<Vec<u64>, DispatchError> {
        let mut handles = Vec::new();
        handles.try_reserve(byte_lens.len()).map_err(|error| {
            DispatchError::BackendError(format!(
                "Fix: reserve resident handle group before allocation; requested {} buffer(s): {error}.",
                byte_lens.len()
            ))
        })?;
        for (index, &byte_len) in byte_lens.iter().enumerate() {
            match self.alloc_resident(byte_len) {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    let allocation_error = error.to_string();
                    if let Err(free_error) = free_resident_handles(
                        self,
                        &handles,
                        "resident grouped allocation rollback",
                    ) {
                        return Err(DispatchError::BackendError(format!(
                            "Fix: resident grouped allocation failed at buffer {index} after {} partial allocation(s): {allocation_error}; rollback also failed: {free_error}.",
                            handles.len()
                        )));
                    }
                    return Err(error);
                }
            }
        }
        Ok(handles)
    }

    /// Upload host bytes into a resident buffer.
    fn upload_resident(&self, _handle: u64, _bytes: &[u8]) -> Result<(), DispatchError> {
        Err(DispatchError::Rejected(
            "Fix: dispatcher does not implement upload_resident.".to_string(),
        ))
    }

    /// Upload several resident buffers with one backend fence when supported.
    fn upload_resident_many(&self, uploads: &[(u64, &[u8])]) -> Result<(), DispatchError> {
        for &(handle, bytes) in uploads {
            self.upload_resident(handle, bytes)?;
        }
        Ok(())
    }

    /// Acquire resident handles for immutable payloads.
    ///
    /// Portable default behavior allocates and uploads exactly like
    /// `alloc_resident` + `upload_resident_many`, then returns
    /// `retained_by_dispatcher = false` so release frees the buffers. CUDA
    /// overrides this to content-address immutable optimizer buffers and skip
    /// H2D traffic on warmed identical programs.
    fn acquire_resident_static_uploads(
        &self,
        _cache_domain: u64,
        payloads: &[&[u8]],
    ) -> Result<ResidentStaticBufferSet, DispatchError> {
        let mut byte_lens = Vec::new();
        byte_lens.try_reserve(payloads.len()).map_err(|error| {
            DispatchError::BackendError(format!(
                "Fix: reserve resident static byte lengths before upload; requested {} payload(s): {error}.",
                payloads.len()
            ))
        })?;
        for payload in payloads {
            byte_lens.push(payload.len());
        }
        let handles = self.alloc_resident_many(&byte_lens)?;

        let mut uploads = Vec::new();
        uploads.try_reserve(payloads.len()).map_err(|error| {
            DispatchError::BackendError(format!(
                "Fix: reserve resident static upload storage before upload; requested {} payload(s): {error}.",
                payloads.len()
            ))
        })?;
        for (&handle, &payload) in handles.iter().zip(payloads.iter()) {
            uploads.push((handle, payload));
        }

        if let Err(error) = self.upload_resident_many(&uploads) {
            let upload_error = error.to_string();
            if let Err(free_error) =
                free_resident_handles(self, &handles, "resident static upload rollback")
            {
                return Err(DispatchError::BackendError(format!(
                    "Fix: resident static upload failed after allocating {} buffer(s): {upload_error}; rollback also failed: {free_error}.",
                    handles.len()
                )));
            }
            return Err(error);
        }

        Ok(ResidentStaticBufferSet {
            handles,
            cache_hit: false,
            retained_by_dispatcher: false,
        })
    }

    /// Release a static resident buffer set acquired from
    /// [`Self::acquire_resident_static_uploads`].
    fn release_resident_static_uploads(
        &self,
        set: ResidentStaticBufferSet,
    ) -> Result<(), DispatchError> {
        if set.retained_by_dispatcher {
            return Ok(());
        }
        for handle in set.handles {
            self.free_resident(handle)?;
        }
        Ok(())
    }

    /// Download a resident buffer's current contents to host bytes.
    fn read_resident(&self, _handle: u64) -> Result<Vec<u8>, DispatchError> {
        Err(DispatchError::Rejected(
            "Fix: dispatcher does not implement read_resident.".to_string(),
        ))
    }

    /// Download several resident buffers with one backend fence when supported.
    fn read_resident_many(&self, handles: &[u64]) -> Result<Vec<Vec<u8>>, DispatchError> {
        handles
            .iter()
            .map(|&handle| self.read_resident(handle))
            .collect()
    }

    /// Download selected byte ranges from resident buffers.
    fn read_resident_ranges(
        &self,
        ranges: &[ResidentReadRange],
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        let mut outputs = Vec::new();
        self.read_resident_ranges_into(ranges, &mut outputs)?;
        Ok(outputs)
    }

    /// Download selected byte ranges from resident buffers into caller-owned
    /// byte slots.
    fn read_resident_ranges_into(
        &self,
        ranges: &[ResidentReadRange],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        let mut unique_handles = Vec::new();
        unique_handles.try_reserve(ranges.len()).map_err(|error| {
            DispatchError::BackendError(format!(
                "Fix: reserve resident ranged-read handle dedupe storage before dispatch; requested {} range(s): {error}.",
                ranges.len()
            ))
        })?;
        let mut range_handle_indices = Vec::new();
        range_handle_indices
            .try_reserve(ranges.len())
            .map_err(|error| {
                DispatchError::BackendError(format!(
                    "Fix: reserve resident ranged-read index storage before dispatch; requested {} range(s): {error}.",
                    ranges.len()
                ))
            })?;
        for range in ranges {
            if let Some(index) = unique_handles
                .iter()
                .position(|&handle| handle == range.handle_id)
            {
                range_handle_indices.push(index);
            } else {
                let index = unique_handles.len();
                unique_handles.push(range.handle_id);
                range_handle_indices.push(index);
            }
        }
        let full_buffers = self.read_resident_many(&unique_handles)?;
        if full_buffers.len() != unique_handles.len() {
            return Err(DispatchError::BackendError(format!(
                "Fix: resident ranged-read batch returned {} buffer(s) for {} unique handle(s).",
                full_buffers.len(),
                unique_handles.len()
            )));
        }
        if outputs.len() < ranges.len() {
            outputs
                .try_reserve(ranges.len() - outputs.len())
                .map_err(|error| {
                    DispatchError::BackendError(format!(
                        "Fix: reserve resident ranged-read output storage before dispatch; requested {} range(s): {error}.",
                        ranges.len()
                    ))
                })?;
            outputs.resize_with(ranges.len(), Vec::new);
        } else {
            outputs.truncate(ranges.len());
        }
        for ((range, &buffer_index), output) in ranges
            .iter()
            .zip(range_handle_indices.iter())
            .zip(outputs.iter_mut())
        {
            let full = full_buffers.get(buffer_index).ok_or_else(|| {
                DispatchError::BackendError(format!(
                    "Fix: resident ranged-read handle index {buffer_index} missing from {} readback buffer(s).",
                    full_buffers.len()
                ))
            })?;
            let end = range
                .byte_offset
                .checked_add(range.byte_len)
                .ok_or_else(|| {
                    DispatchError::BadInputs(format!(
                    "Fix: resident read range for handle {} overflows usize at offset {} len {}.",
                    range.handle_id, range.byte_offset, range.byte_len
                ))
                })?;
            if end > full.len() {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: resident read range for handle {} requested bytes [{}..{}) but buffer readback has {} bytes.",
                    range.handle_id,
                    range.byte_offset,
                    end,
                    full.len()
                )));
            }
            output.clear();
            output.extend_from_slice(&full[range.byte_offset..end]);
        }
        Ok(())
    }

    /// Free a resident buffer previously returned by `alloc_resident`.
    fn free_resident(&self, _handle: u64) -> Result<(), DispatchError> {
        Err(DispatchError::Rejected(
            "Fix: dispatcher does not implement free_resident.".to_string(),
        ))
    }

    /// Dispatch a Program against resident-buffer handles. Each
    /// handle is referenced from the Program's declared buffer in the
    /// same canonical buffer order. RW buffers are not read back  -
    /// caller invokes `read_resident` once at end of pipeline.
    fn dispatch_resident(
        &self,
        _program: &Program,
        _handles: &[u64],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<(), DispatchError> {
        Err(DispatchError::Rejected(
            "Fix: dispatcher does not implement dispatch_resident.".to_string(),
        ))
    }

    /// Dispatch an ordered sequence of resident-buffer Programs.
    ///
    /// Default implementation preserves correctness by fencing each step
    /// through `dispatch_resident`. CUDA overrides this to enqueue the whole
    /// dependent chain on one stream and synchronize once.
    fn dispatch_resident_sequence(
        &self,
        steps: &[ResidentDispatchStep<'_>],
    ) -> Result<(), DispatchError> {
        for step in steps {
            self.dispatch_resident(step.program, step.handle_ids, step.grid_override)?;
        }
        Ok(())
    }

    /// Dispatch an ordered resident sequence and read selected resident buffers.
    ///
    /// Default implementation keeps the portable contract: execute the ordered
    /// sequence, then read buffers through `read_resident_many`. CUDA overrides
    /// this to enqueue the D2H readbacks behind the kernels on the same stream
    /// and pay one host fence.
    fn dispatch_resident_sequence_read_many(
        &self,
        steps: &[ResidentDispatchStep<'_>],
        read_handles: &[u64],
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        self.dispatch_resident_sequence(steps)?;
        self.read_resident_many(read_handles)
    }

    /// Dispatch an ordered resident sequence and read selected byte ranges.
    fn dispatch_resident_sequence_read_ranges(
        &self,
        steps: &[ResidentDispatchStep<'_>],
        read_ranges: &[ResidentReadRange],
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        self.dispatch_resident_sequence(steps)?;
        self.read_resident_ranges(read_ranges)
    }

    /// Upload resident buffers, dispatch an ordered resident sequence, then
    /// read selected resident buffers.
    ///
    /// Default implementation fences at each portable boundary. CUDA overrides
    /// this so H2D uploads, kernels, and D2H readbacks are ordered on one stream
    /// with one host synchronization point.
    fn upload_resident_many_sequence_read_many(
        &self,
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_handles: &[u64],
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        self.upload_resident_many(uploads)?;
        self.dispatch_resident_sequence_read_many(steps, read_handles)
    }

    /// Upload resident buffers, dispatch an ordered resident sequence, then
    /// read selected byte ranges.
    fn upload_resident_many_sequence_read_ranges(
        &self,
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_ranges: &[ResidentReadRange],
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        self.upload_resident_many(uploads)?;
        self.dispatch_resident_sequence_read_ranges(steps, read_ranges)
    }

    /// Same contract as [`Self::upload_resident_many_sequence_read_many`],
    /// but writes readbacks into caller-owned byte slots.
    fn upload_resident_many_sequence_read_many_into(
        &self,
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_handles: &[u64],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        let readbacks =
            self.upload_resident_many_sequence_read_many(uploads, steps, read_handles)?;
        if outputs.len() < readbacks.len() {
            outputs.resize_with(readbacks.len(), Vec::new);
        } else {
            outputs.truncate(readbacks.len());
        }
        for (slot, readback) in outputs.iter_mut().zip(readbacks) {
            slot.clear();
            slot.extend_from_slice(&readback);
        }
        Ok(())
    }

    /// Same contract as [`Self::upload_resident_many_sequence_read_many_into`],
    /// but first clears full resident buffers to zero.
    ///
    /// Portable dispatchers emulate clears as zero-byte payload uploads and
    /// still pay one upload/sequence/read boundary. CUDA overrides this to
    /// enqueue device-side memset operations on the same stream before explicit
    /// uploads and kernels, avoiding PCIe traffic for scratch initialization
    /// without adding host fences.
    fn clear_upload_resident_many_sequence_read_many_into(
        &self,
        clears: &[(u64, usize)],
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_handles: &[u64],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        if clears.is_empty() {
            return self.upload_resident_many_sequence_read_many_into(
                uploads,
                steps,
                read_handles,
                outputs,
            );
        }
        let mut fills = Vec::new();
        fills.try_reserve(clears.len()).map_err(|error| {
            DispatchError::BackendError(format!(
                "Fix: reserve resident clear fill descriptors before dispatch; requested {} clear(s): {error}.",
                clears.len()
            ))
        })?;
        for &(handle, byte_len) in clears {
            fills.push((handle, byte_len, 0));
        }
        self.fill_upload_resident_many_sequence_read_many_into(
            &fills,
            uploads,
            steps,
            read_handles,
            outputs,
        )
    }

    /// Same contract as
    /// [`Self::clear_upload_resident_many_sequence_read_many_into`], but fills
    /// each resident buffer with an arbitrary byte value.
    fn fill_upload_resident_many_sequence_read_many_into(
        &self,
        fills: &[(u64, usize, u8)],
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_handles: &[u64],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        if fills.is_empty() {
            return self.upload_resident_many_sequence_read_many_into(
                uploads,
                steps,
                read_handles,
                outputs,
            );
        }

        with_staged_fill_uploads(
            fills,
            uploads,
            "resident fill payloads",
            "resident fill/upload payloads",
            |combined_uploads| {
                self.upload_resident_many_sequence_read_many_into(
                    combined_uploads,
                    steps,
                    read_handles,
                    outputs,
                )
            },
        )
    }

    /// Same contract as [`Self::upload_resident_many_sequence_read_ranges_into`],
    /// but fills resident buffers first. CUDA overrides this to use device
    /// memset and compact D2H range copies on the same stream.
    fn fill_upload_resident_many_sequence_read_ranges_into(
        &self,
        fills: &[(u64, usize, u8)],
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_ranges: &[ResidentReadRange],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        if fills.is_empty() {
            return self.upload_resident_many_sequence_read_ranges_into(
                uploads,
                steps,
                read_ranges,
                outputs,
            );
        }

        with_staged_fill_uploads(
            fills,
            uploads,
            "resident range-fill payloads",
            "resident range-fill/upload payloads",
            |combined_uploads| {
                self.upload_resident_many_sequence_read_ranges_into(
                    combined_uploads,
                    steps,
                    read_ranges,
                    outputs,
                )
            },
        )
    }

    /// Same contract as [`Self::upload_resident_many_sequence_read_ranges`],
    /// but writes compact readbacks into caller-owned byte slots.
    fn upload_resident_many_sequence_read_ranges_into(
        &self,
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_ranges: &[ResidentReadRange],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        self.upload_resident_many(uploads)?;
        self.dispatch_resident_sequence(steps)?;
        self.read_resident_ranges_into(read_ranges, outputs)
    }
}

fn free_resident_handles<D: OptimizerDispatcher + ?Sized>(
    dispatcher: &D,
    handles: &[u64],
    context: &str,
) -> Result<(), DispatchError> {
    for (index, &handle) in handles.iter().enumerate() {
        dispatcher.free_resident(handle).map_err(|error| {
            DispatchError::BackendError(format!(
                "Fix: {context} failed to free resident handle {handle} at index {index}: {error}."
            ))
        })?;
    }
    Ok(())
}

fn with_staged_fill_uploads<R>(
    fills: &[(u64, usize, u8)],
    uploads: &[(u64, &[u8])],
    fill_context: &'static str,
    combined_context: &'static str,
    run: impl FnOnce(&[(u64, &[u8])]) -> Result<R, DispatchError>,
) -> Result<R, DispatchError> {
    let mut fill_payloads = Vec::new();
    fill_payloads.try_reserve(fills.len()).map_err(|error| {
        DispatchError::BackendError(format!(
            "Fix: reserve {fill_context} before dispatch; requested {} fill(s): {error}.",
            fills.len()
        ))
    })?;
    for &(_, byte_len, value) in fills {
        fill_payloads.push(vec![value; byte_len]);
    }

    let mut combined_uploads = Vec::new();
    combined_uploads
        .try_reserve(fills.len() + uploads.len())
        .map_err(|error| {
            DispatchError::BackendError(format!(
                "Fix: reserve {combined_context} before dispatch; requested {} fill(s) and {} upload(s): {error}.",
                fills.len(),
                uploads.len()
            ))
        })?;
    for ((handle, _, _), fill) in fills.iter().zip(fill_payloads.iter()) {
        combined_uploads.push((*handle, fill.as_slice()));
    }
    combined_uploads.extend_from_slice(uploads);

    run(&combined_uploads)
}

#[cfg(any(test, feature = "cpu-parity"))]
pub mod oracle {
    //! CPU oracle dispatcher for tests and explicit CPU-parity builds. Maps a small allowlist of
    //! self-hosted-optimizer Programs onto their `vyre_primitives`
    //! `cpu_ref` reference implementations and reproduces the
    //! dispatch byte contract.
    //!
    //! This module exists to prove the encoder/decoder are sound
    //! against the same numerical contract the production GPU path
    //! must honor. It is not compiled unless tests or `cpu-parity` are enabled.
    //!
    //! Adding a Program here means the oracle hand-writes the byte
    //! marshalling that the WgpuBackend dispatcher infers from
    //! `BufferDecl`s. That duplication is acceptable for tests; a
    //! production dispatcher reflectively reads BufferDecls.
    //!
    //! For now we cover the Programs the orchestrator currently
    //! invokes (DCE → `persistent_bfs`). When CSE / const-fold land
    //! they each add a small case here.

    use super::{DispatchError, OptimizerDispatcher};
    use vyre_foundation::ir::Program;

    /// CPU oracle dispatcher. Recognizes only the optimizer's own
    /// canonical Programs by matching the wrapping Region's generator
    /// op-id and the declared buffer set.
    pub struct CpuOracleDispatcher;

    impl CpuOracleDispatcher {
        /// Construct the oracle dispatcher. Cheap; does no backend
        /// probing.
        #[must_use]
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for CpuOracleDispatcher {
        fn default() -> Self {
            Self::new()
        }
    }

    impl OptimizerDispatcher for CpuOracleDispatcher {
        fn dispatch(
            &self,
            program: &Program,
            inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            // Identify the optimizer Program by its top-level Region
            // generator. Self-hosted Programs all wrap their bodies
            // in a Region with a known op-id.
            let generator = top_level_region_generator(program).ok_or_else(|| {
                DispatchError::Rejected(
                    "Fix: oracle dispatcher only accepts canonical \
                     graph-primitive Programs whose entry is a single \
                     wrapping Region with a generator id."
                        .to_string(),
                )
            })?;

            match generator {
                vyre_primitives::graph::persistent_bfs::OP_ID => {
                    persistent_bfs_oracle(program, inputs)
                }
                crate::optimizer::dce_program::OP_ID => persistent_bfs_oracle(program, inputs),
                vyre_primitives::graph::exploded::OP_ID => {
                    exploded_ifds_csr_oracle(program, inputs)
                }
                other => Err(DispatchError::Rejected(format!(
                    "Fix: oracle dispatcher does not recognize generator \
                     `{other}`. Wire the oracle for this primitive or \
                     dispatch through the production backend."
                ))),
            }
        }
    }

    fn top_level_region_generator(program: &Program) -> Option<&str> {
        match program.entry() {
            [vyre_foundation::ir::Node::Region { generator, .. }] => Some(generator.as_str()),
            _ => None,
        }
    }

    fn persistent_bfs_oracle(
        program: &Program,
        inputs: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        // Buffer order (per `persistent_bfs.rs::persistent_bfs`):
        //   0 pg_nodes (RO)
        //   1 pg_edge_offsets (RO)
        //   2 pg_edge_targets (RO)
        //   3 pg_edge_kind_mask (RO)
        //   4 pg_node_tags (RO)
        //   5 frontier_in (RO)
        //   6 frontier_out (RW)
        //   7 changed (RW)
        //   8 wg_scratch (workgroup)   -  not an input
        if inputs.len() < 6 {
            return Err(DispatchError::BadInputs(format!(
                "Fix: persistent_bfs oracle expects ≥ 6 input buffers, got {}",
                inputs.len()
            )));
        }
        let nodes = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let edge_offsets = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
        let edge_targets_raw = crate::hardware::dispatch_buffers::read_u32s(&inputs[2]);
        let edge_kind_mask_raw = crate::hardware::dispatch_buffers::read_u32s(&inputs[3]);
        let _node_tags = crate::hardware::dispatch_buffers::read_u32s(&inputs[4]);
        let frontier_in = crate::hardware::dispatch_buffers::read_u32s(&inputs[5]);

        // The Region carries the shape and max_iters in its body
        // structure; rather than re-derive that from IR walks, the
        // oracle re-computes via cpu_ref using the buffers' lengths.
        let node_count = nodes.len() as u32;

        // Iteration cap: if the caller declared `frontier_in` of length L
        // (= bitset_words(node_count)) the oracle uses `node_count` as
        // the saturation budget  -  same default the Program builder uses
        // when callers want closure.
        let max_iters = node_count.max(1);

        let _ = program; // reserved for future cross-checks
        let allow_mask = u32::MAX;
        let edge_count = declared_edge_count(&edge_offsets)?;
        let edge_targets = trim_padded_edge_buffer("edge_targets", &edge_targets_raw, edge_count)?;
        let edge_kind_mask =
            trim_padded_edge_buffer("edge_kind_mask", &edge_kind_mask_raw, edge_count)?;

        let (frontier_out, changed) = vyre_primitives::graph::persistent_bfs::cpu_ref(
            node_count,
            &edge_offsets,
            edge_targets,
            edge_kind_mask,
            &frontier_in,
            allow_mask,
            max_iters,
        );

        // Outputs in declared order: frontier_out first, then changed.
        let frontier_bytes = u32_buffer_to_bytes(&frontier_out);
        let changed_bytes = u32_buffer_to_bytes(&[changed]);
        Ok(vec![frontier_bytes, changed_bytes])
    }

    fn exploded_ifds_csr_oracle(
        program: &Program,
        inputs: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        if inputs.len() != 18 {
            return Err(DispatchError::BadInputs(format!(
                "Fix: exploded IFDS oracle expected 18 input buffers, got {}.",
                inputs.len()
            )));
        }

        let key = vyre_primitives::graph::exploded::ifds_program_cache_key_from_program(program)
            .map_err(DispatchError::BackendError)?;
        let (intra_edges, inter_edges, flow_gen, flow_kill) = parse_ifds_rule_inputs(&key, inputs)?;

        let (row_ptr, col_idx) = vyre_primitives::graph::exploded::build_cpu_reference(
            key.num_procs,
            key.blocks_per_proc,
            key.facts_per_proc,
            &intra_edges,
            &inter_edges,
            &flow_gen,
            &flow_kill,
        );

        let col_len = u32::try_from(col_idx.len()).map_err(|error| {
            DispatchError::BackendError(format!(
                "Fix: exploded IFDS oracle col_idx length does not fit u32: {error}."
            ))
        })?;
        let col_idx_words = program
            .buffer("col_idx")
            .map(|buffer| buffer.count() as usize)
            .unwrap_or(1);
        let mut col_idx_padded = vec![0u32; col_idx_words];
        if col_idx.len() > col_idx_words {
            return Err(DispatchError::BackendError(format!(
                "Fix: exploded IFDS oracle emitted {} columns but program allocates {col_idx_words}."
                ,
                col_idx.len()
            )));
        }
        col_idx_padded[..col_idx.len()].copy_from_slice(&col_idx);

        let row_cursor_words = program
            .buffer("row_cursor")
            .map(|buffer| buffer.count() as usize)
            .unwrap_or(1);
        let row_cursor = vec![0u32; row_cursor_words];

        Ok(vec![
            u32_buffer_to_bytes(&row_ptr),
            u32_buffer_to_bytes(&row_cursor),
            u32_buffer_to_bytes(&col_idx_padded),
            u32_buffer_to_bytes(&[col_len]),
        ])
    }

    fn parse_ifds_rule_inputs(
        key: &vyre_primitives::graph::exploded::IfdsCsrProgramCacheKey,
        inputs: &[Vec<u8>],
    ) -> Result<
        (
            Vec<(u32, u32, u32)>,
            Vec<(u32, u32, u32, u32)>,
            Vec<(u32, u32, u32)>,
            Vec<(u32, u32, u32)>,
        ),
        DispatchError,
    > {
        let intra_proc = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let intra_src_block = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
        let intra_dst_block = crate::hardware::dispatch_buffers::read_u32s(&inputs[2]);
        let inter_src_proc = crate::hardware::dispatch_buffers::read_u32s(&inputs[3]);
        let inter_src_block = crate::hardware::dispatch_buffers::read_u32s(&inputs[4]);
        let inter_dst_proc = crate::hardware::dispatch_buffers::read_u32s(&inputs[5]);
        let inter_dst_block = crate::hardware::dispatch_buffers::read_u32s(&inputs[6]);
        let gen_proc = crate::hardware::dispatch_buffers::read_u32s(&inputs[7]);
        let gen_block = crate::hardware::dispatch_buffers::read_u32s(&inputs[8]);
        let gen_fact = crate::hardware::dispatch_buffers::read_u32s(&inputs[9]);
        let kill_proc = crate::hardware::dispatch_buffers::read_u32s(&inputs[10]);
        let kill_block = crate::hardware::dispatch_buffers::read_u32s(&inputs[11]);
        let kill_fact = crate::hardware::dispatch_buffers::read_u32s(&inputs[12]);

        let intra_edges = read_ifds_triples(
            "intra",
            key.intra_count,
            &intra_proc,
            &intra_src_block,
            &intra_dst_block,
        )?;
        let inter_edges = read_ifds_quads(
            "inter",
            key.inter_count,
            &inter_src_proc,
            &inter_src_block,
            &inter_dst_proc,
            &inter_dst_block,
        )?;
        let flow_gen = read_ifds_triples("GEN", key.gen_count, &gen_proc, &gen_block, &gen_fact)?;
        let flow_kill =
            read_ifds_triples("KILL", key.kill_count, &kill_proc, &kill_block, &kill_fact)?;

        Ok((intra_edges, inter_edges, flow_gen, flow_kill))
    }

    fn read_ifds_triples(
        kind: &str,
        count: u32,
        proc: &[u32],
        a: &[u32],
        b: &[u32],
    ) -> Result<Vec<(u32, u32, u32)>, DispatchError> {
        let count = count as usize;
        for (name, column) in [("proc", proc), ("a", a), ("b", b)] {
            if column.len() < count {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: exploded IFDS oracle {kind} {name} column has {} word(s), expected {count}."
                    ,
                    column.len()
                )));
            }
        }
        Ok((0..count)
            .map(|index| (proc[index], a[index], b[index]))
            .collect())
    }

    fn read_ifds_quads(
        kind: &str,
        count: u32,
        a: &[u32],
        b: &[u32],
        c: &[u32],
        d: &[u32],
    ) -> Result<Vec<(u32, u32, u32, u32)>, DispatchError> {
        let count = count as usize;
        for (name, column) in [
            ("src_proc", a),
            ("src_block", b),
            ("dst_proc", c),
            ("dst_block", d),
        ] {
            if column.len() < count {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: exploded IFDS oracle {kind} {name} column has {} word(s), expected {count}."
                    ,
                    column.len()
                )));
            }
        }
        Ok((0..count)
            .map(|index| (a[index], b[index], c[index], d[index]))
            .collect())
    }

    fn declared_edge_count(edge_offsets: &[u32]) -> Result<usize, DispatchError> {
        edge_offsets
            .last()
            .copied()
            .map(|edge_count| edge_count as usize)
            .ok_or_else(|| {
                DispatchError::BadInputs(
                    "Fix: persistent_bfs oracle requires a CSR offset sentinel.".to_string(),
                )
            })
    }

    fn trim_padded_edge_buffer<'a>(
        name: &str,
        buffer: &'a [u32],
        edge_count: usize,
    ) -> Result<&'a [u32], DispatchError> {
        if buffer.len() < edge_count {
            return Err(DispatchError::BadInputs(format!(
                "Fix: persistent_bfs oracle {name} has {} words but CSR declares {edge_count} edges.",
                buffer.len()
            )));
        }
        Ok(&buffer[..edge_count])
    }

    fn u32_buffer_to_bytes(words: &[u32]) -> Vec<u8> {
        vyre_primitives::wire::pack_u32_slice(words)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct RangedReadDispatcher {
        buffers: Vec<(u64, Vec<u8>)>,
        read_calls: Cell<usize>,
        batched_handles: RefCell<Vec<u64>>,
    }

    impl OptimizerDispatcher for RangedReadDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            Err(DispatchError::Rejected(
                "Fix: ranged-read test dispatcher does not implement dispatch.".to_string(),
            ))
        }

        fn read_resident(&self, handle: u64) -> Result<Vec<u8>, DispatchError> {
            self.read_calls.set(self.read_calls.get() + 1);
            self.buffers
                .iter()
                .find(|(candidate, _)| *candidate == handle)
                .map(|(_, bytes)| bytes.clone())
                .ok_or_else(|| {
                    DispatchError::BadInputs(format!(
                        "Fix: test dispatcher missing resident handle {handle}."
                    ))
                })
        }

        fn read_resident_many(&self, handles: &[u64]) -> Result<Vec<Vec<u8>>, DispatchError> {
            self.batched_handles.borrow_mut().extend_from_slice(handles);
            handles
                .iter()
                .map(|&handle| self.read_resident(handle))
                .collect()
        }
    }

    struct FailingAllocDispatcher {
        next_handle: Cell<u64>,
        fail_at_call: usize,
        allocations: RefCell<Vec<usize>>,
        freed: RefCell<Vec<u64>>,
    }

    impl FailingAllocDispatcher {
        fn new(first_handle: u64, fail_at_call: usize) -> Self {
            Self {
                next_handle: Cell::new(first_handle),
                fail_at_call,
                allocations: RefCell::new(Vec::new()),
                freed: RefCell::new(Vec::new()),
            }
        }
    }

    impl OptimizerDispatcher for FailingAllocDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            Err(DispatchError::Rejected(
                "Fix: failing allocation test dispatcher does not implement dispatch.".to_string(),
            ))
        }

        fn alloc_resident(&self, byte_len: usize) -> Result<u64, DispatchError> {
            let call = self.allocations.borrow().len();
            self.allocations.borrow_mut().push(byte_len);
            if call == self.fail_at_call {
                return Err(DispatchError::BackendError(
                    "Fix: injected optimizer resident allocation failure".to_string(),
                ));
            }
            let handle = self.next_handle.get();
            self.next_handle.set(handle + 1);
            Ok(handle)
        }

        fn free_resident(&self, handle: u64) -> Result<(), DispatchError> {
            self.freed.borrow_mut().push(handle);
            Ok(())
        }
    }

    #[test]
    fn generated_fill_upload_staging_preserves_fill_then_upload_order() {
        let host_payload = [0xA5_u8, 0x5A];
        let mut staged = Vec::new();

        with_staged_fill_uploads(
            &[(7, 3, 0x11), (9, 2, 0x22)],
            &[(13, host_payload.as_slice())],
            "test fill payloads",
            "test combined uploads",
            |uploads| {
                for &(handle, bytes) in uploads {
                    staged.push((handle, bytes.to_vec()));
                }
                Ok(())
            },
        )
        .expect("Fix: shared resident fill staging should succeed");

        assert_eq!(
            staged,
            vec![
                (7, vec![0x11, 0x11, 0x11]),
                (9, vec![0x22, 0x22]),
                (13, host_payload.to_vec()),
            ],
            "resident fill staging must preserve device-fill uploads before caller uploads"
        );
    }

    #[test]
    fn resident_grouped_allocation_rolls_back_partial_handles() {
        let dispatcher = FailingAllocDispatcher::new(90, 2);

        let err = dispatcher
            .alloc_resident_many(&[4, 8, 12])
            .expect_err("Fix: injected grouped allocation failure should surface");

        assert!(
            matches!(err, DispatchError::BackendError(message) if message.contains("injected optimizer resident allocation failure"))
        );
        assert_eq!(dispatcher.allocations.borrow().as_slice(), &[4, 8, 12]);
        assert_eq!(
            dispatcher.freed.borrow().as_slice(),
            &[90, 91],
            "Fix: grouped resident allocation must free every prior handle on failure."
        );
    }

    #[test]
    fn ranged_readback_deduplicates_full_buffer_reads_by_handle() {
        let dispatcher = RangedReadDispatcher {
            buffers: vec![(7, (0u8..32).collect()), (9, (100u8..132).collect())],
            read_calls: Cell::new(0),
            batched_handles: RefCell::new(Vec::new()),
        };

        let outputs = dispatcher
            .read_resident_ranges(&[
                ResidentReadRange {
                    handle_id: 7,
                    byte_offset: 4,
                    byte_len: 4,
                },
                ResidentReadRange {
                    handle_id: 9,
                    byte_offset: 2,
                    byte_len: 3,
                },
                ResidentReadRange {
                    handle_id: 7,
                    byte_offset: 12,
                    byte_len: 5,
                },
            ])
            .expect("Fix: ranged readback must succeed for in-bounds dedup keys; return Err on overlap violations - deduplicated ranged readback must succeed");

        assert_eq!(
            outputs,
            vec![
                vec![4, 5, 6, 7],
                vec![102, 103, 104],
                vec![12, 13, 14, 15, 16]
            ]
        );
        assert_eq!(
            dispatcher.read_calls.get(),
            2,
            "Fix: default ranged readback must read each unique resident handle once, not once per range."
        );
        assert_eq!(
            dispatcher.batched_handles.borrow().as_slice(),
            &[7, 9],
            "Fix: default ranged readback must preserve first-seen handle order for batched backend overrides."
        );
    }

    #[test]
    fn ranged_readback_into_reuses_output_slots_without_intermediate_readbacks() {
        let dispatcher = RangedReadDispatcher {
            buffers: vec![(7, (0u8..32).collect()), (9, (100u8..132).collect())],
            read_calls: Cell::new(0),
            batched_handles: RefCell::new(Vec::new()),
        };
        let mut outputs = vec![
            Vec::with_capacity(16),
            Vec::with_capacity(16),
            Vec::with_capacity(16),
        ];
        let capacities = outputs.iter().map(Vec::capacity).collect::<Vec<_>>();

        dispatcher
            .read_resident_ranges_into(
                &[
                    ResidentReadRange {
                        handle_id: 7,
                        byte_offset: 0,
                        byte_len: 4,
                    },
                    ResidentReadRange {
                        handle_id: 9,
                        byte_offset: 4,
                        byte_len: 4,
                    },
                    ResidentReadRange {
                        handle_id: 7,
                        byte_offset: 8,
                        byte_len: 4,
                    },
                ],
                &mut outputs,
            )
            .expect("Fix: caller buffer must be sized for readback range; return Err if storage too small - ranged readback into caller storage must succeed");

        assert_eq!(
            outputs,
            vec![
                vec![0, 1, 2, 3],
                vec![104, 105, 106, 107],
                vec![8, 9, 10, 11]
            ]
        );
        assert_eq!(
            outputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            capacities,
            "Fix: ranged readback into caller storage must retain output slot capacity."
        );
        assert_eq!(dispatcher.read_calls.get(), 2);

        let source = include_str!("dispatcher.rs");
        assert!(
            source.contains("self.read_resident_ranges_into(read_ranges, outputs)")
                && !source.contains(concat!(
                    "let readbacks =\n            self.upload_resident_many_sequence_read_ranges"
                )),
            "Fix: resident range readback _into path must not allocate an intermediate Vec<Vec<u8>> before copying into caller-owned outputs."
        );
    }

    #[test]
    fn generated_ranged_readbacks_deduplicate_handles_without_reordering_ranges() {
        let dispatcher = RangedReadDispatcher {
            buffers: (0..8u64)
                .map(|handle| {
                    (
                        handle,
                        (0..64u8)
                            .map(|byte| byte.wrapping_add((handle as u8).wrapping_mul(17)))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect(),
            read_calls: Cell::new(0),
            batched_handles: RefCell::new(Vec::new()),
        };
        let ranges = (0..2048usize)
            .map(|case| ResidentReadRange {
                handle_id: ((case.wrapping_mul(5).wrapping_add(case / 11)) % 8) as u64,
                byte_offset: (case.wrapping_mul(7)) % 48,
                byte_len: (case % 16) + 1,
            })
            .collect::<Vec<_>>();

        let outputs = dispatcher
            .read_resident_ranges(&ranges)
            .expect("Fix: generated matrix fixtures must stay in-bounds; fix fixture or return Err - generated ranged readback matrix must succeed");

        assert_eq!(outputs.len(), ranges.len());
        for (range, output) in ranges.iter().zip(outputs.iter()) {
            let full = dispatcher
                .buffers
                .iter()
                .find(|(handle, _)| *handle == range.handle_id)
                .map(|(_, bytes)| bytes.as_slice())
                .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated range uses known handle");
            assert_eq!(
                output.as_slice(),
                &full[range.byte_offset..range.byte_offset + range.byte_len],
                "generated range must preserve caller range order and byte-exact slices"
            );
        }
        assert_eq!(
            dispatcher.read_calls.get(),
            8,
            "Fix: generated ranged readback matrix must issue one full read per unique handle."
        );
    }
}
