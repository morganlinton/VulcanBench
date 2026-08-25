//! Mega-scan integrator.
//!
//! Fuses the G-stack innovations into one `RulePipeline` that program-analysis consumer
//! dispatches. Right now the integrator wires G1 (subgroup-cooperative
//! NFA scan) end-to-end. As G2-G10 land their composition hooks here,
//! keeping one authoritative entry point for every scan configuration.
//!
//! # Why a single entry point
//!
//! Each innovation has its own buffer contracts (lane-major NFA
//! transition tables, CHD perfect-hash buckets, persistent-engine
//! work queues, etc.). Attempting to wire those inside program-analysis consumer would
//! push backend-specific knowledge into the language compiler  -
//! exactly the coupling vyre's layer boundaries exist to prevent.
//! `RulePipeline::new` holds the composition rules; callers hand in
//! patterns + input, the integrator returns a ready-to-dispatch
//! `Program` plus the host-side bit-tables the Program expects to
//! find at its declared storage buffers.

use vyre::VyreBackend;
use vyre_foundation::ir::Program;
use vyre_foundation::match_result::Match;

use super::nfa;

const NFA_LANES: usize = vyre_primitives::nfa::subgroup_nfa::LANES_PER_SUBGROUP;

/// A ready-to-dispatch pipeline produced by the integrator.
#[derive(Debug, Clone)]
pub struct RulePipeline {
    /// GPU-resident Program. Dispatch with the pattern plan's
    /// workgroup configuration.
    pub program: Program,
    /// Lane-major transition table, sized
    /// `num_states × 256 × LANES_PER_SUBGROUP` u32s. Upload to the
    /// `nfa_transition` storage buffer.
    pub transition_table: Vec<u32>,
    /// Lane-major epsilon table, sized
    /// `num_states × LANES_PER_SUBGROUP` u32s. Upload to the
    /// `nfa_epsilon` storage buffer.
    pub epsilon_table: Vec<u32>,
    /// Compiled NFA plan (accept states, num_states, input length).
    pub plan: nfa::NfaPlan,
}

