//! Resident-buffer dispatch for [`GpuLiteralSet`] region-presence scans.
//!
//! # Why this exists
//!
//! [`GpuLiteralSet::scan_presence_by_region`](super::literal_set::GpuLiteralSet::scan_presence_by_region)
//! and its async sibling issue every dispatch through `dispatch_borrowed`, which
//! re-encodes and **re-uploads seven immutable tables on every call**: the DFA
//! transition / output-offset / output-record tables, the per-pattern length
//! table, and the three suffix-prefilter masks (end mask, 2-gram mask, 3-gram
//! bloom). For a large detector set the DFA transition table alone is
//! `state_count × 256` u32s — multiple MiB — and it is *identical* across every
//! file of a corpus. A consumer that scans many coalesced batches (a downstream
//! consumer's phase-1 layout) pays that multi-MiB host→device transfer once per batch even
//! though only the haystack and the per-region presence output actually change.
//!
//! [`ResidentPresencePipeline`] uploads those seven tables **once** into
//! backend-resident resources and keeps them resident for the session lifetime.
//! Each [`scan_into`](ResidentPresencePipeline::scan_into) then transfers only the
//! per-file haystack (a ranged upload into the resident haystack buffer), the small
//! per-scan control values (haystack length, region starts, region base), and
//! zeroes the used prefix of the resident presence buffer, dispatches against the
//! resident tables, and reads back the per-region presence bitmap — the per-scan
//! transfer drops from `O(tables + haystack)` to `O(haystack + region rows)`.
//! This is the region-presence counterpart of
//! [`RulePipeline::prepare_resident`](super::mega_scan::RulePipeline::prepare_resident)
//! (the regex/NFA mega-scan path) and of
//! [`GpuLiteralSet::prepare_presence_by_region_dispatch`](super::literal_set::GpuLiteralSet::prepare_presence_by_region_dispatch)
//! (the backend-neutral single-shot prepared payload).
//!
//! The decoded bitmap is byte-identical to
//! [`GpuLiteralSet::scan_presence_by_region`]'s return (bit `p` of region `r`'s
//! row is set iff pattern `p`'s literal occurs in region `r`), so a consumer can
//! swap the borrowed path for a resident session without changing any
//! post-processing — proven by the GPU parity test in the integration suite and
//! the host-orchestration unit test below.
//!
//! # The `max_regions` cap
//!
//! The resident program is built for `max_regions` coalesced files: that count
//! sizes the resident presence buffer (binding 6) and the kernel's
//! `ceil_log2(max_regions)` region binary-search width. The ACTUAL per-scan region
//! count is read dynamically from `buf_len(region_starts)`, so one resident
//! session serves any batch with `region_count <= max_regions`. A batch that
//! exceeds the cap is rejected **loudly** (it would index past the resident
//! presence buffer); the caller re-dispatches it through the per-batch-sized
//! borrowed [`GpuLiteralSet::scan_presence_by_region`] — never a silent truncation.
//!
//! # Backend support
//!
//! Resident dispatch requires a backend that implements the resident half of the
//! [`VyreBackend`] contract (`allocate_resident`, `upload_resident*`,
//! `dispatch_resident_timed`). The wgpu and CUDA backends do; the CPU reference
//! does not. [`GpuLiteralSet::prepare_resident_presence`] surfaces the backend's
//! `UnsupportedFeature` error **loudly** — the caller must handle it explicitly
//! (fail closed, or a loud/recorded fallback), never degrade silently.

use vyre::{BackendError, VyreBackend};
use vyre_driver::{Resource, TimedDispatchResult};
use vyre_foundation::ir::Program;

use super::dispatch_io;
use super::literal_set::{decode_presence_words_into, GpuLiteralSet};

const U32_BYTES: usize = std::mem::size_of::<u32>();

/// Number of buffer bindings in the region-presence program (see
/// [`super::literal_set::GpuLiteralSet::build_presence_by_region_dispatch`]).
const PRESENCE_BY_REGION_BINDINGS: usize = 12;

