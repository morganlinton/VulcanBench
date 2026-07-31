//! Resident-buffer dispatch for [`RulePipeline`] (the regex/NFA mega-scan path).
//!
//! # Why this exists
//!
//! [`RulePipeline::scan`](super::mega_scan::RulePipeline::scan) issues every
//! dispatch through `dispatch_borrowed`, which re-creates GPU buffers and
//! **re-uploads the lane-major NFA transition table on every call**. That table
//! is `num_states × 256 × LANES_PER_SUBGROUP` u32s — tens of MiB for a large
//! detector set — and it is *immutable* across scans of the same pipeline. A
//! consumer that scans many buffers (a directory walk coalesced into batches)
//! pays that multi-MiB host→device transfer once per batch even though only the
//! haystack and the hit buffer actually change.
//!
//! [`ResidentRulePipeline`] uploads the transition and epsilon tables **once**
//! into backend-resident resources and keeps them resident for the lifetime of
//! the session. Each [`scan`](ResidentRulePipeline::scan_into) then transfers
//! only the haystack (a ranged upload into the resident haystack buffer), a
//! 4-byte hit-counter reset, and the two 4-byte control values (haystack length
//! and the per-workgroup scan bound), dispatches against the resident tables, and
//! decodes the hit buffer — the per-scan transfer drops from `O(tables + haystack)`
//! to `O(haystack)`. This is the regex-path counterpart of
//! [`GpuLiteralSet::prepare_scan_dispatch`](super::literal_set::GpuLiteralSet::prepare_scan_dispatch).
//!
//! The match wire format is byte-identical to [`RulePipeline::scan`] (slot 0 =
//! atomic counter, then `(pattern_id, start, end)` triples), so a consumer can
//! swap the borrowed path for a resident session without changing any
//! post-processing — proven by the host-orchestration unit test below and the
//! CUDA backend parity test
//! (`vyre-driver-cuda/tests/resident_rule_pipeline_cuda_parity.rs`).
//!
//! # Backend support
//!
//! Resident dispatch requires a backend that implements the resident half of
//! the [`VyreBackend`] contract (`allocate_resident`, `upload_resident*`,
//! `dispatch_resident_timed`). The wgpu and CUDA backends do; the CPU reference
//! does not. [`RulePipeline::prepare_resident`] returns the backend's
//! `UnsupportedFeature` error **loudly** — the caller must handle it explicitly
//! (fail closed, or a loud/recorded fallback), never degrade silently.

use vyre::{BackendError, DispatchConfig, VyreBackend};
use vyre_driver::Resource;
use vyre_foundation::ir::Program;
use vyre_foundation::match_result::Match;

use super::dispatch_io;
use super::mega_scan::{hit_buffer_byte_len, RulePipeline};

/// A [`RulePipeline`] with its immutable NFA tables uploaded into
/// backend-resident resources, ready for repeated low-overhead scans.
///
/// Construct with [`RulePipeline::prepare_resident`]. The session owns four
/// resident resources (haystack, transition table, epsilon table, hit buffer);
/// call [`free`](Self::free) to release them, or drop the session and let the
/// backend reclaim them when its device context is torn down.
///
/// The session is `Send + Sync`: the resident handles are opaque ids and all
/// mutation happens through the borrowed `backend`, so a single session can be
/// shared across scan threads (each thread supplies its own packing scratch).
#[derive(Debug)]
pub struct ResidentRulePipeline {
    /// The pipeline's compiled GPU program (cheap to hold; the heavy tables are
    /// resident, not in this clone).
    program: Program,
    /// Resident haystack buffer, sized to `haystack_capacity` padded bytes.
    haystack: Resource,
    /// Resident lane-major transition table (immutable, uploaded once).
    transition: Resource,
    /// Resident lane-major epsilon table (immutable, uploaded once).
    epsilon: Resource,
    /// Resident hit buffer (`max_matches × 3 + 1` u32s); counter reset per scan.
    hits: Resource,
    /// Resident haystack-length control buffer (1 u32; re-uploaded per scan).
    haystack_len_buf: Resource,
    /// Resident max-scan-bytes control buffer (1 u32; re-uploaded per scan).
    max_scan_bytes_buf: Resource,
    /// Padded byte capacity of the resident haystack buffer.
    haystack_capacity: usize,
    /// Match cap this session's hit buffer was sized for.
    max_matches: u32,
}