impl RulePipeline {
    /// Dispatch this pipeline against `haystack` using the provided
    /// `backend`, returning up to `max_matches` matches.
    ///
    /// This is the regex-multimatch counterpart of
    /// [`crate::scan::GpuLiteralSet::scan`]  -  same backend trait,
    /// same hit-buffer encoding (slot 0 = atomic counter, then triples
    /// of `(pattern_id, start, end)`), so callers can swap the two
    /// matchers without changing post-processing code.
    ///
    /// Equivalent to [`Self::scan_bounded`] with `max_scan_bytes =
    /// u32::MAX` - every workgroup walks to the end of the haystack
    /// (O(N²) total work). Use [`Self::scan_bounded`] when the longest
    /// possible match is known to bound per-workgroup work and make
    /// the kernel O(N × max_scan_bytes).
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] on dispatch or readback failure.
    /// Returns an error wrapping the message
    /// `"haystack length exceeds u32 capacity"` when `haystack.len()`
    /// cannot be encoded as `u32`  -  split the input first.
    pub fn scan<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        max_matches: u32,
    ) -> Result<Vec<Match>, vyre::BackendError> {
        let mut matches = Vec::new();
        self.scan_into(backend, haystack, max_matches, &mut matches)?;
        Ok(matches)
    }

    /// Dispatch this pipeline with a per-workgroup cursor cap. Each
    /// workgroup walks bytes from its `WorkgroupId(0)` start to
    /// `min(haystack_len, start + max_scan_bytes)`. Returns up to
    /// `max_matches` matches.
    ///
    /// Pass the longest possible match length over the pipeline's
    /// pattern set as `max_scan_bytes` to drop per-shard cost from
    /// O(N²) (every workgroup scans to end-of-haystack) to O(N ×
    /// max_scan_bytes). For bounded detector regexes that bound
    /// is ~80-200 bytes; the resulting 62 MiB-shard cost drops from
    /// ~30 s to a few milliseconds.
    ///
    /// # Errors
    /// Same as [`Self::scan`].
    pub fn scan_bounded<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        max_matches: u32,
        max_scan_bytes: u32,
    ) -> Result<Vec<Match>, vyre::BackendError> {
        let mut matches = Vec::new();
        self.scan_bounded_into(backend, haystack, max_matches, max_scan_bytes, &mut matches)?;
        Ok(matches)
    }

    /// Dispatch this pipeline and decode matches into caller-owned scratch.
    ///
    /// This removes the per-dispatch result-vector allocation from hot scan
    /// loops while preserving the exact wire layout and sorted output contract
    /// of [`Self::scan`].
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] on dispatch or readback failure.
    pub fn scan_into<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        max_matches: u32,
        matches: &mut Vec<Match>,
    ) -> Result<(), vyre::BackendError> {
        self.scan_bounded_into(backend, haystack, max_matches, u32::MAX, matches)
    }

    /// Per-workgroup-bounded counterpart of [`Self::scan_into`]. See
    /// [`Self::scan_bounded`] for the bound's semantics.
    ///
    /// # Errors
    /// Same as [`Self::scan_into`].
    pub fn scan_bounded_into<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        max_matches: u32,
        max_scan_bytes: u32,
        matches: &mut Vec<Match>,
    ) -> Result<(), vyre::BackendError> {
        let mut scratch = crate::scan::dispatch_io::ScanDispatchScratch::default();
        self.scan_bounded_into_with_scratch(
            backend,
            haystack,
            max_matches,
            max_scan_bytes,
            matches,
            &mut scratch,
        )
    }

    /// Per-workgroup-bounded scan that reuses caller-owned match and byte
    /// staging scratch.
    ///
    /// This is the hot-loop API for regex/NFA scans: `matches` reuses decoded
    /// match storage, `scratch.haystack_bytes` reuses packed haystack bytes, and
    /// `scratch.hit_bytes` reuses the zeroed hit buffer.
    ///
    /// # Errors
    /// Same as [`Self::scan_bounded_into`].
    pub fn scan_bounded_into_with_scratch<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        max_matches: u32,
        max_scan_bytes: u32,
        matches: &mut Vec<Match>,
        scratch: &mut crate::scan::dispatch_io::ScanDispatchScratch,
    ) -> Result<(), vyre::BackendError> {
        use crate::scan::dispatch_io;

        matches.clear();
        let haystack_len = dispatch_io::scan_guard(
            haystack,
            "RulePipeline::scan",
            dispatch_io::DEFAULT_MAX_SCAN_BYTES,
        )?;

        // Buffer order matches the BufferDecl declarations in
        // `nfa::nfa_scan`: input, nfa_transition, nfa_epsilon, hits,
        // nfa_haystack_len, nfa_max_scan_bytes. The hit buffer
        // pre-allocates `max_matches * 3 + 1` u32 slots (slot 0 =
        // atomic counter, then triples). `nfa_haystack_len` is a 1-u32
        // input the kernel reads at runtime so a single compiled
        // program services every haystack size from zero up to its
        // declared capacity. `nfa_max_scan_bytes` caps each workgroup's
        // cursor walk so the kernel is O(N × bound) instead of O(N²).
        zeroed_hit_buffer_into(max_matches, &mut scratch.hit_bytes)?;
        dispatch_io::pack_haystack_u32_into(haystack, &mut scratch.haystack_bytes)?;
        let hit_bytes = scratch.hit_bytes.as_slice();
        let haystack_bytes = scratch.haystack_bytes.as_slice();
        let transition_bytes = dispatch_io::u32_words_as_le_bytes(&self.transition_table);
        let epsilon_bytes = dispatch_io::u32_words_as_le_bytes(&self.epsilon_table);
        let haystack_len_bytes = haystack_len.to_le_bytes();
        let max_scan_bytes_bytes = max_scan_bytes.to_le_bytes();

        let config = dispatch_io::candidate_start_dispatch_config(haystack_len);

        let borrowed_inputs: smallvec::SmallVec<[&[u8]; 6]> = [
            haystack_bytes,
            transition_bytes.as_ref(),
            epsilon_bytes.as_ref(),
            hit_bytes,
            haystack_len_bytes.as_slice(),
            max_scan_bytes_bytes.as_slice(),
        ]
        .into_iter()
        .collect();
        let outputs = backend.dispatch_borrowed(&self.program, &borrowed_inputs, &config)?;

        // The hit buffer is the only ReadWrite storage in the program;
        // backends return outputs in declaration order, so it lives at
        // index 0 of `outputs`.
        let hit_bytes = dispatch_io::try_output_bytes(&outputs, 0, "RulePipeline hit buffer")?;
        let count = dispatch_io::try_read_u32_prefix(hit_bytes, "RulePipeline hit buffer")?;
        // Triples start at byte 4 (after the atomic counter). The counter is an
        // atomic incremented for every match found — including matches past slot
        // `max_matches` the kernel could not write — so a count over the cap means
        // matches were dropped. Fail closed rather than silently decode the
        // truncated prefix (Law 10).
        dispatch_io::try_unpack_match_triples_capped_into(
            &hit_bytes[4..],
            count,
            max_matches,
            "RulePipeline hit buffer",
            matches,
        )?;
        Ok(())
    }

    /// Compute matches against `haystack` on the CPU using the same NFA
    /// the GPU program runs. Mirrors [`super::GpuLiteralSet::reference_scan`]
    ///  -  same `Match` type, same sort, so any consumer can write a
    /// single parity test that swaps backends and asserts equality.
    ///
    /// This is intentionally O(n × patterns)  -  it is only meant for
    /// parity / debugging, not production scanning.
    ///
    /// # Panics
    ///
    /// Aborts when the CPU stepper cannot honor the `u32` match ABI the GPU
    /// path uses (a haystack longer than `u32::MAX` bytes). This is a LOUD
    /// failure on purpose: an empty `Vec<Match>` is indistinguishable from "no
    /// secrets here", so swallowing a scan failure into `[]` would be a silent
    /// recall lie — a >4 GiB haystack would be reported clean (Law 10).
    /// Callers that must handle an over-`u32` haystack without unwinding use
    /// the fallible [`Self::try_reference_scan`] instead.
    #[must_use]
    pub fn reference_scan(&self, haystack: &[u8]) -> Vec<Match> {
        match self.try_reference_scan(haystack) {
            Ok(matches) => matches,
            Err(error) => {
                // Returning an empty match set would be indistinguishable from
                // "no secrets here" — a total recall-loss silent fallback
                // (Law 10). Fail closed instead. Callers that must handle a
                // >u32 haystack without unwinding call try_reference_scan.
                panic!(
                    "vyre-libs RulePipeline::reference_scan cannot honor the u32 match ABI for this haystack: {error} — \
                     returning an empty match set would silently report the input as clean; \
                     use try_reference_scan and split the haystack below u32::MAX bytes."
                )
            }
        }
    }

    /// Fallible CPU parity scan.
    ///
    /// # Errors
    ///
    /// Returns [`vyre::BackendError`] when haystack positions cannot fit the
    /// same `u32` match ABI used by the GPU path.
    pub fn try_reference_scan(&self, haystack: &[u8]) -> Result<Vec<Match>, vyre::BackendError> {
        let mut results = Vec::new();
        self.try_reference_scan_into(haystack, &mut results)?;
        Ok(results)
    }

    /// CPU parity scan into caller-owned result storage.
    ///
    /// The NFA state words are stack-backed fixed arrays, so the parity oracle
    /// no longer allocates two subgroup vectors for every `(start, cursor)`
    /// pair while still mirroring the GPU transition-table semantics.
    ///
    /// # Errors
    ///
    /// Returns [`vyre::BackendError`] when haystack positions cannot fit the
    /// same `u32` match ABI used by the GPU path.
    pub fn try_reference_scan_into(
        &self,
        haystack: &[u8],
        results: &mut Vec<Match>,
    ) -> Result<(), vyre::BackendError> {
        crate::scan::dispatch_io::scan_guard(haystack, "RulePipeline::reference_scan", u32::MAX)?;
        results.clear();
        for start in 0..haystack.len() {
            let start_u32 = u32::try_from(start).map_err(|_| {
                vyre::BackendError::new(
                    "RulePipeline::reference_scan start offset exceeds u32 capacity. Fix: split the haystack before parity scanning.",
                )
            })?;
            let mut state = [0_u32; NFA_LANES];
            let mut next = [0_u32; NFA_LANES];
            state[0] = 1;
            // Regex-compiled plans wire the shared entry state to each pattern's start via
            // ε-edges (and `*`/`?`/alternation add more). Close ε from the seed before the
            // first byte, mirroring the GPU program's initial ε loop; without this the walk
            // never leaves the entry state and reports zero matches for any ε-bearing NFA.
            close_epsilon(&mut state, &self.epsilon_table, self.plan.num_states as usize);
            for (cursor, &byte) in haystack.iter().enumerate().skip(start) {
                next.fill(0);
                for (lane, &peer) in state.iter().enumerate() {
                    for bit in 0..32 {
                        if (peer >> bit) & 1 == 0 {
                            continue;
                        }
                        let src_state = lane * 32 + bit;
                        if src_state >= self.plan.num_states as usize {
                            continue;
                        }
                        let base = src_state * 256 * NFA_LANES + (byte as usize) * NFA_LANES;
                        for (dst_lane, slot) in next.iter_mut().enumerate() {
                            *slot |= self.transition_table[base + dst_lane];
                        }
                    }
                }
                std::mem::swap(&mut state, &mut next);
                // ε-close the post-transition state set (same fixpoint the GPU eps loop runs).
                close_epsilon(&mut state, &self.epsilon_table, self.plan.num_states as usize);
                for (&accept_state, &(pattern_id, _pattern_len)) in self
                    .plan
                    .accept_state_ids
                    .iter()
                    .zip(&self.plan.accept_states)
                {
                    let lane = (accept_state / 32) as usize;
                    let bit = accept_state % 32;
                    if lane < state.len() && (state[lane] & (1_u32 << bit)) != 0 {
                        let end_u32 = u32::try_from(cursor + 1).map_err(|_| {
                            vyre::BackendError::new(
                                "RulePipeline::reference_scan end offset exceeds u32 capacity. Fix: split the haystack before parity scanning.",
                            )
                        })?;
                        results.push(Match::new(pattern_id, start_u32, end_u32));
                    }
                }
            }
        }
        results.sort_unstable();
        Ok(())
    }
}