/// A [`GpuLiteralSet`] with its immutable region-presence tables uploaded into
/// backend-resident resources, ready for repeated low-overhead scans.
///
/// Construct with [`GpuLiteralSet::prepare_resident_presence`]. The session owns
/// twelve resident resources — haystack, the seven immutable tables, the read-write
/// presence buffer, and the three per-scan control buffers (haystack_len,
/// region_starts, region_base) — so the dispatch is ALL-resident (the CUDA backend's
/// resident dispatch rejects a borrowed-resource mix). Call [`free`](Self::free) to
/// release them, or drop the session and let the backend reclaim them when its
/// device context is torn down.
///
/// The session is `Send + Sync`: the resident handles are opaque ids and all
/// mutation happens through the borrowed `backend`, so a single session can be
/// shared across scan threads (each thread supplies its own packing scratch).
#[derive(Debug)]
pub struct ResidentPresencePipeline {
    /// Region-presence program sized for `max_regions` coalesced files.
    program: Program,
    /// Resident haystack buffer, sized to `haystack_capacity` padded bytes.
    haystack: Resource,
    /// Resident DFA transition table (immutable, uploaded once).
    transitions: Resource,
    /// Resident DFA output-offset table (immutable, uploaded once).
    output_offsets: Resource,
    /// Resident DFA output-record table (immutable, uploaded once).
    output_records: Resource,
    /// Resident per-pattern length table (immutable, uploaded once).
    pattern_lengths: Resource,
    /// Resident per-region presence buffer (read-write; used prefix reset per scan).
    presence: Resource,
    /// Resident suffix prefilter end mask (immutable, uploaded once).
    candidate_end_mask: Resource,
    /// Resident suffix prefilter 2-gram mask (immutable, uploaded once).
    candidate_suffix2_mask: Resource,
    /// Resident suffix prefilter 3-gram bloom (immutable, uploaded once).
    candidate_suffix3_bloom: Resource,
    /// Resident haystack-length control buffer (1 u32; re-uploaded per scan).
    haystack_len_buf: Resource,
    /// Resident region-starts control buffer (sized for `max_regions`; re-uploaded
    /// per scan, padded with a `u32::MAX` sentinel so `buf_len` stays fixed and no
    /// hit maps to a padding region — see [`ResidentPresencePipeline::scan_into`]).
    region_starts_buf: Resource,
    /// Resident shard-base control buffer (1 u32; re-uploaded per scan).
    region_base_buf: Resource,
    /// Padded byte capacity of the resident haystack buffer.
    haystack_capacity: usize,
    /// Largest coalesced-file count this session's presence buffer was sized for.
    max_regions: u32,
    /// Pattern count (bit width of each per-region presence row).
    pattern_count: u32,
    /// Presence bitmap `u32` words per region.
    presence_words: u32,
    /// Program workgroup X extent, for the per-scan byte-scan dispatch geometry.
    workgroup_x: u32,
}

// SAFETY mirror of the `ResidentRulePipeline`/`GpuLiteralSet` contract: `Resource`
// handles are plain ids and `Program` is `Send + Sync`.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    let _ = assert_send_sync::<ResidentPresencePipeline>;
};