// SAFETY mirror of the `RulePipeline`/`GpuLiteralSet` contract: `Resource`
// handles are plain ids and `Program` is `Send + Sync`.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    let _ = assert_send_sync::<ResidentRulePipeline>;
};

impl RulePipeline {
    /// Upload this pipeline's immutable NFA tables into backend-resident
    /// resources and return a [`ResidentRulePipeline`] for repeated scans.
    ///
    /// `haystack_capacity_bytes` is the largest haystack the session will scan
    /// (e.g. the consumer's coalesced-batch cap); the resident haystack buffer
    /// is allocated once at that padded size and every scan uploads only its
    /// real bytes. `max_matches` sizes the resident hit buffer and caps decoded
    /// matches, exactly as in [`RulePipeline::scan`].
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend does not support resident
    /// resources, or when allocation / upload of the resident tables fails. The
    /// caller must handle this loudly (fail closed or a recorded fallback) —
    /// never degrade silently.
    pub fn prepare_resident(
        &self,
        backend: &dyn VyreBackend,
        haystack_capacity_bytes: usize,
        max_matches: u32,
    ) -> Result<ResidentRulePipeline, BackendError> {
        let haystack_capacity = dispatch_io::haystack_padded_u32_byte_len(haystack_capacity_bytes)?;

        // Fail closed early if the resident haystack would be SMALLER than the NFA
        // program's static input-buffer decl. A resident dispatch binds this buffer
        // to the program input (binding 0); the CUDA backend rejects a resident
        // buffer whose byte length is below the binding's `static_byte_len`
        // (`vyre-driver-cuda/.../resident_dispatch/batch.rs`), but wgpu tolerates an
        // oversized/undersized buffer. Without this guard an undersized
        // `haystack_capacity_bytes` PASSES on wgpu (the dev backend) and fails only
        // at dispatch on CUDA (a downstream consumer's backend) with a confusing late
        // `InvalidProgram` — a dev-passes / prod-fails backend divergence. Surface
        // it here, portably, at prepare time. Reuses `BufferDecl::static_byte_len`,
        // the exact source CUDA checks against, so a runtime-sized input
        // (`count == 0` → `Ok(None)`) correctly imposes no requirement.
        if let Some(input_decl) = self.program.buffers().iter().find(|decl| decl.binding == 0) {
            if let Some(required_bytes) = input_decl.static_byte_len().map_err(BackendError::new)? {
                if haystack_capacity < required_bytes {
                    return Err(BackendError::new(format!(
                        "ResidentRulePipeline::prepare_resident: the NFA program's input buffer (binding 0) statically declares {required_bytes} byte(s) (input_len = {input_len}), but the requested resident haystack capacity is only {haystack_capacity} padded byte(s). A resident dispatch binds this buffer to the program input; the CUDA backend requires the resident buffer to be at least the static decl, so this would fail at dispatch with a confusing late error (wgpu would silently tolerate it). Fix: pass haystack_capacity_bytes >= {input_len} to prepare_resident, or rebuild the RulePipeline (build / build_rule_pipeline) with a smaller input_len.",
                        input_len = self.plan.input_len,
                    )));
                }
            }
        }

        let haystack = backend.allocate_resident(haystack_capacity)?;

        let transition_bytes = dispatch_io::u32_words_as_le_bytes(&self.transition_table);
        let transition = backend.allocate_resident(transition_bytes.len())?;
        backend.upload_resident(&transition, transition_bytes.as_ref())?;

        let epsilon_bytes = dispatch_io::u32_words_as_le_bytes(&self.epsilon_table);
        let epsilon = backend.allocate_resident(epsilon_bytes.len())?;
        backend.upload_resident(&epsilon, epsilon_bytes.as_ref())?;

        let hit_capacity = hit_buffer_byte_len(max_matches)?;
        let hits = backend.allocate_resident(hit_capacity)?;

        // The two 1-u32 control buffers (haystack_len, max_scan_bytes) are ALSO
        // resident. The CUDA backend's resident dispatch rejects any borrowed
        // resource (it resolves every binding to a resident handle), so a resident
        // dispatch must be ALL-resident — a borrowed-control mix works on wgpu but
        // fails closed on CUDA. They are allocated once here and re-uploaded per scan.
        let control_byte_len = std::mem::size_of::<u32>();
        let haystack_len_buf = backend.allocate_resident(control_byte_len)?;
        let max_scan_bytes_buf = backend.allocate_resident(control_byte_len)?;

        Ok(ResidentRulePipeline {
            program: self.program.clone(),
            haystack,
            transition,
            epsilon,
            hits,
            haystack_len_buf,
            max_scan_bytes_buf,
            haystack_capacity,
            max_matches,
        })
    }
}