/// Epsilon-close an NFA state set in place: repeatedly OR in every state reachable by an
/// ε-edge until fixpoint. Lane-major `epsilon_table` layout is `[num_states × LANES]`
/// (`epsilon_table[src * LANES + dst_lane]` holds the destination bits `dst_lane` owns,
/// reachable from `src`), mirroring the byte `transition_table` minus the 256-byte axis.
///
/// No-op when there are no ε-edges (empty or all-zero table), so literal NFAs — whose
/// matches never depend on ε — are unaffected. Bounded to `num_states` iterations: each
/// pass advances the ε-frontier by one hop and OR is monotone, so the closure is complete.
fn close_epsilon(state: &mut [u32; NFA_LANES], epsilon_table: &[u32], num_states: usize) {
    if epsilon_table.is_empty() || num_states == 0 {
        return;
    }
    for _ in 0..num_states {
        let snapshot = *state;
        for (lane, &peer) in snapshot.iter().enumerate() {
            if peer == 0 {
                continue;
            }
            for bit in 0..32 {
                if (peer >> bit) & 1 == 0 {
                    continue;
                }
                let src_state = lane * 32 + bit;
                if src_state >= num_states {
                    continue;
                }
                let base = src_state * NFA_LANES;
                for (dst_lane, slot) in state.iter_mut().enumerate() {
                    if let Some(&bits) = epsilon_table.get(base + dst_lane) {
                        *slot |= bits;
                    }
                }
            }
        }
        if *state == snapshot {
            break; // fixpoint reached
        }
    }
}