impl GpuLiteralSet {
    /// Upload this matcher's immutable region-presence tables into backend-resident
    /// resources and return a [`ResidentPresencePipeline`] for repeated scans.
    ///
    /// `haystack_capacity_bytes` is the largest coalesced haystack the session will
    /// scan (e.g. the consumer's batch cap); the resident haystack buffer is
    /// allocated once at that padded size and every scan uploads only its real
    /// bytes. `max_regions` is the largest coalesced-file count the session will
    /// scan; it sizes the resident presence buffer and the kernel's region
    /// binary-search width, and caps decoded regions.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend does not support resident
    /// resources, when `max_regions` is zero, or when allocation / upload of the
    /// resident tables fails. The caller must handle this loudly (fail closed or a
    /// recorded fallback) — never degrade silently.
    pub fn prepare_resident_presence(
        &self,
        backend: &dyn VyreBackend,
        haystack_capacity_bytes: usize,
        max_regions: u32,
    ) -> Result<ResidentPresencePipeline, BackendError> {
        let tables = self.resident_presence_tables(max_regions)?;

        let haystack_capacity = dispatch_io::haystack_padded_u32_byte_len(haystack_capacity_bytes)?;
        let haystack = backend.allocate_resident(haystack_capacity)?;

        // The seven immutable tables: allocate + upload ONCE.
        let transitions = allocate_and_upload(backend, &tables.transitions)?;
        let output_offsets = allocate_and_upload(backend, &tables.output_offsets)?;
        let output_records = allocate_and_upload(backend, &tables.output_records)?;
        let pattern_lengths = allocate_and_upload(backend, &tables.pattern_lengths)?;
        let candidate_end_mask = allocate_and_upload(backend, &tables.candidate_end_mask)?;
        let candidate_suffix2_mask =
            allocate_and_upload(backend, &tables.candidate_suffix2_mask)?;
        let candidate_suffix3_bloom =
            allocate_and_upload(backend, &tables.candidate_suffix3_bloom)?;

        // The read-write presence buffer: sized for the full max_regions capacity,
        // zeroed per scan over the used prefix only.
        let presence_capacity_words = (max_regions as usize)
            .checked_mul(tables.presence_words as usize)
            .ok_or_else(|| {
                BackendError::new(format!(
                    "resident region-presence capacity {max_regions} regions × {} words/region overflows host usize. Fix: lower max_regions or shard the pattern set.",
                    tables.presence_words
                ))
            })?;
        let presence_capacity_bytes = presence_capacity_words
            .checked_mul(U32_BYTES)
            .ok_or_else(|| {
                BackendError::new(
                    "resident region-presence presence-buffer byte capacity overflows host usize. Fix: lower max_regions or shard the pattern set.".to_string(),
                )
            })?;
        let presence = backend.allocate_resident(presence_capacity_bytes)?;

        // The three per-scan control buffers are ALSO resident. The CUDA backend's
        // resident dispatch rejects any borrowed resource (it resolves every binding
        // to a resident handle), so a resident dispatch must be ALL-resident — a
        // borrowed-control mix works on wgpu but fails closed on CUDA, a downstream
        // consumer's backend. haystack_len and region_base are one u32 each; region_starts is
        // sized for the full max_regions cap and padded per scan so its `buf_len`
        // (the kernel's live region count) stays fixed at max_regions.
        let region_starts_capacity_bytes =
            (max_regions as usize).checked_mul(U32_BYTES).ok_or_else(|| {
                BackendError::new(
                    "resident region-presence region-starts byte capacity overflows host usize. Fix: lower max_regions.".to_string(),
                )
            })?;
        let haystack_len_buf = backend.allocate_resident(U32_BYTES)?;
        let region_starts_buf = backend.allocate_resident(region_starts_capacity_bytes)?;
        let region_base_buf = backend.allocate_resident(U32_BYTES)?;

        Ok(ResidentPresencePipeline {
            program: tables.program,
            haystack,
            transitions,
            output_offsets,
            output_records,
            pattern_lengths,
            presence,
            candidate_end_mask,
            candidate_suffix2_mask,
            candidate_suffix3_bloom,
            haystack_len_buf,
            region_starts_buf,
            region_base_buf,
            haystack_capacity,
            max_regions,
            pattern_count: tables.pattern_count,
            presence_words: tables.presence_words,
            workgroup_x: tables.workgroup_x,
        })
    }
}

/// Allocate a resident buffer sized to `bytes` and upload them once.
fn allocate_and_upload(backend: &dyn VyreBackend, bytes: &[u8]) -> Result<Resource, BackendError> {
    let resource = backend.allocate_resident(bytes.len())?;
    backend.upload_resident(&resource, bytes)?;
    Ok(resource)
}

impl ResidentPresencePipeline {
    /// Scan `haystack` (a coalesced batch with ascending `region_starts` beginning
    /// at 0) against the resident pipeline, decoding the per-region presence bitmap
    /// into caller-owned `out`. Equivalent to
    /// [`GpuLiteralSet::scan_presence_by_region`] but with the immutable tables
    /// already resident (no per-scan table transfer).
    ///
    /// `region_base` is added to every candidate position before the region binary
    /// search; pass `0` for a single-dispatch scan (see
    /// [`GpuLiteralSet::scan_presence_by_region_with_scratch`] for the sharded
    /// meaning). `scratch` reuses the packed-haystack / presence-reset staging
    /// buffer across calls; pass a per-thread `Vec` that lives as long as the scan
    /// loop.
    ///
    /// On return, `out` holds `region_starts.len() × presence_words` packed `u32`
    /// words: bit `p` of region `r`'s row is set iff pattern `p` occurs in region
    /// `r`.
    ///
    /// # Errors
    /// Returns [`BackendError`] when `region_starts` is empty / does not begin at 0,
    /// when `region_count` exceeds the session's `max_regions` cap, when `haystack`
    /// exceeds the session's haystack capacity, or on upload / dispatch / readback
    /// failure. On any error `out` is left cleared (no partial bitmap).
    pub fn scan_into(
        &self,
        backend: &dyn VyreBackend,
        haystack: &[u8],
        region_starts: &[u32],
        region_base: u32,
        out: &mut Vec<u32>,
        scratch: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        self.scan_into_timed(backend, haystack, region_starts, region_base, out, scratch)?;
        Ok(())
    }