impl ResidentRulePipeline {
    /// Scan `haystack` against the resident pipeline, decoding matches into
    /// caller-owned `matches`. Equivalent to [`RulePipeline::scan`] but with the
    /// NFA tables already resident (no per-scan table transfer).
    ///
    /// `scratch` reuses the packed-haystack staging buffer across calls; pass a
    /// per-thread `Vec` that lives as long as the scan loop.
    ///
    /// Walks every workgroup to end-of-haystack (`max_scan_bytes = u32::MAX`),
    /// matching [`RulePipeline::scan`]. Use [`scan_bounded_into`](Self::scan_bounded_into)
    /// to cap per-workgroup work to the longest possible match length.
    ///
    /// # Errors
    /// Returns [`BackendError`] on upload, dispatch, or readback failure, or
    /// when `haystack` exceeds the session's configured capacity.
    pub fn scan_into(
        &self,
        backend: &dyn VyreBackend,
        haystack: &[u8],
        matches: &mut Vec<Match>,
        scratch: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        self.scan_bounded_into(backend, haystack, u32::MAX, matches, scratch)
    }

    /// Per-workgroup-bounded resident scan. See [`RulePipeline::scan_bounded`]
    /// for the bound's semantics (O(N × max_scan_bytes) instead of O(N²)).
    ///
    /// # Errors
    /// Same as [`scan_into`](Self::scan_into).
    pub fn scan_bounded_into(
        &self,
        backend: &dyn VyreBackend,
        haystack: &[u8],
        max_scan_bytes: u32,
        matches: &mut Vec<Match>,
        scratch: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        matches.clear();
        let haystack_len = dispatch_io::scan_guard(
            haystack,
            "ResidentRulePipeline::scan",
            dispatch_io::DEFAULT_MAX_SCAN_BYTES,
        )?;

        // Stage the haystack into the resident buffer (real bytes only; the
        // kernel bounds its cursor with nfa_haystack_len so the stale tail of
        // the resident buffer is never read).
        dispatch_io::pack_haystack_u32_into(haystack, scratch)?;
        if scratch.len() > self.haystack_capacity {
            return Err(BackendError::new(format!(
                "ResidentRulePipeline haystack is {} packed byte(s) but the resident buffer holds {}. Fix: raise haystack_capacity_bytes in prepare_resident or shard the haystack.",
                scratch.len(),
                self.haystack_capacity
            )));
        }
        backend.upload_resident_at(&self.haystack, 0, scratch)?;

        // Reset only the atomic hit counter (slot 0). Triples are written from
        // slot 0 upward and only `count` of them are read back, so stale triples
        // beyond the new count are never observed — a 4-byte reset, not a full
        // hit-buffer clear.
        backend.upload_resident_at(&self.hits, 0, &0u32.to_le_bytes())?;

        // Stage the two 1-u32 control buffers into their resident slots. They MUST
        // be resident, not borrowed: the CUDA resident dispatch resolves every
        // binding to a resident handle and rejects a borrowed mix, so an all-resident
        // dispatch is the only form portable across wgpu AND CUDA. The upload copies
        // the source synchronously, so reusing the just-staged `scratch` above is
        // safe.
        backend.upload_resident_at(&self.haystack_len_buf, 0, &haystack_len.to_le_bytes())?;
        backend.upload_resident_at(&self.max_scan_bytes_buf, 0, &max_scan_bytes.to_le_bytes())?;

        // Buffer binding order MUST match `nfa::nfa_scan`'s BufferDecl order:
        // input(0), nfa_transition(1), nfa_epsilon(2), hits(3),
        // nfa_haystack_len(4), nfa_max_scan_bytes(5) — every binding resident.
        let resources = [
            self.haystack.clone(),
            self.transition.clone(),
            self.epsilon.clone(),
            self.hits.clone(),
            self.haystack_len_buf.clone(),
            self.max_scan_bytes_buf.clone(),
        ];

        let mut config = DispatchConfig::default();
        // Candidate-start parallelism: one workgroup per haystack byte, matching
        // `dispatch_io::candidate_start_dispatch_config`.
        config.grid_override = Some([haystack_len.max(1), 1, 1]);

        let timed = backend.dispatch_resident_timed(&self.program, &resources, &config)?;

        // The hit buffer is the program's only ReadWrite storage, returned at
        // output index 0 — identical decode to `RulePipeline::scan`.
        let hit_bytes =
            dispatch_io::try_output_bytes(&timed.outputs, 0, "ResidentRulePipeline hit buffer")?;
        let count = dispatch_io::try_read_u32_prefix(hit_bytes, "ResidentRulePipeline hit buffer")?;
        // Truncation guard: the resident hit buffer is fixed-size, so a batch
        // that overflows `max_matches` would silently drop matches (a false
        // negative). The shared capped decode surfaces it as an error so the
        // consumer degrades to a per-batch-sized borrowed dispatch instead.
        // Never decode a truncated set.
        dispatch_io::try_unpack_match_triples_capped_into(
            &hit_bytes[4..],
            count,
            self.max_matches,
            "ResidentRulePipeline hit buffer",
            matches,
        )
    }