/// Integrator entry point. Takes a pattern set + the input length the
/// pipeline will be dispatched against and returns everything program-analysis consumer
/// needs to issue a single dispatch.
///
/// Additional G-stack options land here as optional parameters  -
/// callers that don't opt in keep the current behaviour.
#[must_use]
pub fn build(patterns: &[&str], input_buf: &str, hit_buf: &str, input_len: u32) -> RulePipeline {
    let plan = nfa::compile(patterns).for_input_len(input_len);
    let program = nfa::nfa_scan(patterns, input_buf, hit_buf, input_len);
    let transition_table = nfa::build_transition_table(patterns);
    let epsilon_table = nfa::build_epsilon_table(patterns);
    RulePipeline {
        program,
        transition_table,
        epsilon_table,
        plan,
    }
}

pub(crate) fn hit_buffer_byte_len(max_matches: u32) -> Result<usize, vyre::BackendError> {
    let match_words = usize::try_from(max_matches)
        .map_err(|_| {
            vyre::BackendError::new(
                "RulePipeline::scan max_matches exceeds host usize capacity. Fix: reduce max_matches or shard the scan.",
            )
        })?
        .checked_mul(3)
        .and_then(|words| words.checked_add(1))
        .ok_or_else(|| {
            vyre::BackendError::new(
                "RulePipeline::scan hit-buffer word count overflowed. Fix: reduce max_matches or shard the scan.",
            )
        })?;
    match_words.checked_mul(4).ok_or_else(|| {
        vyre::BackendError::new(
            "RulePipeline::scan hit-buffer byte count overflowed. Fix: reduce max_matches or shard the scan.",
        )
    })
}

fn zeroed_hit_buffer(max_matches: u32) -> Result<Vec<u8>, vyre::BackendError> {
    let byte_len = hit_buffer_byte_len(max_matches)?;
    let mut bytes = Vec::new();
    zeroed_hit_buffer_into(max_matches, &mut bytes)?;
    debug_assert_eq!(bytes.len(), byte_len);
    Ok(bytes)
}

fn zeroed_hit_buffer_into(max_matches: u32, bytes: &mut Vec<u8>) -> Result<(), vyre::BackendError> {
    let byte_len = hit_buffer_byte_len(max_matches)?;
    bytes.clear();
    vyre_foundation::allocation::try_reserve_vec_to_capacity(bytes, byte_len).map_err(
        |source| {
            vyre::BackendError::new(format!(
                "RulePipeline::scan could not reserve {byte_len} hit-buffer byte(s): {source}. Fix: lower max_matches or shard the scan."
            ))
        },
    )?;
    bytes.resize(byte_len, 0);
    Ok(())
}

fn reserve_wire_vec<T>(
    vec: &mut Vec<T>,
    requested: usize,
    field: &'static str,
) -> Result<(), PipelineWireError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(vec, requested).map_err(|source| {
        PipelineWireError::StorageReserveFailed {
            field,
            requested,
            message: source.to_string(),
        }
    })
}

const PIPELINE_WIRE_MAGIC: &[u8; 4] = b"VRPL";
// V4: nfa_scan added the `nfa_max_scan_bytes` storage buffer so the
// per-workgroup cursor cap is read from a 1-u32 input. Old V3 blobs
// encode a Program without that binding; decoding one and
// re-dispatching would crash on a missing-binding lookup. Bumping
// the version forces every cache consumer to re-compile against the
// V4 program shape.
//
// V3: nfa_scan added the `nfa_haystack_len` storage buffer so the
// runtime cursor bound is read from a 1-u32 input instead of baked
// into the compiled program. Old V2 blobs encode a Program without
// that binding; decoding one and re-dispatching would crash on a
// missing-binding lookup.
const PIPELINE_WIRE_VERSION: u32 = 4;

/// Errors returned by [`RulePipeline::from_bytes`]. Mirrors the layered
/// error pattern of `LiteralSetWireError`  -  outer envelope failures
/// forward to `WireFraming`, inner failures keep typed variants.
#[derive(Debug)]
#[non_exhaustive]
pub enum PipelineWireError {
    /// Outer envelope (magic / version / section length) was rejected.
    WireFraming(vyre_foundation::serial::envelope::EnvelopeError),
    /// Nested vyre IR `Program` blob was rejected.
    InvalidProgram(String),
    /// One of the four `u32`-array sections had the wrong length to be
    /// consistent with the recorded `num_states` header field. Stale
    /// blob  -  recompile.
    ShapeMismatch {
        /// Static description of which section's length cross-check
        /// failed.
        reason: &'static str,
    },
    /// Serialization scratch storage could not be reserved.
    StorageReserveFailed {
        /// Scratch vector being reserved.
        field: &'static str,
        /// Requested target capacity.
        requested: usize,
        /// Allocator failure details.
        message: String,
    },
}