    /// Like [`scan_into`](Self::scan_into) but returns the dispatch's
    /// [`TimedDispatchResult`] so a consumer or benchmark can attribute the
    /// per-scan cost between the GPU kernel (`device_ns`) and host-side
    /// staging/readback (`wall_ns - device_ns`). The decoded per-region presence
    /// bitmap is written to `out` identically to [`scan_into`](Self::scan_into);
    /// the returned result's `outputs` are the same raw presence bytes already
    /// decoded into `out`.
    ///
    /// # Errors
    /// Same as [`scan_into`](Self::scan_into).
    pub fn scan_into_timed(
        &self,
        backend: &dyn VyreBackend,
        haystack: &[u8],
        region_starts: &[u32],
        region_base: u32,
        out: &mut Vec<u32>,
        scratch: &mut Vec<u8>,
    ) -> Result<TimedDispatchResult, BackendError> {
        out.clear();

        let region_count = u32::try_from(region_starts.len()).map_err(|_| {
            BackendError::new(
                "resident region-presence: region count exceeds u32 GPU ABI".to_string(),
            )
        })?;
        if region_count == 0 {
            return Err(BackendError::new(
                "resident region-presence: region_starts must be non-empty. Fix: pass one start offset per coalesced file, beginning with 0.".to_string(),
            ));
        }
        if region_starts[0] != 0 {
            return Err(BackendError::new(
                "resident region-presence: region_starts[0] must be 0 (the kernel binary-search lower bound). Fix: the first coalesced file must start at offset 0.".to_string(),
            ));
        }
        if region_count > self.max_regions {
            return Err(BackendError::new(format!(
                "resident region-presence batch has {region_count} regions but the session was prepared for at most {}. Fix: raise max_regions in prepare_resident_presence, or dispatch this batch through the per-batch-sized borrowed GpuLiteralSet::scan_presence_by_region (a larger cap would index past the resident presence buffer).",
                self.max_regions
            )));
        }

        let haystack_len = dispatch_io::scan_guard(
            haystack,
            "ResidentPresencePipeline::scan",
            dispatch_io::DEFAULT_MAX_SCAN_BYTES,
        )?;

        // (1) Stage the haystack into the resident buffer (real bytes only; the
        // kernel bounds its cursor with haystack_len so the stale tail is never
        // read).
        dispatch_io::pack_haystack_u32_into(haystack, scratch)?;
        if scratch.len() > self.haystack_capacity {
            return Err(BackendError::new(format!(
                "ResidentPresencePipeline haystack is {} packed byte(s) but the resident buffer holds {}. Fix: raise haystack_capacity_bytes in prepare_resident_presence or shard the haystack.",
                scratch.len(),
                self.haystack_capacity
            )));
        }
        backend.upload_resident_at(&self.haystack, 0, scratch)?;

        // (2) Zero the USED prefix of the resident presence buffer (binding 6 is
        // OR-accumulated by the kernel, so it must arrive zeroed). Rows beyond
        // region_count are never written (the kernel bounds the region index by
        // buf_len(region_starts)) and never read, so only the used prefix needs
        // clearing — the resident analogue of `ResidentRulePipeline`'s 4-byte
        // counter reset. Reusing `scratch` is safe: `upload_resident_at` copies the
        // source synchronously (wgpu `Queue::write_buffer` into the staging belt,
        // CUDA H2D memcpy), so the buffer is free to repurpose the instant the
        // haystack upload above returns.
        let used_words = (region_count as usize)
            .checked_mul(self.presence_words as usize)
            .ok_or_else(|| {
                BackendError::new(
                    "resident region-presence used-word count overflows host usize. Fix: lower the region count or shard the pattern set.".to_string(),
                )
            })?;
        let reset_bytes = used_words.checked_mul(U32_BYTES).ok_or_else(|| {
            BackendError::new(
                "resident region-presence presence-reset byte count overflows host usize. Fix: lower the region count or shard the pattern set.".to_string(),
            )
        })?;
        scratch.clear();
        scratch.resize(reset_bytes, 0);
        backend.upload_resident_at(&self.presence, 0, scratch)?;

        // (3) Stage the three per-scan control buffers. They MUST be resident, not
        // borrowed: the CUDA resident dispatch resolves every binding to a resident
        // handle and rejects a borrowed mix (`cuda_compiled_persistent_borrowed_resource`),
        // so an all-resident dispatch is the only form portable across wgpu AND CUDA
        // (a downstream consumer's backend). haystack_len and region_base are one u32 each.
        backend.upload_resident_at(&self.haystack_len_buf, 0, &haystack_len.to_le_bytes())?;
        backend.upload_resident_at(&self.region_base_buf, 0, &region_base.to_le_bytes())?;

        // region_starts is a FIXED `max_regions`-sized resident buffer so its
        // `buf_len` — the kernel's live region count — does not change with the
        // batch. The real starts fill [0, region_count); the tail
        // [region_count, max_regions) is padded with `u32::MAX`, a sentinel strictly
        // greater than any candidate position (positions are bounded by the scan
        // size << u32::MAX), so the region binary search never maps a hit to a
        // padding row. Those rows stay untouched and are never decoded — the result
        // for the real regions is identical to a `region_count`-length region_starts.
        // Reusing `scratch` is safe (synchronous upload copy, as above).
        scratch.clear();
        let region_starts_words = self.max_regions as usize;
        scratch.reserve(region_starts_words.saturating_mul(U32_BYTES));
        for &start in region_starts {
            scratch.extend_from_slice(&start.to_le_bytes());
        }
        for _ in (region_count as usize)..region_starts_words {
            scratch.extend_from_slice(&u32::MAX.to_le_bytes());
        }
        backend.upload_resident_at(&self.region_starts_buf, 0, scratch)?;

        // (4) Bind in program order — every binding resident (the CUDA all-resident
        // requirement; wgpu accepts resident bindings identically).
        let resources = [
            self.haystack.clone(),                // 0: haystack
            self.transitions.clone(),             // 1: transitions
            self.output_offsets.clone(),          // 2: output_offsets
            self.output_records.clone(),          // 3: output_records
            self.pattern_lengths.clone(),         // 4: pattern_lengths
            self.haystack_len_buf.clone(),        // 5: haystack_len
            self.presence.clone(),                // 6: presence (read_write)
            self.candidate_end_mask.clone(),      // 7: candidate_end_mask
            self.candidate_suffix2_mask.clone(),  // 8: candidate_suffix2_mask
            self.candidate_suffix3_bloom.clone(), // 9: candidate_suffix3_bloom
            self.region_starts_buf.clone(),       // 10: region_starts (padded)
            self.region_base_buf.clone(),         // 11: region_base
        ];
        debug_assert_eq!(resources.len(), PRESENCE_BY_REGION_BINDINGS);

        let config = dispatch_io::byte_scan_dispatch_config(haystack_len, self.workgroup_x);
        let timed = backend.dispatch_resident_timed(&self.program, &resources, &config)?;

        // The presence buffer is the program's only ReadWrite storage, returned at
        // output index 0 — identical decode to `scan_presence_by_region`.
        let presence_bytes = dispatch_io::try_output_bytes(
            &timed.outputs,
            0,
            "ResidentPresencePipeline presence buffer",
        )?;
        // The single region-presence wire decoder (shared with the sync / async /
        // prepared / fused paths in literal_set), filling the caller's `out`.
        decode_presence_words_into(presence_bytes, used_words, out);
        // Fail CLOSED on a short readback: a presence resource that returns fewer
        // than the used words would otherwise hand back a silently truncated bitmap
        // (some regions reported clean that were never scanned — Law 10).
        if out.len() != used_words {
            let returned = out.len();
            out.clear();
            return Err(BackendError::new(format!(
                "ResidentPresencePipeline presence readback returned {returned} u32 word(s) but the {region_count}-region scan needs {used_words}. Fix: ensure the backend reads back the full binding-6 presence resource."
            )));
        }
        Ok(timed)
    }