    /// The match cap this session's resident hit buffer was sized for.
    #[must_use]
    pub fn max_matches(&self) -> u32 {
        self.max_matches
    }

    /// Padded byte capacity of the resident haystack buffer.
    #[must_use]
    pub fn haystack_capacity(&self) -> usize {
        self.haystack_capacity
    }

    /// Release every resident resource this session owns.
    ///
    /// Call this before the backend's device context is dropped to reclaim the
    /// resident allocations eagerly; otherwise they are reclaimed when the
    /// backend tears down. The session is consumed.
    ///
    /// # Errors
    /// Returns the first [`BackendError`] from freeing a resource; remaining
    /// resources are still attempted.
    pub fn free(self, backend: &dyn VyreBackend) -> Result<(), BackendError> {
        let mut first_err = None;
        for resource in [
            self.haystack,
            self.transition,
            self.epsilon,
            self.hits,
            self.haystack_len_buf,
            self.max_scan_bytes_buf,
        ] {
            if let Err(error) = backend.free_resident(resource) {
                first_err.get_or_insert(error);
            }
        }
        first_err.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::Mutex;
    use vyre::DispatchConfig as Config;
    use vyre_driver::TimedDispatchResult;
    use vyre_foundation::ir::Program;

    /// Mock backend that records resident traffic and returns a canned hit
    /// buffer, so the host orchestration (table-upload-once, per-scan haystack
    /// staging, counter reset, decode) is validated without a GPU. Real
    /// GPU resident-vs-borrowed parity is asserted in the downstream scanner crate
    /// where a live wgpu/CUDA backend is available. `VyreBackend` requires
    /// `Send + Sync`, so the counters use atomics/`Mutex`, not `RefCell`.
    struct MockResidentBackend {
        next_id: AtomicU64,
        /// (handle_id, byte_len) for every allocate_resident call.
        allocations: Mutex<Vec<(u64, usize)>>,
        /// Number of full uploads (table uploads) seen.
        full_uploads: AtomicUsize,
        /// Number of ranged uploads (haystack + counter resets) seen.
        ranged_uploads: AtomicUsize,
        /// Canned hit-buffer bytes returned at output index 0.
        hit_buffer: Vec<u8>,
    }

    impl MockResidentBackend {
        fn new(hit_buffer: Vec<u8>) -> Self {
            Self {
                next_id: AtomicU64::new(1),
                allocations: Mutex::new(Vec::new()),
                full_uploads: AtomicUsize::new(0),
                ranged_uploads: AtomicUsize::new(0),
                hit_buffer,
            }
        }
    }

    impl vyre::backend::private::Sealed for MockResidentBackend {}

    impl VyreBackend for MockResidentBackend {
        fn id(&self) -> &'static str {
            "mock-resident"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &Config,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            unreachable!("resident path does not use borrowed dispatch")
        }

        fn allocate_resident(&self, byte_len: usize) -> Result<Resource, BackendError> {
            let handle = self.next_id.fetch_add(1, Ordering::Relaxed);
            self.allocations
                .lock()
                .expect("mock allocations mutex")
                .push((handle, byte_len));
            Ok(Resource::Resident(handle))
        }

        fn upload_resident(&self, _resource: &Resource, _bytes: &[u8]) -> Result<(), BackendError> {
            self.full_uploads.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn upload_resident_at(
            &self,
            _resource: &Resource,
            _dst_offset_bytes: usize,
            _bytes: &[u8],
        ) -> Result<(), BackendError> {
            self.ranged_uploads.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn free_resident(&self, _resource: Resource) -> Result<(), BackendError> {
            Ok(())
        }

        fn dispatch_resident_timed(
            &self,
            _program: &Program,
            resources: &[Resource],
            config: &Config,
        ) -> Result<TimedDispatchResult, BackendError> {
            // Contract checks the consumer relies on:
            assert_eq!(resources.len(), 6, "nfa_scan binds six buffers");
            // EVERY binding must be resident — the CUDA resident dispatch rejects a
            // borrowed-resource mix, so no binding (not even the two 1-u32 control
            // buffers) may be Borrowed.
            for (idx, resource) in resources.iter().enumerate() {
                assert!(
                    matches!(resource, Resource::Resident(_)),
                    "binding {idx} must be resident (no borrowed mix in a resident dispatch)"
                );
            }
            assert!(
                config.grid_override.is_some(),
                "resident scan must supply candidate-start grid override"
            );
            Ok(TimedDispatchResult {
                outputs: vec![self.hit_buffer.clone()],
                wall_ns: 0,
                device_ns: None,
                enqueue_ns: None,
                wait_ns: None,
            })
        }
    }

    fn hit_buffer_with(matches: &[(u32, u32, u32)]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + matches.len() * 12);
        bytes.extend_from_slice(&(matches.len() as u32).to_le_bytes());
        for &(pid, start, end) in matches {
            bytes.extend_from_slice(&pid.to_le_bytes());
            bytes.extend_from_slice(&start.to_le_bytes());
            bytes.extend_from_slice(&end.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn prepare_resident_uploads_tables_once_then_scans_transfer_only_haystack() {
        let pipeline = super::super::mega_scan::build(&["ab", "cd"], "input", "hits", 4096);
        let canned = hit_buffer_with(&[(0, 1, 3), (1, 5, 7)]);
        let backend = MockResidentBackend::new(canned);

        let session = pipeline
            .prepare_resident(&backend, 4096, 64)
            .expect("mock backend supports resident allocation");

        // Six resident allocations: haystack, transition, epsilon, hits, and the two
        // 1-u32 control buffers (haystack_len, max_scan_bytes).
        assert_eq!(backend.allocations.lock().unwrap().len(), 6);
        // The two immutable tables are uploaded exactly once, at prepare time.
        assert_eq!(backend.full_uploads.load(Ordering::Relaxed), 2);
        assert_eq!(backend.ranged_uploads.load(Ordering::Relaxed), 0);

        let mut scratch = Vec::new();
        let mut matches = Vec::new();
        for _ in 0..3 {
            session
                .scan_into(&backend, b"zabqcd", &mut matches, &mut scratch)
                .expect("resident scan decodes canned hits");
        }

        // Decode parity: canned triples surface byte-identically to the borrowed
        // path's `Match` decode.
        assert_eq!(matches, vec![Match::new(0, 1, 3), Match::new(1, 5, 7)]);
        // No further full uploads after prepare; each scan does exactly four ranged
        // uploads (haystack stage + counter reset + haystack_len + max_scan_bytes) —
        // the immutable tables never move again.
        assert_eq!(
            backend.full_uploads.load(Ordering::Relaxed),
            2,
            "tables re-uploaded mid-loop"
        );
        assert_eq!(
            backend.ranged_uploads.load(Ordering::Relaxed),
            12,
            "3 scans × (haystack + counter reset + haystack_len + max_scan_bytes)"
        );
    }

    #[test]
    fn scan_rejects_truncating_hit_count_instead_of_dropping_matches() {
        let pipeline = super::super::mega_scan::build(&["ab"], "input", "hits", 64);
        // Canned counter says 9 hits but the session was sized for 4 — decoding
        // would silently drop 5. The guard must error so the caller degrades.
        let mut canned = 9u32.to_le_bytes().to_vec();
        canned.extend(std::iter::repeat(0u8).take(4 * 12)); // only 4 triples present
        let backend = MockResidentBackend::new(canned);
        let session = pipeline
            .prepare_resident(&backend, 64, 4)
            .expect("prepare with a 4-match cap");

        let mut scratch = Vec::new();
        let mut matches = vec![Match::new(7, 7, 7)];
        let err = session
            .scan_into(&backend, b"ab", &mut matches, &mut scratch)
            .expect_err("hit count over the resident cap must error, not truncate");
        let msg = err.to_string();
        assert!(
            msg.contains("exceeds the output-buffer cap 4")
                && msg.contains("drop 5 match(es)")
                && matches.is_empty(),
            "truncation guard must name the cap, the dropped count, and expose no partial matches: {err}"
        );
    }

    #[test]
    fn scan_rejects_haystack_larger_than_resident_capacity() {
        // Build the program's static input decl to match the resident capacity (16
        // bytes) so the prepare-time decl guard passes; the scan-time guard below is
        // what this test exercises (a 64-byte haystack into a 16-byte buffer).
        let pipeline = super::super::mega_scan::build(&["ab"], "input", "hits", 16);
        let backend = MockResidentBackend::new(hit_buffer_with(&[]));
        let session = pipeline
            .prepare_resident(&backend, 16, 8)
            .expect("prepare with a 16-byte haystack capacity");

        let mut scratch = Vec::new();
        let mut matches = Vec::new();
        let err = session
            .scan_into(&backend, &[b'a'; 64], &mut matches, &mut scratch)
            .expect_err("64-byte haystack must not fit a 16-byte resident buffer");
        assert!(
            err.to_string().contains("resident buffer holds") && matches.is_empty(),
            "capacity error must name the limit and expose no stale matches: {err}"
        );
    }

    #[test]
    fn prepare_resident_rejects_capacity_below_static_input_decl() {
        // The NFA program declares a STATIC input buffer of input_len=64 → 16 u32
        // words → 64 bytes (binding 0). The CUDA backend rejects a resident input
        // buffer smaller than that static decl; wgpu silently tolerates it. The
        // prepare-time guard turns that dev-passes / prod-fails divergence into one
        // portable, actionable error BEFORE any allocation.
        let pipeline = super::super::mega_scan::build(&["ab"], "input", "hits", 64);
        let backend = MockResidentBackend::new(hit_buffer_with(&[]));

        // Under-capacity (16 < 64): must fail closed at prepare time, naming the
        // declared bytes, the input_len, the requested capacity, and the fix — and
        // it must NOT have allocated anything (the guard precedes allocation).
        let err = pipeline
            .prepare_resident(&backend, 16, 8)
            .expect_err("capacity 16 below the 64-byte static input decl must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("statically declares 64 byte(s)")
                && msg.contains("input_len = 64")
                && msg.contains("only 16 padded byte(s)")
                && msg.contains("Fix: pass haystack_capacity_bytes >= 64"),
            "prepare guard must name decl bytes, input_len, capacity, and the fix: {msg}"
        );
        assert_eq!(
            backend.allocations.lock().unwrap().len(),
            0,
            "guard must fail BEFORE allocating any resident resource"
        );

        // Boundary (capacity == decl == 64): allowed. CUDA requires byte_len >= decl.
        pipeline
            .prepare_resident(&backend, 64, 8)
            .expect("capacity exactly equal to the static input decl must prepare");
        assert_eq!(
            backend.allocations.lock().unwrap().len(),
            6,
            "the boundary-equal prepare allocates all six resident buffers"
        );

        // Over-capacity (128 > 64): allowed — the CUDA contract is `>= decl`, not
        // exact equality, so a larger resident haystack buffer is valid.
        pipeline
            .prepare_resident(&backend, 128, 8)
            .expect("capacity above the static input decl must prepare (CUDA allows >= decl)");
    }
}