impl std::fmt::Display for PipelineWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WireFraming(e) => write!(f, "RulePipeline wire envelope: {e}"),
            Self::InvalidProgram(msg) => {
                write!(f, "RulePipeline wire blob has invalid Program: {msg}")
            }
            Self::ShapeMismatch { reason } => {
                write!(f, "RulePipeline wire blob shape mismatch: {reason}")
            }
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "RulePipeline wire serialization could not reserve {requested} {field} slot(s): {message}. Fix: shard the pattern pipeline before serialization."
            ),
        }
    }
}

impl std::error::Error for PipelineWireError {}

impl RulePipeline {
    /// Serialize this pipeline into a self-describing binary blob
    /// suitable for on-disk caching. Built on the shared
    /// `vyre_foundation::serial::envelope` primitive  -  any future cache
    /// consumer reuses the same framing without re-implementing
    /// magic / version / truncation handling.
    ///
    /// Sections, in order:
    ///   - `u32`     : `plan.num_states`
    ///   - `u32`     : `plan.input_len`
    ///   - section 0 : vyre `Program::to_bytes` payload
    ///   - words 1   : `transition_table` (lane-major)
    ///   - words 2   : `epsilon_table` (lane-major)
    ///   - words 3   : `plan.accept_states` flattened as
    ///                 `[pid_0, len_0, pid_1, len_1, …]`
    ///   - words 4   : `plan.accept_state_ids`
    ///   - words 5   : accept anchor flags, one bitset word per accept
    ///                 (`bit0=start`, `bit1=end`)
    ///
    /// # Errors
    /// Returns [`PipelineWireError::WireFraming`] if any section
    /// exceeds the envelope's `u32` length-prefix capacity.
    pub fn to_bytes(&self) -> Result<Vec<u8>, PipelineWireError> {
        let mut w = vyre_foundation::serial::envelope::WireWriter::new(
            PIPELINE_WIRE_MAGIC,
            PIPELINE_WIRE_VERSION,
        );
        w.write_u32(self.plan.num_states);
        w.write_u32(self.plan.input_len);
        w.write_section(&self.program.to_bytes())
            .map_err(PipelineWireError::WireFraming)?;
        w.write_words(&self.transition_table)
            .map_err(PipelineWireError::WireFraming)?;
        w.write_words(&self.epsilon_table)
            .map_err(PipelineWireError::WireFraming)?;
        // Flatten accept_states tuples into a flat u32 array; each
        // accept-state contributes two consecutive words.
        let accept_flat_words = self.plan.accept_states.len().checked_mul(2).ok_or(
            PipelineWireError::ShapeMismatch {
                reason: "accept_states length overflows flattened word count",
            },
        )?;
        let mut accept_flat: Vec<u32> = Vec::new();
        reserve_wire_vec(&mut accept_flat, accept_flat_words, "accept state word")?;
        for &(pid, len) in &self.plan.accept_states {
            accept_flat.push(pid);
            accept_flat.push(len);
        }
        w.write_words(&accept_flat)
            .map_err(PipelineWireError::WireFraming)?;
        w.write_words(&self.plan.accept_state_ids)
            .map_err(PipelineWireError::WireFraming)?;
        let mut anchor_flags: Vec<u32> = Vec::new();
        reserve_wire_vec(
            &mut anchor_flags,
            self.plan.accept_states.len(),
            "accept anchor flag",
        )?;
        for idx in 0..self.plan.accept_states.len() {
            let mut flags = 0u32;
            if self
                .plan
                .accept_start_anchored
                .get(idx)
                .copied()
                .unwrap_or(false)
            {
                flags |= 1;
            }
            if self
                .plan
                .accept_end_anchored
                .get(idx)
                .copied()
                .unwrap_or(false)
            {
                flags |= 2;
            }
            anchor_flags.push(flags);
        }
        w.write_words(&anchor_flags)
            .map_err(PipelineWireError::WireFraming)?;
        Ok(w.into_bytes())
    }

    /// Decode a `RulePipeline` from a blob produced by
    /// [`Self::to_bytes`].
    ///
    /// # Errors
    /// Returns [`PipelineWireError`] when the envelope rejects the
    /// outer header, the nested `Program` is invalid, or the section
    /// shapes don't match the recorded `num_states`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PipelineWireError> {
        let mut r = vyre_foundation::serial::envelope::WireReader::new(
            bytes,
            PIPELINE_WIRE_MAGIC,
            PIPELINE_WIRE_VERSION,
        )
        .map_err(PipelineWireError::WireFraming)?;

        let num_states = r.read_u32().map_err(PipelineWireError::WireFraming)?;
        let input_len = r.read_u32().map_err(PipelineWireError::WireFraming)?;

        let program_bytes = r.read_section().map_err(PipelineWireError::WireFraming)?;
        let program = vyre_foundation::ir::Program::from_bytes(program_bytes)
            .map_err(|e| PipelineWireError::InvalidProgram(format!("{e}")))?;