    /// Largest coalesced-file count this session's presence buffer was sized for.
    #[must_use]
    pub fn max_regions(&self) -> u32 {
        self.max_regions
    }

    /// Pattern count (bit width of each per-region presence row).
    #[must_use]
    pub fn pattern_count(&self) -> u32 {
        self.pattern_count
    }

    /// Presence bitmap `u32` words per region.
    #[must_use]
    pub fn presence_words(&self) -> u32 {
        self.presence_words
    }

    /// Padded byte capacity of the resident haystack buffer.
    #[must_use]
    pub fn haystack_capacity(&self) -> usize {
        self.haystack_capacity
    }

    /// Release every resident resource this session owns.
    ///
    /// Call this before the backend's device context is dropped to reclaim the
    /// resident allocations eagerly; otherwise they are reclaimed when the backend
    /// tears down. The session is consumed.
    ///
    /// # Errors
    /// Returns the first [`BackendError`] from freeing a resource; remaining
    /// resources are still attempted.
    pub fn free(self, backend: &dyn VyreBackend) -> Result<(), BackendError> {
        let mut first_err = None;
        for resource in [
            self.haystack,
            self.transitions,
            self.output_offsets,
            self.output_records,
            self.pattern_lengths,
            self.presence,
            self.candidate_end_mask,
            self.candidate_suffix2_mask,
            self.candidate_suffix3_bloom,
            self.haystack_len_buf,
            self.region_starts_buf,
            self.region_base_buf,
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
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::Mutex;
    use vyre::DispatchConfig as Config;
    use vyre_driver::TimedDispatchResult;

    // pattern_id order matches the integration corpus: key=0 .. api=7.
    const LITERALS: &[&[u8]] = &[
        b"key", b"token", b"secret", b"AKIA", b"ghp_", b"sk_live_", b"password", b"api",
    ];

    /// Mock backend that records resident traffic and returns a canned presence
    /// buffer, so the host orchestration (seven-table-upload-once, per-scan haystack
    /// staging + presence reset, 12-binding dispatch, decode) is validated without a
    /// GPU. Real GPU resident-vs-borrowed parity is asserted in the integration
    /// suite where a live wgpu backend is available. `VyreBackend` requires
    /// `Send + Sync`, so the counters use atomics / `Mutex`.
    struct MockResidentBackend {
        next_id: AtomicU64,
        /// (handle_id, byte_len) for every allocate_resident call.
        allocations: Mutex<Vec<(u64, usize)>>,
        /// Number of full uploads (immutable table uploads) seen.
        full_uploads: AtomicUsize,
        /// Number of ranged uploads (haystack stage + presence reset) seen.
        ranged_uploads: AtomicUsize,
        /// Byte lengths of every ranged upload, in order (haystack, reset, ...).
        ranged_upload_lens: Mutex<Vec<usize>>,
        /// Canned presence-buffer bytes returned at output index 0.
        presence_buffer: Vec<u8>,
    }

    impl MockResidentBackend {
        fn new(presence_buffer: Vec<u8>) -> Self {
            Self {
                next_id: AtomicU64::new(1),
                allocations: Mutex::new(Vec::new()),
                full_uploads: AtomicUsize::new(0),
                ranged_uploads: AtomicUsize::new(0),
                ranged_upload_lens: Mutex::new(Vec::new()),
                presence_buffer,
            }
        }
    }

    impl vyre::backend::private::Sealed for MockResidentBackend {}

    impl VyreBackend for MockResidentBackend {
        fn id(&self) -> &'static str {
            "mock-resident-presence"
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
            bytes: &[u8],
        ) -> Result<(), BackendError> {
            self.ranged_uploads.fetch_add(1, Ordering::Relaxed);
            self.ranged_upload_lens
                .lock()
                .expect("mock ranged-upload mutex")
                .push(bytes.len());
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
            assert_eq!(
                resources.len(),
                PRESENCE_BY_REGION_BINDINGS,
                "region-presence binds twelve buffers"
            );
            // EVERY binding must be resident — the CUDA resident dispatch rejects a
            // borrowed-resource mix, so no binding (not even the tiny per-scan
            // control buffers) may be Borrowed.
            for idx in 0..PRESENCE_BY_REGION_BINDINGS {
                assert!(
                    matches!(resources[idx], Resource::Resident(_)),
                    "binding {idx} must be resident (no borrowed mix in a resident dispatch)"
                );
            }
            assert!(
                config.grid_override.is_some(),
                "resident region-presence scan must supply a byte-scan grid override"
            );
            Ok(TimedDispatchResult {
                outputs: vec![self.presence_buffer.clone()],
                wall_ns: 0,
                device_ns: None,
                enqueue_ns: None,
                wait_ns: None,
            })
        }
    }

    /// Decode one single-word region row into the set of pattern ids whose bit is set.
    fn present_ids(word: u32, pattern_count: u32) -> BTreeSet<u32> {
        (0..pattern_count).filter(|&p| (word >> p) & 1 == 1).collect()
    }

    #[test]
    fn prepare_uploads_tables_once_then_scans_transfer_only_haystack_and_reset() {
        let matcher = GpuLiteralSet::compile(LITERALS);
        let pattern_count = LITERALS.len() as u32;
        assert_eq!(pattern_count, 8);
        // 8 patterns -> 1 presence word/region. max_regions = 4 -> capacity 4 words.
        // Canned presence: planted prefix [row0,row1,row2] + a stale 4th word the
        // 3-region decode must ignore.
        let row0 = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 7); // {key,token,secret,AKIA,api}
        let row1 = (1 << 4) | (1 << 5) | (1 << 6); // {ghp_,sk_live_,password}
        let row2 = 0u32; // {}
        let stale = 0xDEAD_BEEFu32;
        let mut canned = Vec::new();
        for w in [row0, row1, row2, stale] {
            canned.extend_from_slice(&w.to_le_bytes());
        }
        let backend = MockResidentBackend::new(canned);

        let session = matcher
            .prepare_resident_presence(&backend, 4096, 4)
            .expect("mock backend supports resident allocation");

        // Twelve resident allocations: haystack + 7 immutable tables + presence +
        // the three per-scan control buffers (haystack_len, region_starts, region_base).
        {
            let allocs = backend.allocations.lock().unwrap();
            assert_eq!(allocs.len(), 12, "haystack + 7 tables + presence + 3 controls");
            // presence (idx 8) = max_regions × words × 4 = 4 × 1 × 4 = 16 bytes.
            assert_eq!(allocs[8].1, 4 * 1 * U32_BYTES, "presence sized for max_regions");
            assert_eq!(allocs[9].1, U32_BYTES, "haystack_len control is one u32");
            assert_eq!(allocs[10].1, 4 * U32_BYTES, "region_starts sized for max_regions");
            assert_eq!(allocs[11].1, U32_BYTES, "region_base control is one u32");
        }
        // The seven immutable tables are uploaded exactly once, at prepare time; the
        // control buffers are staged per scan (ranged), not at prepare.
        assert_eq!(
            backend.full_uploads.load(Ordering::Relaxed),
            7,
            "seven immutable tables uploaded once each"
        );
        assert_eq!(backend.ranged_uploads.load(Ordering::Relaxed), 0);

        // A 3-region coalesced batch (regions at 0 / 4 / 9; first start == 0).
        let haystack = b"aaa\nbbbb\nccc\n";
        let region_starts = [0u32, 4, 9];
        let mut out = Vec::new();
        let mut scratch = Vec::new();
        for _ in 0..3 {
            session
                .scan_into(&backend, haystack, &region_starts, 0, &mut out, &mut scratch)
                .expect("resident region-presence scan decodes canned bitmap");
        }

        // Decode parity: the canned planted prefix surfaces; the stale 4th word is
        // never observed by the 3-region decode.
        assert_eq!(out, vec![row0, row1, row2], "3 regions × 1 word, stale tail ignored");
        assert_eq!(present_ids(out[0], pattern_count), BTreeSet::from([0, 1, 2, 3, 7]));
        assert_eq!(present_ids(out[1], pattern_count), BTreeSet::from([4, 5, 6]));
        assert_eq!(present_ids(out[2], pattern_count), BTreeSet::new());

        // No further full uploads after prepare; each scan does exactly FIVE ranged
        // uploads (haystack stage, presence reset, haystack_len, region_base,
        // region_starts) — the immutable tables never move again.
        assert_eq!(
            backend.full_uploads.load(Ordering::Relaxed),
            7,
            "immutable tables re-uploaded mid-loop"
        );
        assert_eq!(
            backend.ranged_uploads.load(Ordering::Relaxed),
            15,
            "3 scans × 5 ranged uploads (haystack, presence reset, haystack_len, region_base, region_starts)"
        );
        // Per-scan upload order is [haystack, reset, haystack_len, region_base,
        // region_starts]. The presence reset (2nd of each group of 5) uploads exactly
        // used_words × 4 = 3 × 1 × 4 = 12 bytes; region_starts (5th) uploads the full
        // padded max_regions × 4 = 16 bytes regardless of the 3-region batch.
        let lens = backend.ranged_upload_lens.lock().unwrap();
        let nth_of_each_scan = |offset: usize| -> Vec<usize> {
            lens.iter().skip(offset).step_by(5).copied().collect()
        };
        assert_eq!(
            nth_of_each_scan(1),
            vec![12, 12, 12],
            "each presence reset zeroes only the 3-region used prefix"
        );
        assert_eq!(
            nth_of_each_scan(2),
            vec![U32_BYTES, U32_BYTES, U32_BYTES],
            "haystack_len control is one u32 per scan"
        );
        assert_eq!(
            nth_of_each_scan(3),
            vec![U32_BYTES, U32_BYTES, U32_BYTES],
            "region_base control is one u32 per scan"
        );
        assert_eq!(
            nth_of_each_scan(4),
            vec![4 * U32_BYTES, 4 * U32_BYTES, 4 * U32_BYTES],
            "region_starts is uploaded padded to the full max_regions width every scan"
        );
    }