        let transition_table = r.read_words().map_err(PipelineWireError::WireFraming)?;
        let epsilon_table = r.read_words().map_err(PipelineWireError::WireFraming)?;
        let accept_flat = r.read_words().map_err(PipelineWireError::WireFraming)?;
        let accept_state_ids = r.read_words().map_err(PipelineWireError::WireFraming)?;
        let anchor_flags = r.read_words().map_err(PipelineWireError::WireFraming)?;

        if accept_flat.len() % 2 != 0 {
            return Err(PipelineWireError::ShapeMismatch {
                reason: "accept_states array length is not even",
            });
        }
        let accept_states: Vec<(u32, u32)> =
            accept_flat.chunks_exact(2).map(|w| (w[0], w[1])).collect();
        if accept_state_ids.len() != accept_states.len() {
            return Err(PipelineWireError::ShapeMismatch {
                reason: "accept_state_ids length disagrees with accept_states length",
            });
        }
        if anchor_flags.len() != accept_states.len() {
            return Err(PipelineWireError::ShapeMismatch {
                reason: "accept anchor flag length disagrees with accept_states length",
            });
        }
        let accept_start_anchored = anchor_flags.iter().map(|flags| flags & 1 != 0).collect();
        let accept_end_anchored = anchor_flags.iter().map(|flags| flags & 2 != 0).collect();

        Ok(RulePipeline {
            program,
            transition_table,
            epsilon_table,
            plan: nfa::NfaPlan {
                num_states,
                input_len,
                accept_states,
                accept_state_ids,
                accept_start_anchored,
                accept_end_anchored,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct RuleReadbackBackend {
        outputs: Vec<Vec<u8>>,
    }

    impl vyre::backend::private::Sealed for RuleReadbackBackend {}

    impl VyreBackend for RuleReadbackBackend {
        fn id(&self) -> &'static str {
            "rule-readback-test"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &vyre::DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, vyre::BackendError> {
            Ok(self.outputs.clone())
        }

        fn dispatch_borrowed(
            &self,
            _program: &Program,
            _inputs: &[&[u8]],
            _config: &vyre::DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, vyre::BackendError> {
            Ok(self.outputs.clone())
        }
    }

    fn hit_buffer_bytes(count: u32, triples: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + triples.len());
        bytes.extend_from_slice(&count.to_le_bytes());
        bytes.extend_from_slice(triples);
        bytes
    }

    fn match_triple_bytes(pattern_id: u32, start: u32, end: u32) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12);
        bytes.extend_from_slice(&pattern_id.to_le_bytes());
        bytes.extend_from_slice(&start.to_le_bytes());
        bytes.extend_from_slice(&end.to_le_bytes());
        bytes
    }

    #[test]
    fn scan_fails_closed_when_kernel_count_exceeds_cap() {
        // Law 10 regression at the RulePipeline decode call site: the kernel's
        // atomic counter reports 9 hits into a buffer sized for the cap of 4
        // triples. The capped decode must error (naming the overflow and the 5
        // dropped matches) instead of silently returning the truncated 4 — a
        // false negative the caller could not detect.
        let pipe = build(&["ab"], "input", "hits", 16);
        let triples: Vec<u8> = (0..4u32)
            .flat_map(|i| match_triple_bytes(0, i, i + 2))
            .collect();
        let backend = RuleReadbackBackend {
            outputs: vec![hit_buffer_bytes(9, &triples)],
        };
        let mut matches = vec![Match::new(7, 7, 7)];
        let err = pipe
            .scan_into(&backend, b"abab", 4, &mut matches)
            .expect_err("count 9 over cap 4 must fail closed, not truncate to 4");
        let msg = err.to_string();
        assert!(
            msg.contains("RulePipeline hit buffer")
                && msg.contains("exceeds the output-buffer cap 4")
                && msg.contains("drop 5 match(es)")
                && matches.is_empty(),
            "RulePipeline must surface the dropped-match overflow and expose no partial set: {msg}"
        );
    }

    #[test]
    fn scan_decodes_exact_set_within_cap() {
        // The positive twin: a count within the cap decodes the real triples
        // verbatim (assert concrete values, never is_empty), proving the guard
        // does not reject legitimate in-bounds results.
        let pipe = build(&["ab"], "input", "hits", 16);
        let mut triples = Vec::new();
        triples.extend_from_slice(&match_triple_bytes(0, 1, 3));
        triples.extend_from_slice(&match_triple_bytes(1, 2, 4));
        let backend = RuleReadbackBackend {
            outputs: vec![hit_buffer_bytes(2, &triples)],
        };
        let mut matches = vec![Match::new(9, 9, 9)];
        pipe.scan_into(&backend, b"abab", 8, &mut matches)
            .expect("count 2 within cap 8 must decode");
        assert_eq!(matches, vec![Match::new(0, 1, 3), Match::new(1, 2, 4)]);
    }

    #[test]
    fn integrator_returns_primitive_compatible_tables() {
        let pipe = build(&["abc"], "input", "hits", 16);
        let plan = nfa::compile(&["abc"]);
        let expected_trans_len = (plan.num_states as usize)
            * 256
            * vyre_primitives::nfa::subgroup_nfa::LANES_PER_SUBGROUP;
        let expected_eps_len =
            (plan.num_states as usize) * vyre_primitives::nfa::subgroup_nfa::LANES_PER_SUBGROUP;
        assert_eq!(pipe.transition_table.len(), expected_trans_len);
        assert_eq!(pipe.epsilon_table.len(), expected_eps_len);
    }

    #[test]
    fn integrator_plan_matches_compile() {
        let pipe = build(&["ab", "cd"], "input", "hits", 8);
        assert_eq!(pipe.plan.num_states, 5);
        assert_eq!(pipe.plan.input_len, 8);
        assert_eq!(pipe.plan.accept_states.len(), 2);
    }

    #[test]
    fn rule_pipeline_reference_scan_into_matches_owned_scan_and_reuses_scratch() {
        let pipe = build(&["ab", "bc"], "input", "hits", 16);
        let owned = pipe.reference_scan(b"zabc");
        let mut scratch = Vec::with_capacity(16);
        let retained_capacity = scratch.capacity();

        pipe.try_reference_scan_into(b"zabc", &mut scratch)
            .expect("Fix: RulePipeline CPU oracle should scan small haystacks");

        assert_eq!(scratch, owned);
        assert!(scratch.capacity() >= retained_capacity);
        assert_eq!(scratch, vec![Match::new(0, 1, 3), Match::new(1, 2, 4)]);
    }

    #[test]
    fn rule_pipeline_reference_scan_forwards_through_fallible_and_trait_object() {
        // The infallible wrapper, the inherent fallible scan, and the
        // `dyn MatchScan` fallible method must all yield the SAME real matches.
        // This pins that the trait override forwards to the real fallible
        // stepper (not the panicking default) and that the infallible wrapper
        // returns the fallible result verbatim — asserted on concrete triples,
        // never `is_empty`.
        use crate::scan::engine::MatchScan;

        let pipe = build(&["ab", "bc"], "input", "hits", 16);
        let haystack = b"zabc";
        let expected = vec![Match::new(0, 1, 3), Match::new(1, 2, 4)];

        let infallible = pipe.reference_scan(haystack);
        let fallible = pipe
            .try_reference_scan(haystack)
            .expect("Fix: small-haystack reference scan must succeed");
        let via_trait = MatchScan::try_reference_scan(&pipe, haystack)
            .expect("Fix: trait-object reference scan must succeed");

        assert_eq!(infallible, expected, "infallible reference_scan content");
        assert_eq!(
            fallible, expected,
            "inherent try_reference_scan must match infallible result verbatim"
        );
        assert_eq!(
            via_trait, expected,
            "dyn MatchScan::try_reference_scan must forward to the real fallible stepper, not the panicking default"
        );
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn rule_pipeline_reference_scan_fails_loud_not_empty_on_oversized_haystack() {
        // Law 10: a >u32 haystack must NOT be swallowed into an empty match set
        // (which reads as "clean"); it must surface loudly. The infallible
        // wrapper aborts; the fallible variant returns Err — neither returns an
        // empty Vec.
        //
        // The guard rejects on length BEFORE the scan loop reads a single byte,
        // so the 4 GiB+1 backing allocation stays lazily zero-paged (only page
        // tables are committed, not 4 GiB of RAM).
        let oversized = vec![0u8; (u32::MAX as usize) + 1];
        let pipe = build(&["ab"], "input", "hits", 16);

        // Fallible path: a real error, not a fabricated-empty success.
        let err = pipe
            .try_reference_scan(&oversized)
            .expect_err("Fix: an over-u32 haystack must error, never report zero matches");
        let msg = err.to_string();
        assert!(
            msg.contains("RulePipeline::reference_scan") && msg.contains("u32"),
            "error must name the scan and the u32 ABI limit, got: {msg}"
        );

        // Infallible path: aborts loudly instead of swallowing to `[]`.
        let prior_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {})); // keep the test log quiet
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pipe.reference_scan(&oversized)
        }));
        std::panic::set_hook(prior_hook);

        let panic_payload = outcome
            .expect_err("Fix: reference_scan must panic on an over-u32 haystack, never return empty");
        let panic_msg = panic_payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic_payload.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(
            panic_msg.contains("try_reference_scan"),
            "panic must point operators at the fallible variant, got: {panic_msg}"
        );
    }

    #[test]
    fn rule_pipeline_hit_buffer_allocation_is_checked_and_zeroed() {
        let bytes = super::zeroed_hit_buffer(2)
            .expect("Fix: small RulePipeline hit buffer should allocate");

        assert_eq!(bytes.len(), (2 * 3 + 1) * 4);
        assert!(bytes.iter().all(|&byte| byte == 0));
    }

    #[test]
    fn rule_pipeline_hit_buffer_into_reuses_and_zeroes_scratch() {
        let mut scratch = vec![0xAA; 128];
        let retained = scratch.capacity();

        super::zeroed_hit_buffer_into(3, &mut scratch)
            .expect("Fix: RulePipeline hit buffer scratch should reserve");

        assert_eq!(scratch.len(), (3 * 3 + 1) * 4);
        assert!(scratch.iter().all(|&byte| byte == 0));
        assert!(scratch.capacity() >= retained);
    }

    #[test]
    fn rule_pipeline_scan_rejects_missing_hit_output_slot() {
        let pipe = build(&["ab"], "input", "hits", 16);
        let backend = RuleReadbackBackend {
            outputs: Vec::new(),
        };
        let mut matches = vec![Match::new(99, 1, 2)];

        let err = pipe
            .scan_into(&backend, b"ab", 1, &mut matches)
            .expect_err("missing RulePipeline hit output must fail");

        let msg = err.to_string();
        assert!(
            matches.is_empty(),
            "scan errors must not expose stale matches"
        );
        assert!(
            msg.contains("RulePipeline hit buffer") && msg.contains("output index 0"),
            "RulePipeline missing-output error must identify the omitted slot: {msg}"
        );
    }

    #[test]
    fn rule_pipeline_scan_rejects_short_hit_counter_readback() {
        let pipe = build(&["ab"], "input", "hits", 16);
        let backend = RuleReadbackBackend {
            outputs: vec![vec![1, 2, 3]],
        };
        let mut matches = vec![Match::new(99, 1, 2)];

        let err = pipe
            .scan_into(&backend, b"ab", 1, &mut matches)
            .expect_err("short RulePipeline counter readback must fail");

        let msg = err.to_string();
        assert!(
            matches.is_empty(),
            "scan errors must not expose stale matches"
        );
        assert!(
            msg.contains("RulePipeline hit buffer") && msg.contains("requires 4 bytes"),
            "RulePipeline counter error must name the malformed hit buffer: {msg}"
        );
    }

    #[test]
    fn rule_pipeline_scan_rejects_match_payload_shorter_than_reported_count() {
        let pipe = build(&["ab"], "input", "hits", 16);
        let backend = RuleReadbackBackend {
            outputs: vec![hit_buffer_bytes(2, &match_triple_bytes(0, 0, 2))],
        };
        let mut matches = vec![Match::new(99, 1, 2)];

        let err = pipe
            .scan_into(&backend, b"ab", 2, &mut matches)
            .expect_err("short RulePipeline match payload must fail");

        let msg = err.to_string();
        assert!(
            matches.is_empty(),
            "scan errors must not expose stale matches"
        );
        assert!(
            msg.contains("readback was 12 byte(s)")
                && msg.contains("count=2")
                && msg.contains("requires 24 byte(s)"),
            "RulePipeline match-payload error must identify observed and required bytes: {msg}"
        );
    }

    #[test]
    fn rule_pipeline_reference_scan_state_is_stack_backed() {
        let production = include_str!("mega_scan.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: mega_scan.rs must contain production section");

        assert!(
            production.contains("let mut state = [0_u32; NFA_LANES];")
                && production.contains("let mut next = [0_u32; NFA_LANES];")
                && production.contains("next.fill(0);")
                && !production.contains("vec![0_u32;")
                && !production.contains("Vec::with_capacity"),
            "Fix: RulePipeline scan and wire paths must use checked shared reservation helpers instead of nested subgroup vector allocation or infallible capacity allocation."
        );
    }

    /// Contract: the compiled program declares the canonical
    /// `nfa_haystack_len` 1-u32 storage buffer so the runtime cursor
    /// loop can read the actual haystack byte count without a
    /// recompile. The presence of this buffer is the wire-level
    /// guarantee that any haystack ≤ declared capacity can dispatch
    /// against the same program. Removing this buffer would silently
    /// re-introduce the "input expected N bytes but received M" hard
    /// error on every short-input dispatch - locking it as a contract.
    #[test]
    fn rule_pipeline_program_declares_haystack_len_buffer() {
        let pipe = build(&["ab"], "input", "hits", 1024);
        let names: Vec<&str> = pipe.program.buffers.iter().map(|b| b.name()).collect();
        assert!(
            names.iter().any(|n| *n == super::nfa::HAYSTACK_LEN_BUF),
            "Fix: nfa_scan must declare `{}` so the cursor loop bound \
             is runtime-supplied; without it, RulePipeline can only \
             dispatch at exactly its compile-time input_len.",
            super::nfa::HAYSTACK_LEN_BUF
        );
    }

    /// Contract: the compiled program declares the canonical
    /// `nfa_max_scan_bytes` 1-u32 storage buffer so the per-workgroup
    /// cursor cap is runtime-supplied. Without this buffer the cursor
    /// loop runs unbounded per workgroup, making the kernel O(N²) on
    /// large inputs - the discord-bot-token-on-62 MiB case that drove
    /// the bound into existence. Removing this buffer would silently
    /// reintroduce that perf cliff.
    #[test]
    fn rule_pipeline_program_declares_max_scan_bytes_buffer() {
        let pipe = build(&["ab"], "input", "hits", 1024);
        let names: Vec<&str> = pipe.program.buffers.iter().map(|b| b.name()).collect();
        assert!(
            names.iter().any(|n| *n == super::nfa::MAX_SCAN_BYTES_BUF),
            "Fix: nfa_scan must declare `{}` so the per-workgroup \
             cursor cap is runtime-supplied; without it, RulePipeline \
             dispatches at O(N²) per shard.",
            super::nfa::MAX_SCAN_BYTES_BUF
        );
    }
}