    #[test]
    fn scan_rejects_region_count_over_the_max_regions_cap() {
        let matcher = GpuLiteralSet::compile(LITERALS);
        // Canned presence is irrelevant; the cap guard fires before dispatch.
        let backend = MockResidentBackend::new(vec![0u8; 4]);
        let session = matcher
            .prepare_resident_presence(&backend, 4096, 2)
            .expect("prepare with a 2-region cap");

        let haystack = b"a\nb\nc\n";
        let region_starts = [0u32, 2, 4]; // 3 regions > cap of 2
        let mut out = vec![999];
        let mut scratch = Vec::new();
        let err = session
            .scan_into(&backend, haystack, &region_starts, 0, &mut out, &mut scratch)
            .expect_err("a batch over the resident region cap must error, not truncate");
        assert!(
            err.to_string().contains("session was prepared for at most 2") && out.is_empty(),
            "cap error must name the limit and expose no partial bitmap: {err}"
        );
        // The over-cap batch must never reach the device.
        assert_eq!(
            backend.ranged_uploads.load(Ordering::Relaxed),
            0,
            "rejected batch must not stage any resident upload"
        );
    }

    #[test]
    fn scan_rejects_haystack_larger_than_resident_capacity() {
        let matcher = GpuLiteralSet::compile(LITERALS);
        let backend = MockResidentBackend::new(vec![0u8; 4]);
        let session = matcher
            .prepare_resident_presence(&backend, 8, 4)
            .expect("prepare with an 8-byte haystack capacity");

        let mut out = Vec::new();
        let mut scratch = Vec::new();
        let region_starts = [0u32];
        let err = session
            .scan_into(&backend, &[b'a'; 64], &region_starts, 0, &mut out, &mut scratch)
            .expect_err("64-byte haystack must not fit an 8-byte resident buffer");
        assert!(
            err.to_string().contains("resident buffer holds") && out.is_empty(),
            "capacity error must name the limit and expose no stale bitmap: {err}"
        );
    }

    #[test]
    fn prepare_rejects_zero_max_regions() {
        let matcher = GpuLiteralSet::compile(LITERALS);
        let backend = MockResidentBackend::new(vec![0u8; 4]);
        let err = matcher
            .prepare_resident_presence(&backend, 4096, 0)
            .expect_err("max_regions = 0 cannot size the presence buffer");
        assert!(
            err.to_string().contains("max_regions must be >= 1"),
            "zero-cap error must explain the cause: {err}"
        );
    }
}
