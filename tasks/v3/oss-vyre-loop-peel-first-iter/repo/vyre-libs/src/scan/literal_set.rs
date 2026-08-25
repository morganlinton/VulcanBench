//! High-level GPU literal matching engine.
//!
//! Composed entirely from `vyre-libs` LEGO blocks.

use crate::scan::classic_ac::{
    build_ac_bounded_count_suffix3_prefilter_program,
    classic_ac_candidate_suffix3_bloom_words, presence_bitmap_words, presence_by_region_words,
    try_build_ac_bounded_ranges_suffix3_prefilter_program_ext,
    try_build_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program,
    try_build_ac_bounded_ranges_suffix3_presence_by_region_program,
    try_build_ac_bounded_ranges_suffix3_presence_program, CLASSIC_AC_SUFFIX2_MASK_WORDS,
};
use crate::scan::dfa::{dfa_compile, CompiledDfa};
use crate::scan::dispatch_io::ScanDispatchScratch;
use std::borrow::Cow;
use std::collections::TryReserveError;
use vyre::backend::PendingDispatch;
use vyre::ir::{Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};
pub use vyre_foundation::match_result::Match;
use vyre_primitives::hash::fnv1a::{fnv1a64_initial_state, fnv1a64_update_byte};
use vyre_primitives::matching::DfaWireError;

const LITERAL_SET_DEFAULT_MAX_MATCHES: u32 = 10_000;
const MATCH_TRIPLE_WORDS: u32 = 3;
const U32_BYTES: usize = std::mem::size_of::<u32>();
const U32_COUNTER_BYTES: usize = std::mem::size_of::<u32>();
const LITERAL_SET_INPUT_COUNT: usize = 10;
const LITERAL_SET_COUNT_INPUT_COUNT: usize = 8;

/// Resident-resource index containing the mutable literal-set match counter.
pub const LITERAL_SET_MATCH_COUNT_RESOURCE_INDEX: usize = 6;

/// Resident-resource index containing literal-set match triples.
pub const LITERAL_SET_MATCHES_RESOURCE_INDEX: usize = 10;

/// Resident-resource index containing the mutable literal-set match counter.
pub const LITERAL_SET_RESET_RESOURCE_INDICES: [usize; 1] = [LITERAL_SET_MATCH_COUNT_RESOURCE_INDEX];

/// Resident-resource binding order for a prepared literal-set scan.
pub const LITERAL_SET_SCAN_RESOURCE_INDICES: [usize; 11] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

/// Resident-resource index containing the mutable literal-set count result.
pub const LITERAL_SET_COUNT_RESOURCE_INDEX: usize = 7;

/// Resident-resource index containing the mutable literal-set count result.
pub const LITERAL_SET_COUNT_RESET_RESOURCE_INDICES: [usize; 1] = [LITERAL_SET_COUNT_RESOURCE_INDEX];

/// Resident-resource binding order for a prepared literal-set count scan.
pub const LITERAL_SET_COUNT_SCAN_RESOURCE_INDICES: [usize; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

/// Back-compatible literal match type.
pub type LiteralMatch = Match;

/// Errors returned by [`GpuLiteralSet::try_compile`].
#[derive(Debug)]
pub enum LiteralSetCompileError {
    /// Number of patterns does not fit the GPU ABI's `u32` count field.
    PatternCountOverflow {
        /// Number of patterns supplied by the caller.
        count: usize,
    },
    /// One pattern length does not fit the GPU ABI's `u32` length field.
    PatternLengthOverflow {
        /// Index of the oversized pattern.
        pattern_index: usize,
        /// Byte length of the oversized pattern.
        len: usize,
    },
    /// Total concatenated pattern bytes overflowed host `usize`.
    PatternByteCountOverflow,
    /// Total concatenated pattern bytes do not fit the GPU ABI's `u32` field.
    PatternByteCountExceedsGpuAbi {
        /// Concatenated pattern byte count.
        count: usize,
    },
    /// Compiler staging allocation failed.
    StorageReserveFailed {
        /// Scratch vector being reserved.
        field: &'static str,
        /// Requested target capacity.
        requested: usize,
        /// Allocator failure details.
        message: String,
    },
    /// Dispatch program construction failed for the compiled DFA.
    DispatchProgramBuildFailed {
        /// Actionable builder diagnostic.
        message: String,
    },
}

impl std::fmt::Display for LiteralSetCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PatternCountOverflow { count } => write!(
                f,
                "literal_set pattern count {count} exceeds u32 capacity. Fix: shard the pattern set before GPU compilation."
            ),
            Self::PatternLengthOverflow { pattern_index, len } => write!(
                f,
                "literal_set pattern {pattern_index} length {len} exceeds u32 capacity. Fix: split or reject oversized literals before GPU compilation."
            ),
            Self::PatternByteCountOverflow => write!(
                f,
                "literal_set total pattern byte count overflowed host usize. Fix: shard the pattern set before GPU compilation."
            ),
            Self::PatternByteCountExceedsGpuAbi { count } => write!(
                f,
                "literal_set total pattern byte count {count} exceeds u32 capacity. Fix: shard the pattern set before GPU compilation."
            ),
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "literal_set compile failed to reserve {requested} {field} slot(s): {message}. Fix: shard the pattern set before GPU compilation."
            ),
            Self::DispatchProgramBuildFailed { message } => write!(
                f,
                "literal_set DFA dispatch program build failed: {message}"
            ),
        }
    }
}

impl std::error::Error for LiteralSetCompileError {}

/// A high-level literal matching engine.
pub struct GpuLiteralSet {
    /// Underlying DFA components.
    pub dfa: CompiledDfa,
    /// Concatenated literal bytes, one byte per u32 word for GPU comparison.
    pub pattern_bytes: Vec<u32>,
    /// Start offset of each pattern in `pattern_bytes`.
    pub pattern_offsets: Vec<u32>,
    /// Pattern lengths for start-offset calculation.
    pub pattern_lengths: Vec<u32>,
    /// The pre-built vyre Program.
    pub program: Program,
}

/// Reusable hot-loop state for [`GpuLiteralSet`] scans.
///
/// This extends the generic scan dispatch scratch with a one-entry cache for
/// cap-specific `Program` layouts plus suffix-prefilter tables. Callers that
/// repeatedly scan with the same non-default `max_matches` avoid rebuilding the
/// rewritten output-buffer declaration and candidate masks on every dispatch.
#[derive(Debug, Default)]
pub struct LiteralSetScanScratch {
    /// Shared scan staging used by other matching engines.
    pub dispatch: ScanDispatchScratch,
    cached_program: Option<CachedLiteralSetProgram>,
    cached_count_program: Option<CachedLiteralSetCountProgram>,
    cached_prefilter: Option<LiteralSetPrefilterTables>,
}

/// Backend-neutral prepared literal-set scan payload.
///
/// This owns the exact byte buffers consumed by the GPU program. Callers with
/// resident-resource support can upload `inputs` once, append a zeroed output
/// resource sized from `matches_output_bytes`, reset
/// [`LITERAL_SET_RESET_RESOURCE_INDICES`], and dispatch
/// [`LITERAL_SET_SCAN_RESOURCE_INDICES`] without rebuilding the literal tables.
#[derive(Clone, Debug)]
pub struct LiteralSetPreparedScan {
    /// Cap-specific dispatch program for this scan.
    pub program: Program,
    /// Input buffers in program binding order, excluding the `matches` output.
    pub inputs: Vec<Vec<u8>>,
    /// Standard byte-scan dispatch geometry for `haystack_len`.
    pub dispatch_config: DispatchConfig,
    /// Validated haystack byte length.
    pub haystack_len: u32,
    /// Caller-provided output cap.
    pub max_matches: u32,
    /// Full resident output allocation size for the `matches` resource.
    pub matches_output_bytes: usize,
    /// Total bytes in `inputs`.
    pub encoded_input_bytes: u64,
}

impl LiteralSetPreparedScan {
    /// Byte length required to read the match counter.
    #[must_use]
    pub const fn match_count_readback_bytes(&self) -> usize {
        U32_COUNTER_BYTES
    }

    /// Byte length required to read up to `match_count` match triples.
    ///
    /// The returned range is clamped to `max_matches`, matching the decoder
    /// used by [`GpuLiteralSet::scan`].
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] when byte-size arithmetic overflows.
    pub fn match_triples_readback_bytes(
        &self,
        match_count: u32,
    ) -> Result<usize, vyre::BackendError> {
        literal_set_match_triple_bytes(match_count.min(self.max_matches))
    }

    /// Decode scan outputs into caller-owned match storage.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] when output buffers are missing,
    /// malformed, or too short for the reported match count.
    pub fn decode_outputs_into(
        &self,
        outputs: &[Vec<u8>],
        matches: &mut Vec<Match>,
    ) -> Result<(), vyre::BackendError> {
        decode_literal_set_outputs_into(outputs, self.max_matches, matches)
    }
}

/// Backend-neutral prepared literal-set count payload.
///
/// This is the count/presence sibling of [`LiteralSetPreparedScan`]: it keeps
/// the DFA and suffix-prefilter inputs in program binding order but returns
/// only the mutable `match_count` resource.
#[derive(Clone, Debug)]
pub struct LiteralSetPreparedCount {
    /// Count-only suffix-prefiltered dispatch program.
    pub program: Program,
    /// Input buffers in program binding order.
    pub inputs: Vec<Vec<u8>>,
    /// Standard byte-scan dispatch geometry for `haystack_len`.
    pub dispatch_config: DispatchConfig,
    /// Validated haystack byte length.
    pub haystack_len: u32,
    /// Total bytes in `inputs`.
    pub encoded_input_bytes: u64,
}

impl LiteralSetPreparedCount {
    /// Byte length required to read the count result.
    #[must_use]
    pub const fn count_readback_bytes(&self) -> usize {
        U32_COUNTER_BYTES
    }

    /// Decode the count output from either borrowed or resident dispatch.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] when the output slot is missing or too
    /// short for one `u32` counter.
    pub fn decode_outputs(&self, outputs: &[Vec<u8>]) -> Result<u32, vyre::BackendError> {
        decode_literal_set_count_outputs(outputs)
    }
}

/// Resident-resource index of the per-region presence read-write buffer in a
/// prepared region-presence dispatch: the one resource a resident runtime resets
/// (zeroes) before each scan and reads back after.
pub const LITERAL_SET_PRESENCE_BY_REGION_OUTPUT_RESOURCE_INDEX: usize = 6;

/// Backend-neutral prepared RESIDENT region-presence dispatch payload.
///
/// The presence sibling of [`LiteralSetPreparedScan`] / [`LiteralSetPreparedCount`].
/// Owns the exact byte buffers consumed by the region-presence program. A
/// resident runtime uploads the immutable DFA / suffix-prefilter tables ONCE,
/// resets [`LITERAL_SET_PRESENCE_BY_REGION_OUTPUT_RESOURCE_INDEX`], dispatches,
/// and reads back the per-region presence bitmap — re-uploading only the haystack
/// across the files of a corpus. A direct caller can dispatch `inputs` through a
/// borrowed-input backend and decode binding 0 via [`Self::decode_presence`].
#[derive(Clone, Debug)]
pub struct LiteralSetPreparedPresenceByRegion {
    /// Region-presence dispatch program.
    pub program: Program,
    /// Input buffers in program binding order (binding 6 is the zeroed per-region
    /// presence read-write resource = the whole output).
    pub inputs: Vec<Vec<u8>>,
    /// Standard byte-scan dispatch geometry for `haystack_len`.
    pub dispatch_config: DispatchConfig,
    /// Validated haystack byte length.
    pub haystack_len: u32,
    /// Number of coalesced regions (rows in the presence bitmap).
    pub region_count: u32,
    /// Total `u32` words in the per-region presence bitmap
    /// (`region_count * presence_bitmap_words(pattern_count)`).
    pub total_words: usize,
    /// Byte size of the binding-6 presence resource (`total_words * 4`): the
    /// reset + readback length for a resident dispatch.
    pub presence_output_bytes: usize,
    /// Total bytes in `inputs`.
    pub encoded_input_bytes: u64,
}

impl LiteralSetPreparedPresenceByRegion {
    /// Decode the per-region presence bitmap from a dispatch's output buffers:
    /// `region_count * presence_bitmap_words(pattern_count)` packed `u32` words,
    /// IDENTICAL to [`GpuLiteralSet::scan_presence_by_region`]'s return.
    /// `outputs[0]` is the presence-resource readback.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] when the output slot is missing or too short.
    pub fn decode_presence(&self, outputs: &[Vec<u8>]) -> Result<Vec<u32>, vyre::BackendError> {
        let presence_bytes = crate::scan::dispatch_io::try_output_bytes(
            outputs,
            0,
            "literal_set prepared presence_by_region",
        )?;
        Ok(decode_presence_words(presence_bytes, self.total_words))
    }
}

/// The seven corpus-invariant region-presence table byte buffers, at their program
/// binding positions. ONE source of truth for which region-presence tables are
/// immutable across a corpus: shared by [`GpuLiteralSet::build_presence_by_region_dispatch`]
/// (the borrowed / async / prepared paths) and
/// [`GpuLiteralSet::resident_presence_tables`] (the resident pipeline), so all four
/// paths encode byte-identical tables. Every buffer is built through the fail-closed
/// `copy_u32_words_as_le_bytes`.
struct PresenceImmutableTableBytes {
    /// Lane-major DFA transition table (binding 1).
    transitions: Vec<u8>,
    /// DFA output-offset table (binding 2).
    output_offsets: Vec<u8>,
    /// DFA output-record table (binding 3).
    output_records: Vec<u8>,
    /// Per-pattern length table (binding 4).
    pattern_lengths: Vec<u8>,
    /// Suffix prefilter end mask (binding 7).
    candidate_end_mask: Vec<u8>,
    /// Suffix prefilter 2-gram mask (binding 8).
    candidate_suffix2_mask: Vec<u8>,
    /// Suffix prefilter 3-gram bloom (binding 9).
    candidate_suffix3_bloom: Vec<u8>,
}

/// IMMUTABLE region-presence tables (corpus-invariant) plus a `max_regions`-sized
/// program, produced by [`GpuLiteralSet::resident_presence_tables`] and consumed
/// by [`ResidentPresencePipeline`](crate::scan::resident_presence::ResidentPresencePipeline).
///
/// Every field here is a function of the compiled matcher alone — none depends on
/// the haystack or region layout — so a resident session uploads them once and
/// re-dispatches across a whole corpus.
pub(crate) struct ResidentPresenceTables {
    /// Region-presence program sized for up to `max_regions` coalesced files.
    pub(crate) program: Program,
    /// Lane-major DFA transition table bytes (binding 1).
    pub(crate) transitions: Vec<u8>,
    /// DFA output-offset table bytes (binding 2).
    pub(crate) output_offsets: Vec<u8>,
    /// DFA output-record table bytes (binding 3).
    pub(crate) output_records: Vec<u8>,
    /// Per-pattern length table bytes (binding 4).
    pub(crate) pattern_lengths: Vec<u8>,
    /// Suffix prefilter end-mask bytes (binding 7).
    pub(crate) candidate_end_mask: Vec<u8>,
    /// Suffix prefilter 2-gram mask bytes (binding 8).
    pub(crate) candidate_suffix2_mask: Vec<u8>,
    /// Suffix prefilter 3-gram bloom bytes (binding 9).
    pub(crate) candidate_suffix3_bloom: Vec<u8>,
    /// Pattern count (bit width of each per-region presence row).
    pub(crate) pattern_count: u32,
    /// Presence bitmap `u32` words per region (`presence_bitmap_words(pattern_count)`).
    pub(crate) presence_words: u32,
    /// Program workgroup X extent, for the per-scan byte-scan dispatch geometry.
    pub(crate) workgroup_x: u32,
}

#[derive(Debug)]
struct CachedLiteralSetProgram {
    base_fingerprint: [u8; 32],
    max_matches: u32,
    program: Program,
}

#[derive(Debug)]
struct CachedLiteralSetCountProgram {
    pattern_fingerprint: u64,
    program: Program,
}

#[derive(Debug)]
struct LiteralSetPrefilterTables {
    pattern_fingerprint: u64,
    candidate_end_mask: [u32; 8],
    candidate_suffix2_mask: [u32; CLASSIC_AC_SUFFIX2_MASK_WORDS],
    candidate_suffix3_bloom: Vec<u32>,
}

/// Borrowed little-endian byte views of the DFA + prefilter `u32` tables, prepared
/// once and then referenced (in binding order) by a GPU dispatch's `borrowed_inputs`
/// array.
///
/// Four scan entry points — [`GpuLiteralSet::scan_presence`],
/// [`GpuLiteralSet::scan_presence_by_region_with_scratch`],
/// [`GpuLiteralSet::scan_presence_and_positions_by_region`] (via its scratch entry),
/// and `scan_into_with_program` — declared the IDENTICAL seven-view block inline.
/// This is the single source of that byte prep so the view set cannot drift between
/// the four kernels (a divergence would silently miswire one dispatch's bindings).
/// On little-endian hosts every view is a zero-copy borrow
/// ([`dispatch_io::u32_words_as_le_bytes`] → `bytemuck::cast_slice`); on big-endian
/// it owns a byte-swapped copy. The fields' lifetimes are tied to the source `dfa`,
/// `pattern_lengths`, and prefilter tables, so this struct must outlive the
/// `borrowed_inputs` array that references it.
///
/// `count_with_program` deliberately keeps its own NARROWER prep: the count-only
/// kernel never reads `output_records`/`pattern_lengths`, so building them there
/// would be misleading (it would imply the count kernel consumes them).
struct DfaPrefilterByteViews<'a> {
    transitions: Cow<'a, [u8]>,
    output_offsets: Cow<'a, [u8]>,
    output_records: Cow<'a, [u8]>,
    pattern_lengths: Cow<'a, [u8]>,
    candidate_end_mask: Cow<'a, [u8]>,
    candidate_suffix2_mask: Cow<'a, [u8]>,
    candidate_suffix3_bloom: Cow<'a, [u8]>,
}

impl<'a> DfaPrefilterByteViews<'a> {
    fn new(
        dfa: &'a CompiledDfa,
        pattern_lengths: &'a [u32],
        prefilter: &'a LiteralSetPrefilterTables,
    ) -> Self {
        use crate::scan::dispatch_io::u32_words_as_le_bytes;
        Self {
            transitions: u32_words_as_le_bytes(&dfa.transitions),
            output_offsets: u32_words_as_le_bytes(&dfa.output_offsets),
            output_records: u32_words_as_le_bytes(&dfa.output_records),
            pattern_lengths: u32_words_as_le_bytes(pattern_lengths),
            candidate_end_mask: u32_words_as_le_bytes(&prefilter.candidate_end_mask),
            candidate_suffix2_mask: u32_words_as_le_bytes(&prefilter.candidate_suffix2_mask),
            candidate_suffix3_bloom: u32_words_as_le_bytes(&prefilter.candidate_suffix3_bloom),
        }
    }
}

/// In-flight handle for [`GpuLiteralSet::scan_presence_by_region_async`].
///
/// Returned the moment the GPU dispatch is submitted, so the caller can run
/// host-side work concurrently with the device scan, then decode the per-region
/// presence bitmap with [`Self::await_words`]. The owned input buffers the scan
/// was submitted with are retained here (never read again on the host) so their
/// backing memory stays valid for the whole dispatch on backends whose async
/// upload reads host memory after submit returns.
pub struct PendingPresenceByRegion {
    pending: Box<dyn PendingDispatch>,
    total_words: usize,
    // Owned dispatch inputs kept alive until `await_words`. Retained purely so
    // the device-side async upload's backing memory remains valid for the whole
    // dispatch; never read again on the host.
    _inputs: Vec<Vec<u8>>,
}

impl PendingPresenceByRegion {
    /// Non-blocking readiness probe. `true` means [`Self::await_words`] will not
    /// block the caller thread. Backends that cannot probe without cost report
    /// `true` unconditionally (the caller then blocks inside `await_words`). See
    /// [`vyre::backend::PendingDispatch::is_ready`].
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.pending.is_ready()
    }

    /// Block until the GPU scan completes and decode the per-region presence
    /// bitmap: `region_count × presence_bitmap_words(pattern_count)` packed `u32`
    /// words, IDENTICAL to [`GpuLiteralSet::scan_presence_by_region`]'s return
    /// (bit `p` of region `r`'s row is set iff pattern `p`'s literal occurs in
    /// region `r`). Calling this when [`Self::is_ready`] is `true` does not block.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] on dispatch/readback failure.
    pub fn await_words(self) -> Result<Vec<u32>, vyre::BackendError> {
        let outputs = self.pending.await_result()?;
        let presence_bytes = crate::scan::dispatch_io::try_output_bytes(
            &outputs,
            0,
            "literal_set presence_by_region async",
        )?;
        Ok(decode_presence_words(presence_bytes, self.total_words))
    }
}

impl GpuLiteralSet {
    /// Compile a set of literal patterns into a GPU-ready matcher.
    ///
    /// # Panics
    ///
    /// Aborts when staging allocation fails or a pattern count/length cannot be
    /// represented by the GPU ABI. Returning an empty matcher would silently
    /// match NOTHING — reporting every input as clean (Law 10). Fail closed
    /// instead; callers that must recover use [`Self::try_compile`].
    #[must_use]
    pub fn compile(patterns: &[&[u8]]) -> Self {
        match Self::try_compile(patterns) {
            Ok(compiled) => compiled,
            Err(error) => {
                panic!(
                    "vyre-libs GpuLiteralSet::compile failed: {error} — \
                     returning an empty matcher would silently match nothing and report every input as clean; \
                     use try_compile and reduce the pattern set below the GPU ABI limits."
                )
            }
        }
    }

    /// Compile a set of literal patterns into a GPU-ready matcher, surfacing
    /// allocation and ABI-size failures instead of truncating them.
    ///
    /// # Errors
    ///
    /// Returns [`LiteralSetCompileError`] when staging allocation fails or a
    /// pattern count/length cannot be represented by the GPU ABI.
    pub fn try_compile(patterns: &[&[u8]]) -> Result<Self, LiteralSetCompileError> {
        let dfa = dfa_compile(patterns);
        let declared_pattern_count = u32::try_from(patterns.len()).map_err(|_| {
            LiteralSetCompileError::PatternCountOverflow {
                count: patterns.len(),
            }
        })?;
        let total_pattern_bytes = patterns.iter().try_fold(0usize, |sum, pattern| {
            sum.checked_add(pattern.len())
                .ok_or(LiteralSetCompileError::PatternByteCountOverflow)
        })?;
        u32::try_from(total_pattern_bytes).map_err(|_| {
            LiteralSetCompileError::PatternByteCountExceedsGpuAbi {
                count: total_pattern_bytes,
            }
        })?;
        let mut pattern_lengths = Vec::new();
        reserve_vec(&mut pattern_lengths, patterns.len(), "pattern length")?;
        let mut pattern_offsets = Vec::new();
        reserve_vec(&mut pattern_offsets, patterns.len(), "pattern offset")?;
        let mut pattern_bytes = Vec::new();
        reserve_vec(
            &mut pattern_bytes,
            total_pattern_bytes,
            "packed pattern byte",
        )?;
        for (pattern_index, pattern) in patterns.iter().enumerate() {
            let offset = u32::try_from(pattern_bytes.len()).map_err(|_| {
                LiteralSetCompileError::PatternByteCountExceedsGpuAbi {
                    count: pattern_bytes.len(),
                }
            })?;
            let len = u32::try_from(pattern.len()).map_err(|_| {
                LiteralSetCompileError::PatternLengthOverflow {
                    pattern_index,
                    len: pattern.len(),
                }
            })?;
            pattern_offsets.push(offset);
            pattern_lengths.push(len);
            pattern_bytes.extend(pattern.iter().map(|&byte| u32::from(byte)));
        }

        let program = try_build_literal_set_program(&dfa, declared_pattern_count)
            .map_err(|message| LiteralSetCompileError::DispatchProgramBuildFailed { message })?;

        Ok(Self {
            dfa,
            pattern_bytes,
            pattern_offsets,
            pattern_lengths,
            program,
        })
    }

    /// Reference oracle implementation for parity testing.
    #[must_use]
    pub fn reference_scan(&self, haystack: &[u8]) -> Vec<Match> {
        let mut state = 0u32;
        let mut results = Vec::new();
        for (pos, &byte) in haystack.iter().enumerate() {
            state = self.dfa.transitions[(state as usize) * 256 + (byte as usize)];
            let begin = self.dfa.output_offsets[state as usize] as usize;
            let end = self.dfa.output_offsets[state as usize + 1] as usize;
            for &pattern_id in &self.dfa.output_records[begin..end] {
                let len = self.pattern_lengths[pattern_id as usize];
                results.push(Match::new(
                    pattern_id,
                    (pos as u32 + 1).saturating_sub(len),
                    pos as u32 + 1,
                ));
            }
        }
        results.sort_unstable();
        results
    }

    /// GPU scan dispatch.
    ///
    /// # Errors
    /// Returns [\`vyre::BackendError\`] if dispatch or readback fails.
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

    /// GPU scan dispatch that decodes into caller-owned match scratch.
    ///
    /// Long-running scanners can reuse `matches` across inputs and avoid one
    /// heap allocation per dispatch. Output ordering and truncation semantics
    /// match [`Self::scan`].
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] if dispatch or readback fails.
    pub fn scan_into<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        max_matches: u32,
        matches: &mut Vec<Match>,
    ) -> Result<(), vyre::BackendError> {
        let mut scratch = ScanDispatchScratch::default();
        self.scan_into_with_scratch(backend, haystack, max_matches, matches, &mut scratch)
    }

    /// GPU count-only dispatch.
    ///
    /// Use this when the caller needs match cardinality or presence without
    /// materializing every `(pattern_id, start, end)` triple. It dispatches the
    /// suffix-prefiltered bounded DFA count kernel and reads one `u32`.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] if dispatch, readback, scan-boundary
    /// validation, or host staging allocation fails.
    pub fn count<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
    ) -> Result<u32, vyre::BackendError> {
        let mut scratch = LiteralSetScanScratch::default();
        self.count_with_literal_scratch(backend, haystack, &mut scratch)
    }

    /// GPU scan dispatch that decodes into caller-owned match scratch and
    /// reuses caller-owned byte staging.
    ///
    /// `matches` reuses decoded match storage and `scratch` reuses the packed
    /// haystack buffer across dispatches. For stable literal-set hot loops, use
    /// [`Self::prepare_literal_scratch`] with
    /// [`Self::scan_into_with_literal_scratch`] to also reuse the derived
    /// suffix-prefilter tables and cap-specific program layout.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] if dispatch, readback, scan-boundary
    /// validation, or host staging allocation fails.
    pub fn scan_into_with_scratch<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        max_matches: u32,
        matches: &mut Vec<Match>,
        scratch: &mut ScanDispatchScratch,
    ) -> Result<(), vyre::BackendError> {
        let dispatch_program = self.program_for_match_capacity(max_matches)?;
        let prefilter_tables = self.build_prefilter_tables()?;
        self.scan_into_with_program(
            backend,
            haystack,
            max_matches,
            matches,
            scratch,
            dispatch_program.as_ref(),
            &prefilter_tables,
        )
    }

    /// Prepare literal-set-owned hot-loop scratch for repeated dispatches.
    ///
    /// This builds the cap-specific `Program` layout and suffix-prefilter
    /// tables outside the timed scan path. It is useful for callers that know
    /// their match-capacity budget before scanning a stream of similarly shaped
    /// inputs.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] if match-capacity sizing or
    /// suffix-prefilter staging fails.
    pub fn prepare_literal_scratch(
        &self,
        max_matches: u32,
        scratch: &mut LiteralSetScanScratch,
    ) -> Result<(), vyre::BackendError> {
        self.program_for_match_capacity_cached(max_matches, &mut scratch.cached_program)?;
        self.prefilter_tables_cached(&mut scratch.cached_prefilter)?;
        Ok(())
    }

    /// Prepare count-only hot-loop scratch for repeated dispatches.
    ///
    /// This builds the count dispatch `Program` and suffix-prefilter tables
    /// outside the timed count path without preparing match-list output state.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] if suffix-prefilter staging fails.
    pub fn prepare_count_scratch(
        &self,
        scratch: &mut LiteralSetScanScratch,
    ) -> Result<(), vyre::BackendError> {
        self.count_program_cached(&mut scratch.cached_count_program)?;
        self.prefilter_tables_cached(&mut scratch.cached_prefilter)?;
        Ok(())
    }

    /// Prepare a backend-neutral dispatch payload for this literal set.
    ///
    /// The returned plan owns packed haystack bytes, DFA tables, suffix
    /// prefilter tables, the zeroed match counter, and the cap-specific
    /// `Program`. Direct callers can dispatch `inputs` through a normal
    /// borrowed-input backend. Runtimes with resident resources can upload the
    /// same `inputs` once and reuse the immutable resources across repeated
    /// scans of the same haystack.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] if scan-boundary validation,
    /// cap-specific program sizing, suffix-prefilter staging, or input-buffer
    /// allocation fails.
    pub fn prepare_scan_dispatch(
        &self,
        haystack: &[u8],
        max_matches: u32,
    ) -> Result<LiteralSetPreparedScan, vyre::BackendError> {
        let dispatch_program = self.program_for_match_capacity(max_matches)?;
        let prefilter_tables = self.build_prefilter_tables()?;
        self.prepare_scan_dispatch_with_program(
            haystack,
            max_matches,
            dispatch_program.as_ref(),
            &prefilter_tables,
        )
    }

    /// Prepare a backend-neutral count-only dispatch payload.
    ///
    /// Runtimes with resident resources can upload the returned `inputs` once,
    /// reset [`LITERAL_SET_COUNT_RESET_RESOURCE_INDICES`], dispatch
    /// [`LITERAL_SET_COUNT_SCAN_RESOURCE_INDICES`], and read back
    /// [`LITERAL_SET_COUNT_RESOURCE_INDEX`] for one `u32` result.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] if scan-boundary validation,
    /// suffix-prefilter staging, or input-buffer allocation fails.
    pub fn prepare_count_dispatch(
        &self,
        haystack: &[u8],
    ) -> Result<LiteralSetPreparedCount, vyre::BackendError> {
        let count_program = self.count_program();
        let prefilter_tables = self.build_prefilter_tables()?;
        self.prepare_count_dispatch_with_program(haystack, &count_program, &prefilter_tables)
    }

    /// GPU scan dispatch with literal-set-owned hot-loop scratch.
    ///
    /// Use this for repeated scans where `max_matches` is usually stable but
    /// not equal to the compiled default. It reuses packed haystack bytes,
    /// suffix-prefilter tables, and the cap-specific rewritten dispatch
    /// `Program`.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] if dispatch, readback, scan-boundary
    /// validation, host staging allocation, or cap-specific program sizing
    /// fails.
    pub fn scan_into_with_literal_scratch<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        max_matches: u32,
        matches: &mut Vec<Match>,
        scratch: &mut LiteralSetScanScratch,
    ) -> Result<(), vyre::BackendError> {
        let cached_program = &mut scratch.cached_program;
        let dispatch_program =
            self.program_for_match_capacity_cached(max_matches, cached_program)?;
        let prefilter_tables = self.prefilter_tables_cached(&mut scratch.cached_prefilter)?;
        self.scan_into_with_program(
            backend,
            haystack,
            max_matches,
            matches,
            &mut scratch.dispatch,
            dispatch_program,
            prefilter_tables,
        )
    }

    /// GPU count-only dispatch with literal-set-owned hot-loop scratch.
    ///
    /// Reuses packed haystack bytes, suffix-prefilter tables, and the count
    /// dispatch `Program` across repeated scans.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] if dispatch, readback, scan-boundary
    /// validation, or host staging allocation fails.
    pub fn count_with_literal_scratch<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        scratch: &mut LiteralSetScanScratch,
    ) -> Result<u32, vyre::BackendError> {
        let count_program = self.count_program_cached(&mut scratch.cached_count_program)?;
        let prefilter_tables = self.prefilter_tables_cached(&mut scratch.cached_prefilter)?;
        self.count_with_program(
            backend,
            haystack,
            &mut scratch.dispatch,
            count_program,
            prefilter_tables,
        )
    }

    /// GPU PRESENCE scan: return a per-pattern presence bitmap as packed `u32`
    /// words (bit `p` — word `p >> 5`, bit `p & 31` — set iff pattern `p`'s literal
    /// occurs in `haystack`). This is the compact-output counterpart of
    /// [`Self::scan`] for prefilter consumers that need only WHICH patterns fired,
    /// not where. The kernel performs one idempotent `atomic_or` per hit into a
    /// `ceil(patterns/32)`-word bitmap instead of appending an `(id,start,end)`
    /// triple through an atomic counter, so match-DENSE inputs stay near the scan
    /// throughput ceiling rather than collapsing on per-hit output serialization +
    /// large triple readback (the dominant cost measured on dense corpora).
    ///
    /// The bitmap is sound for presence: concurrent lanes setting the same bit
    /// race harmlessly (OR is idempotent), and bits in the same word are merged by
    /// `atomic_or`. Inputs 0-5 / 7-9 are byte-identical to [`Self::scan`].
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] on dispatch/readback failure, scan-boundary
    /// validation, or a pattern count exceeding the u32 GPU ABI.
    pub fn scan_presence<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
    ) -> Result<Vec<u32>, vyre::BackendError> {
        let mut scratch = ScanDispatchScratch::default();
        self.scan_presence_with_scratch(backend, haystack, &mut scratch)
    }

    /// [`Self::scan_presence`] with caller-owned hot-loop scratch (reuses the
    /// packed-haystack staging buffer across repeated scans).
    ///
    /// # Errors
    /// See [`Self::scan_presence`].
    pub fn scan_presence_with_scratch<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        scratch: &mut ScanDispatchScratch,
    ) -> Result<Vec<u32>, vyre::BackendError> {
        use crate::scan::dispatch_io;

        let pattern_count = u32::try_from(self.pattern_lengths.len()).map_err(|_| {
            vyre::BackendError::new(
                "literal_set presence: pattern count exceeds u32 GPU ABI".to_string(),
            )
        })?;
        let presence_words = presence_bitmap_words(pattern_count) as usize;
        let program =
            try_build_ac_bounded_ranges_suffix3_presence_program(&self.dfa, pattern_count)
                .map_err(vyre::BackendError::new)?;
        let prefilter_tables = self.build_prefilter_tables()?;

        let haystack_len = dispatch_io::scan_guard(
            haystack,
            "literal_set_presence",
            dispatch_io::DEFAULT_MAX_SCAN_BYTES,
        )?;
        dispatch_io::pack_haystack_u32_into(haystack, &mut scratch.haystack_bytes)?;
        let haystack_bytes = scratch.haystack_bytes.as_slice();
        let views = DfaPrefilterByteViews::new(&self.dfa, &self.pattern_lengths, &prefilter_tables);
        let haystack_len_word = [haystack_len];
        let haystack_len_bytes = dispatch_io::u32_words_as_le_bytes(&haystack_len_word);
        // Presence buffer (binding 6) is read-write: uploaded zeroed, dispatched,
        // and read back. It is the entire output.
        let presence_zeroed = vec![0u8; presence_words.saturating_mul(4)];

        let config =
            dispatch_io::byte_scan_dispatch_config(haystack_len, program.workgroup_size[0]);
        let borrowed_inputs: smallvec::SmallVec<[&[u8]; 10]> = [
            haystack_bytes,                         // 0: haystack (Packed U32)
            views.transitions.as_ref(),             // 1: transitions
            views.output_offsets.as_ref(),          // 2: output_offsets
            views.output_records.as_ref(),          // 3: output_records
            views.pattern_lengths.as_ref(),         // 4: pattern_lengths
            haystack_len_bytes.as_ref(),            // 5: haystack_len
            presence_zeroed.as_slice(),             // 6: presence (read_write)
            views.candidate_end_mask.as_ref(),      // 7: candidate_end_mask
            views.candidate_suffix2_mask.as_ref(),  // 8: candidate_suffix2_mask
            views.candidate_suffix3_bloom.as_ref(), // 9: candidate_suffix3_bloom
        ]
        .into_iter()
        .collect();
        let outputs = backend.dispatch_borrowed(&program, &borrowed_inputs, &config)?;
        // `presence` is the only read-write/output buffer, so it is outputs[0].
        let presence_bytes = dispatch_io::try_output_bytes(&outputs, 0, "literal_set presence")?;
        Ok(decode_presence_words(presence_bytes, presence_words))
    }

    /// GPU REGION-PRESENCE scan: return a per-REGION presence bitmap, where region
    /// `r` is the slice `[region_starts[r], region_starts[r+1])` of `haystack` (a
    /// coalesced batch of independent files). The result is `region_starts.len() ×
    /// presence_bitmap_words(pattern_count)` packed `u32` words: bit `p` of region
    /// `r`'s row is set iff pattern `p`'s literal occurs inside region `r`.
    ///
    /// This is the dense-batch counterpart of [`Self::scan_presence`]: it keeps the
    /// idempotent-`atomic_or` output (no per-hit counter, no triple readback, so it
    /// stays near the scan-throughput ceiling on match-dense corpora) while
    /// preserving per-file attribution, which the global presence bitmap loses. The
    /// consumer (e.g. a coalesced GPU phase-1 scanner) gets the exact per-file
    /// trigger set it needs without materializing spans or reducing triples on the
    /// host.
    ///
    /// `region_starts` must be ascending with `region_starts[0] == 0`. A match never
    /// spans a region boundary (the consumer inserts separator bytes between files),
    /// so the end-position attribution the kernel performs equals start attribution.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] on dispatch/readback failure, scan-boundary
    /// validation, an empty or non-zero-based `region_starts`, or a pattern/region
    /// count exceeding the u32 GPU ABI.
    pub fn scan_presence_by_region<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        region_starts: &[u32],
    ) -> Result<Vec<u32>, vyre::BackendError> {
        let mut scratch = ScanDispatchScratch::default();
        self.scan_presence_by_region_with_scratch(backend, haystack, region_starts, 0, &mut scratch)
    }

    /// [`Self::scan_presence_by_region`] with caller-owned hot-loop scratch and an
    /// explicit `region_base` shard offset.
    ///
    /// `region_base` is added to every candidate position before the region
    /// binary search, so a SHARDED caller can dispatch a slice `haystack` (with
    /// local positions) against the WHOLE batch's `region_starts` by passing the
    /// shard's global start offset. Pass `0` for a single-dispatch scan. Each
    /// shard returns the full `region_count × words` bitmap (rows it didn't touch
    /// stay zero); OR the per-shard bitmaps to assemble the batch result.
    ///
    /// # Errors
    /// See [`Self::scan_presence_by_region`].
    pub fn scan_presence_by_region_with_scratch<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        region_starts: &[u32],
        region_base: u32,
        scratch: &mut ScanDispatchScratch,
    ) -> Result<Vec<u32>, vyre::BackendError> {
        use crate::scan::dispatch_io;

        let pattern_count = u32::try_from(self.pattern_lengths.len()).map_err(|_| {
            vyre::BackendError::new(
                "literal_set region-presence: pattern count exceeds u32 GPU ABI".to_string(),
            )
        })?;
        let region_count = u32::try_from(region_starts.len()).map_err(|_| {
            vyre::BackendError::new(
                "literal_set region-presence: region count exceeds u32 GPU ABI".to_string(),
            )
        })?;
        if region_count == 0 {
            return Err(vyre::BackendError::new(
                "literal_set region-presence: region_starts must be non-empty. Fix: pass one start offset per coalesced file, beginning with 0.".to_string(),
            ));
        }
        if region_starts[0] != 0 {
            return Err(vyre::BackendError::new(
                "literal_set region-presence: region_starts[0] must be 0 (the kernel binary-search lower bound). Fix: the first coalesced file must start at offset 0.".to_string(),
            ));
        }
        let presence_words = presence_bitmap_words(pattern_count) as usize;
        let total_words = presence_by_region_words(pattern_count, region_count) as usize;
        let program = try_build_ac_bounded_ranges_suffix3_presence_by_region_program(
            &self.dfa,
            pattern_count,
            region_count,
        )
        .map_err(vyre::BackendError::new)?;
        let prefilter_tables = self.build_prefilter_tables()?;

        let haystack_len = dispatch_io::scan_guard(
            haystack,
            "literal_set_presence_by_region",
            dispatch_io::DEFAULT_MAX_SCAN_BYTES,
        )?;
        dispatch_io::pack_haystack_u32_into(haystack, &mut scratch.haystack_bytes)?;
        let haystack_bytes = scratch.haystack_bytes.as_slice();
        let views = DfaPrefilterByteViews::new(&self.dfa, &self.pattern_lengths, &prefilter_tables);
        let haystack_len_word = [haystack_len];
        let haystack_len_bytes = dispatch_io::u32_words_as_le_bytes(&haystack_len_word);
        let region_starts_bytes = dispatch_io::u32_words_as_le_bytes(region_starts);
        let region_base_bytes = region_base.to_le_bytes();
        // Per-region presence buffer (binding 6) is read-write: uploaded zeroed,
        // dispatched, read back. It is the entire output.
        let presence_zeroed = vec![0u8; total_words.saturating_mul(4)];

        let config =
            dispatch_io::byte_scan_dispatch_config(haystack_len, program.workgroup_size[0]);
        let borrowed_inputs: smallvec::SmallVec<[&[u8]; 12]> = [
            haystack_bytes,                         // 0: haystack (Packed U32)
            views.transitions.as_ref(),             // 1: transitions
            views.output_offsets.as_ref(),          // 2: output_offsets
            views.output_records.as_ref(),          // 3: output_records
            views.pattern_lengths.as_ref(),         // 4: pattern_lengths
            haystack_len_bytes.as_ref(),            // 5: haystack_len
            presence_zeroed.as_slice(),             // 6: per-region presence (read_write)
            views.candidate_end_mask.as_ref(),      // 7: candidate_end_mask
            views.candidate_suffix2_mask.as_ref(),  // 8: candidate_suffix2_mask
            views.candidate_suffix3_bloom.as_ref(), // 9: candidate_suffix3_bloom
            region_starts_bytes.as_ref(),           // 10: region_starts
            region_base_bytes.as_slice(),           // 11: region_base (shard offset)
        ]
        .into_iter()
        .collect();
        let outputs = backend.dispatch_borrowed(&program, &borrowed_inputs, &config)?;
        let presence_bytes =
            dispatch_io::try_output_bytes(&outputs, 0, "literal_set presence_by_region")?;
        Ok(decode_presence_words(presence_bytes, total_words))
    }

    /// ASYNC counterpart of [`Self::scan_presence_by_region_with_scratch`]: submit
    /// the GPU region-presence dispatch and return a [`PendingPresenceByRegion`]
    /// handle IMMEDIATELY, so the caller can OVERLAP host-side work (e.g. a downstream
    /// scanner's trigger-independent entropy-candidate generation) with the in-flight GPU
    /// scan, then decode the per-region bitmap via
    /// [`PendingPresenceByRegion::await_words`].
    ///
    /// On a backend that genuinely pipelines host/device work (wgpu, cuda) the GPU
    /// scan executes while the caller does host work; on a backend that cannot
    /// (the synchronous default in [`VyreBackend::dispatch_async`]) the handle is
    /// trivially ready and this is equivalent — same bitmap, no overlap, no silent
    /// change of result. See [`vyre::backend::PendingDispatch`].
    ///
    /// Unlike the synchronous entry there is NO `scratch` parameter: the async
    /// dispatch ABI is `&[Vec<u8>]`, so inputs are built into OWNED buffers that
    /// the returned handle RETAINS until `await_words`. This keeps the device-side
    /// upload's backing memory valid for the whole dispatch — required on backends
    /// (e.g. the CUDA stream h2d copy) whose async upload reads host memory after
    /// this call returns.
    ///
    /// `region_base` has the same sharded-offset meaning as in
    /// [`Self::scan_presence_by_region_with_scratch`] (pass `0` for a single-shard
    /// scan).
    ///
    /// # Errors
    /// See [`Self::scan_presence_by_region`]. Errors that surface only during GPU
    /// execution come back from [`PendingPresenceByRegion::await_words`], not here.
    pub fn scan_presence_by_region_async<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        region_starts: &[u32],
        region_base: u32,
    ) -> Result<PendingPresenceByRegion, vyre::BackendError> {
        let (program, inputs, config, total_words, _haystack_len) =
            self.build_presence_by_region_dispatch(haystack, region_starts, region_base)?;
        let pending = backend.dispatch_async(&program, &inputs, &config)?;
        Ok(PendingPresenceByRegion {
            pending,
            total_words,
            _inputs: inputs,
        })
    }

    /// Build the OWNED region-presence dispatch payload shared by
    /// [`Self::scan_presence_by_region_async`] and
    /// [`Self::prepare_presence_by_region_dispatch`]: the cap-specific program,
    /// the 12 input buffers in binding order (binding 6 is the zeroed per-region
    /// presence read-write resource = the whole output), the byte-scan dispatch
    /// config, and the total presence-bitmap `u32` word count.
    ///
    /// Inputs are OWNED (the async ABI is `&[Vec<u8>]`, and a resident runtime
    /// uploads them once), built through the fallible `copy_u32_words_as_le_bytes`
    /// so an allocation failure fails CLOSED (`BackendError`) instead of aborting
    /// on OOM — the same contract as [`Self::prepare_scan_dispatch`].
    ///
    /// # Errors
    /// See [`Self::scan_presence_by_region`].
    /// Encode the seven corpus-invariant region-presence tables into owned
    /// little-endian byte buffers through the fail-closed `copy_u32_words_as_le_bytes`.
    /// The single source of truth for which tables are immutable across a corpus —
    /// reused by the borrowed/async/prepared builder and the resident pipeline so
    /// every path encodes byte-identical tables.
    fn presence_immutable_table_bytes(
        &self,
        prefilter: &LiteralSetPrefilterTables,
    ) -> Result<PresenceImmutableTableBytes, vyre::BackendError> {
        Ok(PresenceImmutableTableBytes {
            transitions: copy_u32_words_as_le_bytes(&self.dfa.transitions, "transition table")?,
            output_offsets: copy_u32_words_as_le_bytes(
                &self.dfa.output_offsets,
                "output offset table",
            )?,
            output_records: copy_u32_words_as_le_bytes(
                &self.dfa.output_records,
                "output record table",
            )?,
            pattern_lengths: copy_u32_words_as_le_bytes(
                &self.pattern_lengths,
                "pattern length table",
            )?,
            candidate_end_mask: copy_u32_words_as_le_bytes(
                &prefilter.candidate_end_mask,
                "candidate end mask",
            )?,
            candidate_suffix2_mask: copy_u32_words_as_le_bytes(
                &prefilter.candidate_suffix2_mask,
                "candidate suffix2 mask",
            )?,
            candidate_suffix3_bloom: copy_u32_words_as_le_bytes(
                &prefilter.candidate_suffix3_bloom,
                "candidate suffix3 bloom",
            )?,
        })
    }

    fn build_presence_by_region_dispatch(
        &self,
        haystack: &[u8],
        region_starts: &[u32],
        region_base: u32,
    ) -> Result<(Program, Vec<Vec<u8>>, DispatchConfig, usize, u32), vyre::BackendError> {
        use crate::scan::dispatch_io;

        let pattern_count = u32::try_from(self.pattern_lengths.len()).map_err(|_| {
            vyre::BackendError::new(
                "literal_set region-presence: pattern count exceeds u32 GPU ABI".to_string(),
            )
        })?;
        let region_count = u32::try_from(region_starts.len()).map_err(|_| {
            vyre::BackendError::new(
                "literal_set region-presence: region count exceeds u32 GPU ABI".to_string(),
            )
        })?;
        if region_count == 0 {
            return Err(vyre::BackendError::new(
                "literal_set region-presence: region_starts must be non-empty. Fix: pass one start offset per coalesced file, beginning with 0.".to_string(),
            ));
        }
        if region_starts[0] != 0 {
            return Err(vyre::BackendError::new(
                "literal_set region-presence: region_starts[0] must be 0 (the kernel binary-search lower bound). Fix: the first coalesced file must start at offset 0.".to_string(),
            ));
        }
        let total_words = presence_by_region_words(pattern_count, region_count) as usize;
        let program = try_build_ac_bounded_ranges_suffix3_presence_by_region_program(
            &self.dfa,
            pattern_count,
            region_count,
        )
        .map_err(vyre::BackendError::new)?;
        let prefilter_tables = self.build_prefilter_tables()?;

        let haystack_len = dispatch_io::scan_guard(
            haystack,
            "literal_set_presence_by_region",
            dispatch_io::DEFAULT_MAX_SCAN_BYTES,
        )?;
        let mut haystack_packed = Vec::new();
        dispatch_io::pack_haystack_u32_into(haystack, &mut haystack_packed)?;
        let haystack_len_word = [haystack_len];
        let region_base_bytes = region_base.to_le_bytes();
        // Per-region presence buffer (binding 6) is read-write: uploaded zeroed,
        // dispatched, read back. It is the entire output.
        let presence_zeroed = vec![0u8; total_words.saturating_mul(4)];

        const PRESENCE_BY_REGION_INPUT_COUNT: usize = 12;
        let mut inputs: Vec<Vec<u8>> = Vec::new();
        vyre_foundation::allocation::try_reserve_vec_to_capacity(
            &mut inputs,
            PRESENCE_BY_REGION_INPUT_COUNT,
        )
        .map_err(|source| {
            vyre::BackendError::new(format!(
                "literal_set region-presence could not reserve {PRESENCE_BY_REGION_INPUT_COUNT} input buffer slot(s): {source}. Fix: shard the literal set or haystack before dispatch."
            ))
        })?;
        let tables = self.presence_immutable_table_bytes(&prefilter_tables)?;
        inputs.push(haystack_packed); // 0: haystack (Packed U32)
        inputs.push(tables.transitions); // 1
        inputs.push(tables.output_offsets); // 2
        inputs.push(tables.output_records); // 3
        inputs.push(tables.pattern_lengths); // 4
        inputs.push(copy_u32_words_as_le_bytes(&haystack_len_word, "haystack length")?); // 5
        inputs.push(presence_zeroed); // 6: per-region presence (read_write)
        inputs.push(tables.candidate_end_mask); // 7
        inputs.push(tables.candidate_suffix2_mask); // 8
        inputs.push(tables.candidate_suffix3_bloom); // 9
        inputs.push(copy_u32_words_as_le_bytes(region_starts, "region starts")?); // 10
        inputs.push(region_base_bytes.to_vec()); // 11: region_base (4 bytes)

        let config =
            dispatch_io::byte_scan_dispatch_config(haystack_len, program.workgroup_size[0]);
        Ok((program, inputs, config, total_words, haystack_len))
    }

    /// Prepare a backend-neutral RESIDENT region-presence dispatch payload: the
    /// same owned buffers [`Self::scan_presence_by_region_async`] dispatches, but
    /// returned for a resident runtime to upload ONCE (the immutable DFA /
    /// suffix-prefilter tables) and re-dispatch across many files of a corpus,
    /// re-uploading only the haystack and resetting the binding-6 presence
    /// resource ([`LITERAL_SET_PRESENCE_BY_REGION_OUTPUT_RESOURCE_INDEX`]). This
    /// is the presence sibling of [`Self::prepare_scan_dispatch`].
    ///
    /// A direct caller can also dispatch `inputs` through a normal borrowed-input
    /// backend and decode binding 0 via [`LiteralSetPreparedPresenceByRegion::decode_presence`].
    ///
    /// # Errors
    /// See [`Self::scan_presence_by_region`].
    pub fn prepare_presence_by_region_dispatch(
        &self,
        haystack: &[u8],
        region_starts: &[u32],
        region_base: u32,
    ) -> Result<LiteralSetPreparedPresenceByRegion, vyre::BackendError> {
        let region_count = u32::try_from(region_starts.len()).map_err(|_| {
            vyre::BackendError::new(
                "literal_set region-presence: region count exceeds u32 GPU ABI".to_string(),
            )
        })?;
        let (program, inputs, dispatch_config, total_words, haystack_len) =
            self.build_presence_by_region_dispatch(haystack, region_starts, region_base)?;
        let presence_output_bytes = total_words.saturating_mul(U32_BYTES);
        let encoded_input_bytes = inputs.iter().try_fold(0_u64, |sum, input| {
            let len = u64::try_from(input.len()).map_err(|source| {
                vyre::BackendError::new(format!(
                    "literal_set prepared region-presence input byte length does not fit u64: {source}. Fix: shard the scan before dispatch."
                ))
            })?;
            sum.checked_add(len).ok_or_else(|| {
                vyre::BackendError::new(
                    "literal_set prepared region-presence input byte total overflowed u64. Fix: shard the scan before dispatch.",
                )
            })
        })?;
        Ok(LiteralSetPreparedPresenceByRegion {
            program,
            inputs,
            dispatch_config,
            haystack_len,
            region_count,
            total_words,
            presence_output_bytes,
            encoded_input_bytes,
        })
    }

    /// Extract the IMMUTABLE region-presence tables — everything that does NOT
    /// change across the files of a corpus — plus a `max_regions`-sized program,
    /// for [`ResidentPresencePipeline`](crate::scan::resident_presence::ResidentPresencePipeline)
    /// to upload into backend-resident resources ONCE.
    ///
    /// The borrowed / async / prepared presence paths re-encode and re-upload the
    /// DFA transition / output / pattern-length tables and the suffix prefilter
    /// masks on EVERY dispatch (see [`Self::build_presence_by_region_dispatch`]).
    /// Those seven buffers depend only on `self`, not on the haystack or region
    /// layout, so a resident session uploads them once and re-dispatches across a
    /// corpus, transferring only the per-file haystack and the binding-6 presence
    /// reset. This is the presence sibling of
    /// [`RulePipeline::prepare_resident`](crate::scan::mega_scan::RulePipeline::prepare_resident)'s
    /// table extraction.
    ///
    /// The returned [`program`](ResidentPresenceTables::program) is sized for
    /// `max_regions` coalesced files (binding 6's element count and the kernel's
    /// `ceil_log2(max_regions)` region binary-search width); the actual per-scan
    /// region count is read dynamically from `buf_len(region_starts)`, so the same
    /// program serves any batch with `region_count <= max_regions`.
    ///
    /// Bytes are built through the same fallible `copy_u32_words_as_le_bytes` the
    /// prepared path uses, so an allocation failure fails CLOSED (`BackendError`)
    /// instead of aborting on OOM.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] when `max_regions` is zero, the pattern
    /// count exceeds the u32 GPU ABI, the presence program cannot be built, or any
    /// table allocation fails.
    pub(crate) fn resident_presence_tables(
        &self,
        max_regions: u32,
    ) -> Result<ResidentPresenceTables, vyre::BackendError> {
        let pattern_count = u32::try_from(self.pattern_lengths.len()).map_err(|_| {
            vyre::BackendError::new(
                "literal_set region-presence: pattern count exceeds u32 GPU ABI".to_string(),
            )
        })?;
        if max_regions == 0 {
            return Err(vyre::BackendError::new(
                "literal_set resident region-presence: max_regions must be >= 1 (it sizes the resident presence buffer and the kernel's region binary-search width). Fix: pass the largest coalesced-batch file count the session will scan.".to_string(),
            ));
        }
        let program = try_build_ac_bounded_ranges_suffix3_presence_by_region_program(
            &self.dfa,
            pattern_count,
            max_regions,
        )
        .map_err(vyre::BackendError::new)?;
        let prefilter_tables = self.build_prefilter_tables()?;
        let tables = self.presence_immutable_table_bytes(&prefilter_tables)?;
        let presence_words = presence_bitmap_words(pattern_count);
        let workgroup_x = program.workgroup_size[0];
        Ok(ResidentPresenceTables {
            program,
            transitions: tables.transitions,
            output_offsets: tables.output_offsets,
            output_records: tables.output_records,
            pattern_lengths: tables.pattern_lengths,
            candidate_end_mask: tables.candidate_end_mask,
            candidate_suffix2_mask: tables.candidate_suffix2_mask,
            candidate_suffix3_bloom: tables.candidate_suffix3_bloom,
            pattern_count,
            presence_words,
            workgroup_x,
        })
    }

    /// FUSED region-presence + match-positions GPU scan with a default dispatch
    /// scratch. See [`Self::scan_presence_and_positions_by_region_with_scratch`] for
    /// the hot-loop variant that reuses caller-owned staging.
    ///
    /// # Errors
    /// See [`Self::scan_presence_and_positions_by_region_with_scratch`].
    pub fn scan_presence_and_positions_by_region<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        region_starts: &[u32],
        region_base: u32,
        max_matches: u32,
        matches: &mut Vec<Match>,
    ) -> Result<Vec<u32>, vyre::BackendError> {
        let mut scratch = ScanDispatchScratch::default();
        self.scan_presence_and_positions_by_region_with_scratch(
            backend,
            haystack,
            region_starts,
            region_base,
            max_matches,
            matches,
            &mut scratch,
        )
    }

    /// FUSED region-presence + match-positions GPU scan: ONE dispatch returns BOTH
    /// the per-region presence bitmap (the return value, identical to
    /// [`Self::scan_presence_by_region`]) AND the `(pattern_id, start, end)` match
    /// triples (decoded into `matches`, identical to [`Self::scan_into`]).
    ///
    /// This collapses the two-pass pattern a coalesced consumer uses today — a
    /// presence-by-region scan to learn WHICH patterns fired per file, then a
    /// SEPARATE position scan over the same haystack to learn WHERE — into a single
    /// suffix3-gated walk. Both outputs are recall-identical to the separate scans by
    /// construction: the same candidate gate, DFA replay, and `output_records`
    /// iteration drive both, so the presence bits equal
    /// [`Self::scan_presence_by_region`]'s and the triples equal [`Self::scan_into`]'s.
    ///
    /// `region_starts` / `region_base` behave as in
    /// [`Self::scan_presence_by_region_with_scratch`]; `max_matches` bounds the triple
    /// output as in [`Self::scan_into`]. The returned bitmap is
    /// `region_starts.len() × presence_bitmap_words(pattern_count)` words.
    ///
    /// ## PERF CAVEAT (MEASURED, RTX 5090 / wgpu / release)
    /// This is a CORRECTNESS-equivalent primitive, NOT (yet) a perf win. Although it
    /// does one haystack walk instead of two, it is ~20x SLOWER than calling
    /// [`Self::scan_presence_by_region`] + [`Self::scan_into`] separately. Cause: the
    /// suffix3 prefilter inlines the replay 3x (i==0 / i==1 / general exits); the
    /// fused replay (region binary search + atomic_or + triple append) is large, so
    /// the fused kernel is ~3x bigger → register/occupancy collapse slows the whole
    /// scan. Prefer the two separate scans until the prefilter calls the replay as a
    /// function rather than inlining it (or a CUDA-backend measurement justifies the
    /// fused path). Proof + numbers: `tests/literal_set_presence_and_positions_gpu.rs`.
    ///
    /// # Errors
    /// Returns [`vyre::BackendError`] on dispatch/readback failure, scan-boundary
    /// validation, an empty or non-zero-based `region_starts`, or a pattern/region
    /// count exceeding the u32 GPU ABI.
    #[allow(clippy::too_many_arguments)]
    pub fn scan_presence_and_positions_by_region_with_scratch<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        region_starts: &[u32],
        region_base: u32,
        max_matches: u32,
        matches: &mut Vec<Match>,
        scratch: &mut ScanDispatchScratch,
    ) -> Result<Vec<u32>, vyre::BackendError> {
        use crate::scan::dispatch_io;

        matches.clear();
        let pattern_count = u32::try_from(self.pattern_lengths.len()).map_err(|_| {
            vyre::BackendError::new(
                "literal_set region-presence+positions: pattern count exceeds u32 GPU ABI"
                    .to_string(),
            )
        })?;
        let region_count = u32::try_from(region_starts.len()).map_err(|_| {
            vyre::BackendError::new(
                "literal_set region-presence+positions: region count exceeds u32 GPU ABI"
                    .to_string(),
            )
        })?;
        if region_count == 0 {
            return Err(vyre::BackendError::new(
                "literal_set region-presence+positions: region_starts must be non-empty. Fix: pass one start offset per coalesced file, beginning with 0.".to_string(),
            ));
        }
        if region_starts[0] != 0 {
            return Err(vyre::BackendError::new(
                "literal_set region-presence+positions: region_starts[0] must be 0 (the kernel binary-search lower bound). Fix: the first coalesced file must start at offset 0.".to_string(),
            ));
        }
        let total_words = presence_by_region_words(pattern_count, region_count) as usize;
        let program =
            try_build_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program(
                &self.dfa,
                pattern_count,
                region_count,
                max_matches,
            )
            .map_err(vyre::BackendError::new)?;
        let prefilter_tables = self.build_prefilter_tables()?;

        let haystack_len = dispatch_io::scan_guard(
            haystack,
            "literal_set_presence_and_positions_by_region",
            dispatch_io::DEFAULT_MAX_SCAN_BYTES,
        )?;
        dispatch_io::pack_haystack_u32_into(haystack, &mut scratch.haystack_bytes)?;
        let haystack_bytes = scratch.haystack_bytes.as_slice();
        let views = DfaPrefilterByteViews::new(&self.dfa, &self.pattern_lengths, &prefilter_tables);
        let haystack_len_word = [haystack_len];
        let haystack_len_bytes = dispatch_io::u32_words_as_le_bytes(&haystack_len_word);
        let region_starts_bytes = dispatch_io::u32_words_as_le_bytes(region_starts);
        let region_base_bytes = region_base.to_le_bytes();
        // Both read-write buffers are uploaded zeroed; the matches output (binding
        // 13) is a pure `BufferDecl::output` the backend allocates from the program.
        let presence_zeroed = vec![0u8; total_words.saturating_mul(4)];
        let match_count_bytes = [0u8; 4];

        let config =
            dispatch_io::byte_scan_dispatch_config(haystack_len, program.workgroup_size[0]);
        let borrowed_inputs: smallvec::SmallVec<[&[u8]; 13]> = [
            haystack_bytes,                         // 0: haystack (Packed U32)
            views.transitions.as_ref(),             // 1: transitions
            views.output_offsets.as_ref(),          // 2: output_offsets
            views.output_records.as_ref(),          // 3: output_records
            views.pattern_lengths.as_ref(),         // 4: pattern_lengths
            haystack_len_bytes.as_ref(),            // 5: haystack_len
            presence_zeroed.as_slice(),             // 6: per-region presence (read_write)
            views.candidate_end_mask.as_ref(),      // 7: candidate_end_mask
            views.candidate_suffix2_mask.as_ref(),  // 8: candidate_suffix2_mask
            views.candidate_suffix3_bloom.as_ref(), // 9: candidate_suffix3_bloom
            region_starts_bytes.as_ref(),           // 10: region_starts
            region_base_bytes.as_slice(),           // 11: region_base (shard offset)
            match_count_bytes.as_slice(),           // 12: match_count (read_write)
                                                    // 13: matches output (backend-allocated)
        ]
        .into_iter()
        .collect();
        let outputs = backend.dispatch_borrowed(&program, &borrowed_inputs, &config)?;

        // Output ordering = read_write + output buffers by binding:
        // presence(6) -> outputs[0], match_count(12) -> outputs[1], matches(13) -> outputs[2].
        let presence_bytes = dispatch_io::try_output_bytes(
            &outputs,
            0,
            "literal_set presence_and_positions_by_region presence",
        )?;
        let presence_words = decode_presence_words(presence_bytes, total_words);

        let count_bytes = dispatch_io::try_output_bytes(
            &outputs,
            1,
            "literal_set presence_and_positions_by_region match count",
        )?;
        let count = dispatch_io::try_read_u32_prefix(
            count_bytes,
            "literal_set presence_and_positions_by_region match count",
        )?;
        let matches_bytes = dispatch_io::try_output_bytes(
            &outputs,
            2,
            "literal_set presence_and_positions_by_region matches",
        )?;
        // The kernel's atomic match counter overcounts past the fixed cap, so a
        // count over `max_matches` means positions were dropped: fail closed
        // instead of silently decoding the truncated prefix (Law 10). Presence
        // bits stay exact (one bit per region/pattern, never truncated); it is
        // the positions list that overflows, so the overflow is surfaced here.
        dispatch_io::try_unpack_match_triples_capped_into(
            matches_bytes,
            count,
            max_matches,
            "literal_set presence_and_positions_by_region matches",
            matches,
        )?;

        Ok(presence_words)
    }

    fn scan_into_with_program<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        max_matches: u32,
        matches: &mut Vec<Match>,
        scratch: &mut ScanDispatchScratch,
        dispatch_program: &Program,
        prefilter_tables: &LiteralSetPrefilterTables,
    ) -> Result<(), vyre::BackendError> {
        use crate::scan::dispatch_io;

        matches.clear();
        let haystack_len =
            dispatch_io::scan_guard(haystack, "literal_set", dispatch_io::DEFAULT_MAX_SCAN_BYTES)?;

        // Buffer order matches the BufferDecl declaration in
        // `try_build_literal_set_program`; reordering here would silently
        // miswire the GPU program.
        dispatch_io::pack_haystack_u32_into(haystack, &mut scratch.haystack_bytes)?;
        let haystack_bytes = scratch.haystack_bytes.as_slice();
        let views = DfaPrefilterByteViews::new(&self.dfa, &self.pattern_lengths, prefilter_tables);
        let haystack_len_word = [haystack_len];
        let haystack_len_bytes = dispatch_io::u32_words_as_le_bytes(&haystack_len_word);
        let match_count_bytes = [0u8; 4];

        let config = dispatch_io::byte_scan_dispatch_config(
            haystack_len,
            dispatch_program.workgroup_size[0],
        );
        let borrowed_inputs: smallvec::SmallVec<[&[u8]; 10]> = [
            // 0: haystack (Packed U32)
            haystack_bytes,
            // 1: transitions
            views.transitions.as_ref(),
            // 2: output_offsets
            views.output_offsets.as_ref(),
            // 3: output_records
            views.output_records.as_ref(),
            // 4: pattern_lengths
            views.pattern_lengths.as_ref(),
            // 5: haystack_len
            haystack_len_bytes.as_ref(),
            // 6: match_count atomic counter
            match_count_bytes.as_slice(),
            // 7: candidate_end_mask
            views.candidate_end_mask.as_ref(),
            // 8: candidate_suffix2_mask
            views.candidate_suffix2_mask.as_ref(),
            // 9: candidate_suffix3_bloom
            views.candidate_suffix3_bloom.as_ref(),
            // 10: matches is a pure `BufferDecl::output`; the backend
            // allocates it from the Program declaration.
        ]
        .into_iter()
        .collect();
        let outputs = backend.dispatch_borrowed(&dispatch_program, &borrowed_inputs, &config)?;

        decode_literal_set_outputs_into(&outputs, max_matches, matches)?;
        Ok(())
    }

    fn count_with_program<B: VyreBackend + ?Sized>(
        &self,
        backend: &B,
        haystack: &[u8],
        scratch: &mut ScanDispatchScratch,
        count_program: &Program,
        prefilter_tables: &LiteralSetPrefilterTables,
    ) -> Result<u32, vyre::BackendError> {
        use crate::scan::dispatch_io;

        let haystack_len =
            dispatch_io::scan_guard(haystack, "literal_set", dispatch_io::DEFAULT_MAX_SCAN_BYTES)?;
        dispatch_io::pack_haystack_u32_into(haystack, &mut scratch.haystack_bytes)?;
        let haystack_bytes = scratch.haystack_bytes.as_slice();
        let transition_bytes = dispatch_io::u32_words_as_le_bytes(&self.dfa.transitions);
        let output_offset_bytes = dispatch_io::u32_words_as_le_bytes(&self.dfa.output_offsets);
        let candidate_end_mask_bytes =
            dispatch_io::u32_words_as_le_bytes(&prefilter_tables.candidate_end_mask);
        let candidate_suffix2_mask_bytes =
            dispatch_io::u32_words_as_le_bytes(&prefilter_tables.candidate_suffix2_mask);
        let candidate_suffix3_bloom_bytes =
            dispatch_io::u32_words_as_le_bytes(&prefilter_tables.candidate_suffix3_bloom);
        let haystack_len_word = [haystack_len];
        let haystack_len_bytes = dispatch_io::u32_words_as_le_bytes(&haystack_len_word);
        let match_count_bytes = [0u8; U32_COUNTER_BYTES];
        let config =
            dispatch_io::byte_scan_dispatch_config(haystack_len, count_program.workgroup_size[0]);

        let borrowed_inputs: smallvec::SmallVec<[&[u8]; 8]> = [
            // 0: haystack (Packed U32)
            haystack_bytes,
            // 1: transitions
            transition_bytes.as_ref(),
            // 2: output_offsets
            output_offset_bytes.as_ref(),
            // 3: candidate_end_mask
            candidate_end_mask_bytes.as_ref(),
            // 4: candidate_suffix2_mask
            candidate_suffix2_mask_bytes.as_ref(),
            // 5: candidate_suffix3_bloom
            candidate_suffix3_bloom_bytes.as_ref(),
            // 6: haystack_len
            haystack_len_bytes.as_ref(),
            // 7: match_count atomic counter and readback
            match_count_bytes.as_slice(),
        ]
        .into_iter()
        .collect();
        let outputs = backend.dispatch_borrowed(count_program, &borrowed_inputs, &config)?;
        decode_literal_set_count_outputs(&outputs)
    }

    fn prepare_scan_dispatch_with_program(
        &self,
        haystack: &[u8],
        max_matches: u32,
        dispatch_program: &Program,
        prefilter_tables: &LiteralSetPrefilterTables,
    ) -> Result<LiteralSetPreparedScan, vyre::BackendError> {
        use crate::scan::dispatch_io;

        let haystack_len =
            dispatch_io::scan_guard(haystack, "literal_set", dispatch_io::DEFAULT_MAX_SCAN_BYTES)?;
        let (_, matches_output_bytes) = literal_set_match_output_layout(max_matches)?;
        let mut inputs = Vec::new();
        vyre_foundation::allocation::try_reserve_vec_to_capacity(
            &mut inputs,
            LITERAL_SET_INPUT_COUNT,
        )
        .map_err(|source| {
            vyre::BackendError::new(format!(
                "literal_set prepared scan could not reserve {LITERAL_SET_INPUT_COUNT} input buffer slot(s): {source}. Fix: shard the literal set or haystack before preparing resident dispatch."
            ))
        })?;

        let mut haystack_bytes = Vec::new();
        dispatch_io::pack_haystack_u32_into(haystack, &mut haystack_bytes)?;
        inputs.push(haystack_bytes);
        inputs.push(copy_u32_words_as_le_bytes(
            &self.dfa.transitions,
            "transition table",
        )?);
        inputs.push(copy_u32_words_as_le_bytes(
            &self.dfa.output_offsets,
            "output offset table",
        )?);
        inputs.push(copy_u32_words_as_le_bytes(
            &self.dfa.output_records,
            "output record table",
        )?);
        inputs.push(copy_u32_words_as_le_bytes(
            &self.pattern_lengths,
            "pattern length table",
        )?);
        inputs.push(haystack_len.to_le_bytes().to_vec());
        inputs.push(vec![0_u8; U32_COUNTER_BYTES]);
        inputs.push(copy_u32_words_as_le_bytes(
            &prefilter_tables.candidate_end_mask,
            "candidate end mask",
        )?);
        inputs.push(copy_u32_words_as_le_bytes(
            &prefilter_tables.candidate_suffix2_mask,
            "candidate suffix2 mask",
        )?);
        inputs.push(copy_u32_words_as_le_bytes(
            &prefilter_tables.candidate_suffix3_bloom,
            "candidate suffix3 bloom",
        )?);

        let encoded_input_bytes = inputs.iter().try_fold(0_u64, |sum, input| {
            let len = u64::try_from(input.len()).map_err(|source| {
                vyre::BackendError::new(format!(
                    "literal_set prepared scan input byte length does not fit u64: {source}. Fix: shard the scan before dispatch."
                ))
            })?;
            sum.checked_add(len).ok_or_else(|| {
                vyre::BackendError::new(
                    "literal_set prepared scan input byte total overflowed u64. Fix: shard the scan before dispatch.",
                )
            })
        })?;

        Ok(LiteralSetPreparedScan {
            program: dispatch_program.clone(),
            inputs,
            dispatch_config: dispatch_io::byte_scan_dispatch_config(
                haystack_len,
                dispatch_program.workgroup_size[0],
            ),
            haystack_len,
            max_matches,
            matches_output_bytes,
            encoded_input_bytes,
        })
    }

    fn prepare_count_dispatch_with_program(
        &self,
        haystack: &[u8],
        count_program: &Program,
        prefilter_tables: &LiteralSetPrefilterTables,
    ) -> Result<LiteralSetPreparedCount, vyre::BackendError> {
        use crate::scan::dispatch_io;

        let haystack_len =
            dispatch_io::scan_guard(haystack, "literal_set", dispatch_io::DEFAULT_MAX_SCAN_BYTES)?;
        let mut inputs = Vec::new();
        vyre_foundation::allocation::try_reserve_vec_to_capacity(
            &mut inputs,
            LITERAL_SET_COUNT_INPUT_COUNT,
        )
        .map_err(|source| {
            vyre::BackendError::new(format!(
                "literal_set prepared count could not reserve {LITERAL_SET_COUNT_INPUT_COUNT} input buffer slot(s): {source}. Fix: shard the literal set or haystack before preparing resident dispatch."
            ))
        })?;

        let mut haystack_bytes = Vec::new();
        dispatch_io::pack_haystack_u32_into(haystack, &mut haystack_bytes)?;
        inputs.push(haystack_bytes);
        inputs.push(copy_u32_words_as_le_bytes(
            &self.dfa.transitions,
            "transition table",
        )?);
        inputs.push(copy_u32_words_as_le_bytes(
            &self.dfa.output_offsets,
            "output offset table",
        )?);
        inputs.push(copy_u32_words_as_le_bytes(
            &prefilter_tables.candidate_end_mask,
            "candidate end mask",
        )?);
        inputs.push(copy_u32_words_as_le_bytes(
            &prefilter_tables.candidate_suffix2_mask,
            "candidate suffix2 mask",
        )?);
        inputs.push(copy_u32_words_as_le_bytes(
            &prefilter_tables.candidate_suffix3_bloom,
            "candidate suffix3 bloom",
        )?);
        inputs.push(haystack_len.to_le_bytes().to_vec());
        inputs.push(vec![0_u8; U32_COUNTER_BYTES]);

        let encoded_input_bytes = inputs.iter().try_fold(0_u64, |sum, input| {
            let len = u64::try_from(input.len()).map_err(|source| {
                vyre::BackendError::new(format!(
                    "literal_set prepared count input byte length does not fit u64: {source}. Fix: shard the scan before dispatch."
                ))
            })?;
            sum.checked_add(len).ok_or_else(|| {
                vyre::BackendError::new(
                    "literal_set prepared count input byte total overflowed u64. Fix: shard the scan before dispatch.",
                )
            })
        })?;

        Ok(LiteralSetPreparedCount {
            program: count_program.clone(),
            inputs,
            dispatch_config: dispatch_io::byte_scan_dispatch_config(
                haystack_len,
                count_program.workgroup_size[0],
            ),
            haystack_len,
            encoded_input_bytes,
        })
    }

    fn prefilter_tables_cached<'a>(
        &'a self,
        cached_prefilter: &'a mut Option<LiteralSetPrefilterTables>,
    ) -> Result<&'a LiteralSetPrefilterTables, vyre::BackendError> {
        let pattern_fingerprint = self.pattern_fingerprint();
        let reuse_cached = cached_prefilter
            .as_ref()
            .is_some_and(|cached| cached.pattern_fingerprint == pattern_fingerprint);
        if !reuse_cached {
            *cached_prefilter =
                Some(self.build_prefilter_tables_with_fingerprint(pattern_fingerprint)?);
        }
        cached_prefilter.as_ref().ok_or_else(|| {
            vyre::BackendError::new(
                "literal_set failed to retain cached suffix-prefilter tables. Fix: retry with generic ScanDispatchScratch.",
            )
        })
    }

    fn build_prefilter_tables(&self) -> Result<LiteralSetPrefilterTables, vyre::BackendError> {
        self.build_prefilter_tables_with_fingerprint(self.pattern_fingerprint())
    }

    fn count_program_cached<'a>(
        &'a self,
        cached_count_program: &'a mut Option<CachedLiteralSetCountProgram>,
    ) -> Result<&'a Program, vyre::BackendError> {
        let pattern_fingerprint = self.pattern_fingerprint();
        let reuse_cached = cached_count_program
            .as_ref()
            .is_some_and(|cached| cached.pattern_fingerprint == pattern_fingerprint);
        if !reuse_cached {
            *cached_count_program = Some(CachedLiteralSetCountProgram {
                pattern_fingerprint,
                program: self.count_program(),
            });
        }
        cached_count_program
            .as_ref()
            .map(|cached| &cached.program)
            .ok_or_else(|| {
                vyre::BackendError::new(
                    "literal_set failed to retain the cached count program. Fix: retry without reusable scratch.",
                )
            })
    }

    fn count_program(&self) -> Program {
        build_ac_bounded_count_suffix3_prefilter_program(&self.dfa)
    }

    fn build_prefilter_tables_with_fingerprint(
        &self,
        pattern_fingerprint: u64,
    ) -> Result<LiteralSetPrefilterTables, vyre::BackendError> {
        let pattern_vectors = self.materialize_pattern_bytes()?;
        let pattern_refs = pattern_vectors
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        Ok(LiteralSetPrefilterTables {
            pattern_fingerprint,
            candidate_end_mask: literal_set_candidate_end_byte_mask_words(&pattern_refs),
            candidate_suffix2_mask: literal_set_candidate_suffix2_mask_words(&pattern_refs),
            candidate_suffix3_bloom: classic_ac_candidate_suffix3_bloom_words(&pattern_refs),
        })
    }

    fn materialize_pattern_bytes(&self) -> Result<Vec<Vec<u8>>, vyre::BackendError> {
        if self.pattern_offsets.len() != self.pattern_lengths.len() {
            return Err(vyre::BackendError::new(format!(
                "literal_set pattern metadata is malformed: {} offsets for {} lengths. Fix: rebuild the literal set with GpuLiteralSet::try_compile before dispatch.",
                self.pattern_offsets.len(),
                self.pattern_lengths.len()
            )));
        }

        let mut patterns = Vec::new();
        vyre_foundation::allocation::try_reserve_vec_to_capacity(
            &mut patterns,
            self.pattern_lengths.len(),
        )
        .map_err(|source| {
            vyre::BackendError::new(format!(
                "literal_set could not reserve {} decoded pattern slot(s): {source}. Fix: shard the pattern set before dispatch.",
                self.pattern_lengths.len()
            ))
        })?;

        for (pattern_index, (&offset, &len)) in self
            .pattern_offsets
            .iter()
            .zip(&self.pattern_lengths)
            .enumerate()
        {
            let start = usize::try_from(offset).map_err(|source| {
                vyre::BackendError::new(format!(
                    "literal_set pattern {pattern_index} offset {offset} cannot fit host usize: {source}. Fix: rebuild the literal set with GpuLiteralSet::try_compile before dispatch."
                ))
            })?;
            let len = usize::try_from(len).map_err(|source| {
                vyre::BackendError::new(format!(
                    "literal_set pattern {pattern_index} length {len} cannot fit host usize: {source}. Fix: rebuild the literal set with GpuLiteralSet::try_compile before dispatch."
                ))
            })?;
            let end = start.checked_add(len).ok_or_else(|| {
                vyre::BackendError::new(format!(
                    "literal_set pattern {pattern_index} byte range overflows host usize. Fix: rebuild the literal set with GpuLiteralSet::try_compile before dispatch."
                ))
            })?;
            let words = self.pattern_bytes.get(start..end).ok_or_else(|| {
                vyre::BackendError::new(format!(
                    "literal_set pattern {pattern_index} byte range {start}..{end} exceeds packed pattern byte table length {}. Fix: rebuild the literal set with GpuLiteralSet::try_compile before dispatch.",
                    self.pattern_bytes.len()
                ))
            })?;
            let mut pattern = Vec::new();
            vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut pattern, words.len())
                .map_err(|source| {
                    vyre::BackendError::new(format!(
                        "literal_set could not reserve {} byte(s) for pattern {pattern_index}: {source}. Fix: shard the pattern set before dispatch.",
                        words.len()
                    ))
                })?;
            for (byte_index, &word) in words.iter().enumerate() {
                let byte = u8::try_from(word).map_err(|source| {
                    vyre::BackendError::new(format!(
                        "literal_set pattern {pattern_index} byte {byte_index} has non-byte word {word}: {source}. Fix: rebuild the literal set with GpuLiteralSet::try_compile before dispatch."
                    ))
                })?;
                pattern.push(byte);
            }
            patterns.push(pattern);
        }

        Ok(patterns)
    }

    fn pattern_fingerprint(&self) -> u64 {
        let mut hash = fnv1a64_initial_state();
        for words in [
            self.pattern_offsets.as_slice(),
            self.pattern_lengths.as_slice(),
            self.pattern_bytes.as_slice(),
        ] {
            for &word in words {
                for byte in word.to_le_bytes() {
                    hash = fnv1a64_update_byte(hash, byte);
                }
            }
        }
        hash
    }

    fn program_for_match_capacity_cached<'a>(
        &'a self,
        max_matches: u32,
        cached_program: &'a mut Option<CachedLiteralSetProgram>,
    ) -> Result<&'a Program, vyre::BackendError> {
        let (declared_words, readback_bytes) = literal_set_match_output_layout(max_matches)?;
        if self.compiled_matches_output_satisfies(declared_words, readback_bytes)? {
            return Ok(&self.program);
        }

        let base_fingerprint = self.program.fingerprint();
        let reuse_cached = cached_program.as_ref().is_some_and(|cached| {
            cached.max_matches == max_matches && cached.base_fingerprint == base_fingerprint
        });
        if !reuse_cached {
            let program = self.rewrite_program_for_match_layout(declared_words, readback_bytes);
            *cached_program = Some(CachedLiteralSetProgram {
                base_fingerprint,
                max_matches,
                program,
            });
        }

        match cached_program.as_ref() {
            Some(cached) => Ok(&cached.program),
            None => Err(vyre::BackendError::new(
                "literal_set failed to retain the cached match-capacity program. Fix: retry with generic ScanDispatchScratch.",
            )),
        }
    }

    fn program_for_match_capacity(
        &self,
        max_matches: u32,
    ) -> Result<Cow<'_, Program>, vyre::BackendError> {
        let (declared_words, readback_bytes) = literal_set_match_output_layout(max_matches)?;
        if self.compiled_matches_output_satisfies(declared_words, readback_bytes)? {
            return Ok(Cow::Borrowed(&self.program));
        }

        Ok(Cow::Owned(self.rewrite_program_for_match_layout(
            declared_words,
            readback_bytes,
        )))
    }

    fn compiled_matches_output_satisfies(
        &self,
        declared_words: u32,
        readback_bytes: usize,
    ) -> Result<bool, vyre::BackendError> {
        let matches_output = self
            .program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "matches" && buffer.is_output())
            .ok_or_else(|| {
                vyre::BackendError::new(
                    "literal_set program is missing its matches output buffer. Fix: rebuild the literal set with GpuLiteralSet::try_compile before dispatch.",
                )
            })?;

        Ok(matches_output.count == declared_words
            && (matches_output.output_byte_range().is_none()
                || matches_output.output_byte_range() == Some(0..readback_bytes)))
    }

    fn rewrite_program_for_match_layout(
        &self,
        declared_words: u32,
        readback_bytes: usize,
    ) -> Program {
        let buffers = self
            .program
            .buffers()
            .iter()
            .cloned()
            .map(|buffer| {
                if buffer.name() == "matches" && buffer.is_output() {
                    buffer
                        .with_count(declared_words)
                        .with_output_byte_range(0..readback_bytes)
                } else {
                    buffer
                }
            })
            .collect::<Vec<_>>();

        self.program.with_rewritten_buffers(buffers)
    }

    /// Serialize this matcher into a self-describing binary blob suitable
    /// for on-disk caching. Composed from the existing layer-1 wire
    /// formats: `Program::to_bytes` for the dispatch IR and
    /// `CompiledDfa::to_bytes` for the transition tables. The pattern
    /// arrays are packed as raw little-endian `u32` words.
    ///
    /// Layout:
    ///   - 4 bytes magic `b"VLIT"`
    ///   - 4 bytes wire version (LE u32)
    ///   - 4 bytes program byte length (LE u32)  + program bytes
    ///   - 4 bytes dfa byte length (LE u32)      + dfa bytes
    ///   - 4 bytes pattern_offsets word count    + words
    ///   - 4 bytes pattern_lengths word count    + words
    ///   - 4 bytes pattern_bytes word count      + words
    ///
    /// Caller-side cache invalidation: the dispatch `Program` already
    /// includes vyre's IR wire version inside its own framing, so a stale
    /// current-version cache surfaces as `LiteralSetWireError::
    /// InvalidProgram` from `Program::from_bytes` (or as a bad magic /
    /// version on this outer envelope). Legacy literal-compare and bounded-DFA
    /// blobs are migrated by decoding their DFA/pattern sections and
    /// rebuilding the current suffix-prefiltered bounded-DFA dispatch program.
    /// # Errors
    /// Returns [`LiteralSetWireError::WireFraming`] if any section
    /// exceeds the envelope's `u32` length-prefix capacity.
    pub fn to_bytes(&self) -> Result<Vec<u8>, LiteralSetWireError> {
        let mut w = vyre_foundation::serial::envelope::WireWriter::new(
            LITERAL_SET_WIRE_MAGIC,
            LITERAL_SET_WIRE_VERSION,
        );
        w.write_section(&self.program.to_bytes())
            .map_err(LiteralSetWireError::WireFraming)?;
        let dfa_bytes = self
            .dfa
            .to_bytes()
            .map_err(LiteralSetWireError::InvalidDfa)?;
        w.write_section(&dfa_bytes)
            .map_err(LiteralSetWireError::WireFraming)?;
        w.write_words(&self.pattern_offsets)
            .map_err(LiteralSetWireError::WireFraming)?;
        w.write_words(&self.pattern_lengths)
            .map_err(LiteralSetWireError::WireFraming)?;
        w.write_words(&self.pattern_bytes)
            .map_err(LiteralSetWireError::WireFraming)?;
        Ok(w.into_bytes())
    }

    /// Decode a `GpuLiteralSet` from a blob produced by [`Self::to_bytes`].
    ///
    /// # Errors
    /// Returns [`LiteralSetWireError`] when the envelope rejects the
    /// outer header, or any inner section (program, DFA) is itself
    /// rejected.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, LiteralSetWireError> {
        let (mut r, wire_version) =
            literal_set_wire_reader(bytes).map_err(LiteralSetWireError::WireFraming)?;

        let program_bytes = r.read_section().map_err(LiteralSetWireError::WireFraming)?;
        if wire_version == LITERAL_SET_WIRE_VERSION {
            Program::from_bytes(program_bytes)
                .map_err(|e| LiteralSetWireError::InvalidProgram(format!("{e}")))?;
        }

        let dfa_bytes = r.read_section().map_err(LiteralSetWireError::WireFraming)?;
        let dfa = CompiledDfa::from_bytes(dfa_bytes).map_err(LiteralSetWireError::InvalidDfa)?;

        let pattern_offsets = r.read_words().map_err(LiteralSetWireError::WireFraming)?;
        let pattern_lengths = r.read_words().map_err(LiteralSetWireError::WireFraming)?;
        let pattern_bytes = r.read_words().map_err(LiteralSetWireError::WireFraming)?;
        let pattern_count =
            u32::try_from(pattern_lengths.len()).map_err(|source| {
                LiteralSetWireError::InvalidProgram(format!(
                    "literal_set decoded pattern length count {} exceeds u32 GPU buffer metadata: {source}. Fix: shard the pattern set before caching.",
                    pattern_lengths.len()
                ))
            })?;
        let program = try_build_literal_set_program(&dfa, pattern_count).map_err(|message| {
            LiteralSetWireError::InvalidProgram(format!(
                "literal_set decoded DFA cannot rebuild current dispatch Program: {message}"
            ))
        })?;

        Ok(Self {
            dfa,
            pattern_bytes,
            pattern_offsets,
            pattern_lengths,
            program,
        })
    }
}

fn literal_set_wire_reader(
    bytes: &[u8],
) -> Result<
    (vyre_foundation::serial::envelope::WireReader<'_>, u32),
    vyre_foundation::serial::envelope::EnvelopeError,
> {
    match vyre_foundation::serial::envelope::WireReader::new(
        bytes,
        LITERAL_SET_WIRE_MAGIC,
        LITERAL_SET_WIRE_VERSION,
    ) {
        Ok(reader) => Ok((reader, LITERAL_SET_WIRE_VERSION)),
        Err(vyre_foundation::serial::envelope::EnvelopeError::VersionMismatch {
            found:
                legacy_version @ (LITERAL_SET_LEGACY_LITERAL_COMPARE_WIRE_VERSION
                | LITERAL_SET_LEGACY_BOUNDED_DFA_WIRE_VERSION),
            ..
        }) => vyre_foundation::serial::envelope::WireReader::new(
            bytes,
            LITERAL_SET_WIRE_MAGIC,
            legacy_version,
        )
        .map(|reader| (reader, legacy_version)),
        Err(error) => Err(error),
    }
}

fn literal_set_candidate_end_byte_mask_words(patterns: &[&[u8]]) -> [u32; 8] {
    let mut mask = [0_u32; 8];
    for pattern in patterns
        .iter()
        .copied()
        .filter(|pattern| !pattern.is_empty())
    {
        let byte = usize::from(pattern[pattern.len() - 1]);
        mask[byte / 32] |= 1_u32 << (byte % 32);
    }
    mask
}

fn literal_set_candidate_suffix2_mask_words(
    patterns: &[&[u8]],
) -> [u32; CLASSIC_AC_SUFFIX2_MASK_WORDS] {
    let mut mask = [0_u32; CLASSIC_AC_SUFFIX2_MASK_WORDS];
    for pattern in patterns
        .iter()
        .copied()
        .filter(|pattern| !pattern.is_empty())
    {
        match pattern.len() {
            1 => {
                let current = usize::from(pattern[0]);
                for previous in 0..=u8::MAX {
                    set_suffix2_candidate_bit(&mut mask, usize::from(previous), current);
                }
            }
            len => {
                set_suffix2_candidate_bit(
                    &mut mask,
                    usize::from(pattern[len - 2]),
                    usize::from(pattern[len - 1]),
                );
            }
        }
    }
    mask
}

fn set_suffix2_candidate_bit(
    mask: &mut [u32; CLASSIC_AC_SUFFIX2_MASK_WORDS],
    previous: usize,
    current: usize,
) {
    let suffix = (previous << 8) | current;
    mask[suffix / 32] |= 1_u32 << (suffix % 32);
}

fn reserve_vec<T>(
    vec: &mut Vec<T>,
    requested: usize,
    field: &'static str,
) -> Result<(), LiteralSetCompileError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(vec, requested).map_err(
        |source: TryReserveError| LiteralSetCompileError::StorageReserveFailed {
            field,
            requested,
            message: source.to_string(),
        },
    )
}

fn copy_u32_words_as_le_bytes(
    words: &[u32],
    field: &'static str,
) -> Result<Vec<u8>, vyre::BackendError> {
    let byte_len = words.len().checked_mul(U32_BYTES).ok_or_else(|| {
        vyre::BackendError::new(format!(
            "literal_set prepared scan {field} byte length overflowed host usize. Fix: shard the literal set before preparing resident dispatch."
        ))
    })?;
    let mut bytes = Vec::new();
    vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut bytes, byte_len).map_err(
        |source| {
            vyre::BackendError::new(format!(
                "literal_set prepared scan could not reserve {byte_len} byte(s) for {field}: {source}. Fix: shard the literal set before preparing resident dispatch."
            ))
        },
    )?;
    if cfg!(target_endian = "little") {
        bytes.extend_from_slice(bytemuck::cast_slice(words));
    } else {
        for &word in words {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    Ok(bytes)
}

/// Decode the first `total_words` little-endian `u32` words of a presence readback
/// into `out` (cleared first). The single decoder for EVERY region-presence wire
/// result — sync, async, prepared, fused, and resident — so the bit layout has one
/// source. Trailing bytes beyond `total_words` are ignored: a resident readback may
/// return the full buffer capacity, of which only the used prefix is meaningful.
pub(crate) fn decode_presence_words_into(
    presence_bytes: &[u8],
    total_words: usize,
    out: &mut Vec<u32>,
) {
    out.clear();
    out.extend(
        presence_bytes
            .chunks_exact(4)
            .take(total_words)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])),
    );
}

/// [`decode_presence_words_into`] returning a freshly-allocated `Vec`.
pub(crate) fn decode_presence_words(presence_bytes: &[u8], total_words: usize) -> Vec<u32> {
    let mut out = Vec::new();
    decode_presence_words_into(presence_bytes, total_words, &mut out);
    out
}

fn decode_literal_set_outputs_into(
    outputs: &[Vec<u8>],
    max_matches: u32,
    matches: &mut Vec<Match>,
) -> Result<(), vyre::BackendError> {
    let count_bytes =
        crate::scan::dispatch_io::try_output_bytes(outputs, 0, "literal_set match count")?;
    let count =
        crate::scan::dispatch_io::try_read_u32_prefix(count_bytes, "literal_set match count")?;
    let matches_bytes =
        crate::scan::dispatch_io::try_output_bytes(outputs, 1, "literal_set matches")?;

    // The kernel's atomic match counter overcounts past the fixed cap, so a
    // count over `max_matches` means matches were dropped: fail closed instead
    // of silently decoding the truncated prefix (Law 10).
    crate::scan::dispatch_io::try_unpack_match_triples_capped_into(
        matches_bytes,
        count,
        max_matches,
        "literal_set matches",
        matches,
    )
}

fn decode_literal_set_count_outputs(outputs: &[Vec<u8>]) -> Result<u32, vyre::BackendError> {
    let count_bytes = crate::scan::dispatch_io::try_output_bytes(outputs, 0, "literal_set count")?;
    crate::scan::dispatch_io::try_read_u32_prefix(count_bytes, "literal_set count")
}

fn literal_set_match_triple_bytes(count: u32) -> Result<usize, vyre::BackendError> {
    let words = count.checked_mul(MATCH_TRIPLE_WORDS).ok_or_else(|| {
        vyre::BackendError::new(format!(
            "literal_set match count {count} overflows the GPU match-output word count. Fix: lower max_matches or split the scan before dispatch."
        ))
    })?;
    usize::try_from(words)
        .ok()
        .and_then(|words| words.checked_mul(U32_BYTES))
        .ok_or_else(|| {
            vyre::BackendError::new(format!(
                "literal_set match count {count} overflows host match-output byte sizing. Fix: lower max_matches or split the scan before dispatch."
            ))
        })
}

fn literal_set_match_output_layout(max_matches: u32) -> Result<(u32, usize), vyre::BackendError> {
    let words = max_matches.checked_mul(MATCH_TRIPLE_WORDS).ok_or_else(|| {
        vyre::BackendError::new(format!(
            "literal_set max_matches={max_matches} overflows the GPU match-output word count. Fix: lower max_matches or split the scan before dispatch."
        ))
    })?;
    let byte_len = literal_set_match_triple_bytes(max_matches)?;
    Ok((words.max(1), byte_len))
}

#[cfg(test)]
mod compile_tests {
    use super::*;

    #[derive(Clone)]
    struct LiteralReadbackBackend {
        outputs: Vec<Vec<u8>>,
    }

    impl vyre::backend::private::Sealed for LiteralReadbackBackend {}

    impl VyreBackend for LiteralReadbackBackend {
        fn id(&self) -> &'static str {
            "literal-readback-test"
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

    #[derive(Clone)]
    struct RecordingLiteralBackend {
        outputs: Vec<Vec<u8>>,
        observed_matches_layouts:
            std::sync::Arc<std::sync::Mutex<Vec<(u32, Option<std::ops::Range<usize>>)>>>,
        observed_program_buffer_ptrs: std::sync::Arc<std::sync::Mutex<Vec<usize>>>,
        observed_input_lengths: std::sync::Arc<std::sync::Mutex<Vec<Vec<usize>>>>,
    }

    impl RecordingLiteralBackend {
        fn new(outputs: Vec<Vec<u8>>) -> Self {
            Self {
                outputs,
                observed_matches_layouts: std::sync::Arc::default(),
                observed_program_buffer_ptrs: std::sync::Arc::default(),
                observed_input_lengths: std::sync::Arc::default(),
            }
        }

        fn observed_matches_layouts(&self) -> Vec<(u32, Option<std::ops::Range<usize>>)> {
            self.observed_matches_layouts
                .lock()
                .expect("Fix: recording literal backend mutex should not be poisoned")
                .clone()
        }

        fn observed_program_buffer_ptrs(&self) -> Vec<usize> {
            self.observed_program_buffer_ptrs
                .lock()
                .expect("Fix: recording literal backend mutex should not be poisoned")
                .clone()
        }

        fn observed_input_lengths(&self) -> Vec<Vec<usize>> {
            self.observed_input_lengths
                .lock()
                .expect("Fix: recording literal backend mutex should not be poisoned")
                .clone()
        }
    }

    impl vyre::backend::private::Sealed for RecordingLiteralBackend {}

    impl VyreBackend for RecordingLiteralBackend {
        fn id(&self) -> &'static str {
            "literal-recording-test"
        }

        fn dispatch(
            &self,
            program: &Program,
            inputs: &[Vec<u8>],
            config: &vyre::DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, vyre::BackendError> {
            let borrowed = inputs.iter().map(Vec::as_slice).collect::<Vec<_>>();
            self.dispatch_borrowed(program, &borrowed, config)
        }

        fn dispatch_borrowed(
            &self,
            program: &Program,
            inputs: &[&[u8]],
            _config: &vyre::DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, vyre::BackendError> {
            let matches = program
                .buffers()
                .iter()
                .find(|buffer| buffer.name() == "matches")
                .ok_or_else(|| vyre::BackendError::new("test program omitted matches buffer"))?;
            self.observed_matches_layouts
                .lock()
                .map_err(|_| vyre::BackendError::new("test observation mutex poisoned"))?
                .push((matches.count, matches.output_byte_range()));
            self.observed_program_buffer_ptrs
                .lock()
                .map_err(|_| vyre::BackendError::new("test observation mutex poisoned"))?
                .push(program.buffers().as_ptr() as usize);
            self.observed_input_lengths
                .lock()
                .map_err(|_| vyre::BackendError::new("test observation mutex poisoned"))?
                .push(inputs.iter().map(|input| input.len()).collect());
            Ok(self.outputs.clone())
        }
    }

    #[derive(Clone)]
    struct RecordingCountBackend {
        outputs: Vec<Vec<u8>>,
        observed_input_lengths: std::sync::Arc<std::sync::Mutex<Vec<Vec<usize>>>>,
        observed_buffer_names: std::sync::Arc<std::sync::Mutex<Vec<Vec<String>>>>,
    }

    impl RecordingCountBackend {
        fn new(outputs: Vec<Vec<u8>>) -> Self {
            Self {
                outputs,
                observed_input_lengths: std::sync::Arc::default(),
                observed_buffer_names: std::sync::Arc::default(),
            }
        }

        fn observed_input_lengths(&self) -> Vec<Vec<usize>> {
            self.observed_input_lengths
                .lock()
                .expect("Fix: recording count backend mutex should not be poisoned")
                .clone()
        }

        fn observed_buffer_names(&self) -> Vec<Vec<String>> {
            self.observed_buffer_names
                .lock()
                .expect("Fix: recording count backend mutex should not be poisoned")
                .clone()
        }
    }

    impl vyre::backend::private::Sealed for RecordingCountBackend {}

    impl VyreBackend for RecordingCountBackend {
        fn id(&self) -> &'static str {
            "literal-count-recording-test"
        }

        fn dispatch(
            &self,
            program: &Program,
            inputs: &[Vec<u8>],
            config: &vyre::DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, vyre::BackendError> {
            let borrowed = inputs.iter().map(Vec::as_slice).collect::<Vec<_>>();
            self.dispatch_borrowed(program, &borrowed, config)
        }

        fn dispatch_borrowed(
            &self,
            program: &Program,
            inputs: &[&[u8]],
            _config: &vyre::DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, vyre::BackendError> {
            self.observed_input_lengths
                .lock()
                .map_err(|_| vyre::BackendError::new("test observation mutex poisoned"))?
                .push(inputs.iter().map(|input| input.len()).collect());
            self.observed_buffer_names
                .lock()
                .map_err(|_| vyre::BackendError::new("test observation mutex poisoned"))?
                .push(
                    program
                        .buffers()
                        .iter()
                        .map(|buffer| buffer.name().to_string())
                        .collect(),
                );
            Ok(self.outputs.clone())
        }
    }

    fn match_count_bytes(count: u32) -> Vec<u8> {
        count.to_le_bytes().to_vec()
    }

    fn match_triple_bytes(pattern_id: u32, start: u32, end: u32) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12);
        bytes.extend_from_slice(&pattern_id.to_le_bytes());
        bytes.extend_from_slice(&start.to_le_bytes());
        bytes.extend_from_slice(&end.to_le_bytes());
        bytes
    }

    fn decode_u32_words(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    fn decode_reference_matches(outputs: &[vyre_reference::value::Value]) -> Vec<Match> {
        let count = decode_u32_words(&outputs[0].to_bytes())[0] as usize;
        decode_u32_words(&outputs[1].to_bytes())
            .into_iter()
            .take(count.saturating_mul(3))
            .collect::<Vec<_>>()
            .chunks_exact(3)
            .map(|chunk| Match::new(chunk[0], chunk[1], chunk[2]))
            .collect()
    }

    #[test]
    fn decode_outputs_fails_closed_when_count_exceeds_cap() {
        // Law 10 regression at the shared literal_set decode call site
        // (`decode_literal_set_outputs_into`, used by GpuLiteralSet::scan): the
        // kernel's atomic counter reports 7 matches into a buffer holding the cap
        // of 3 triples. The capped decode must error (naming the 4 dropped
        // matches), not silently return the truncated 3.
        let mut triples = Vec::new();
        for i in 0..3u32 {
            triples.extend_from_slice(&match_triple_bytes(0, i, i + 1));
        }
        let outputs = vec![match_count_bytes(7), triples];
        let mut matches = vec![Match::new(5, 5, 5)];
        let err = decode_literal_set_outputs_into(&outputs, 3, &mut matches)
            .expect_err("count 7 over cap 3 must fail closed, not truncate");
        let msg = err.to_string();
        assert!(
            msg.contains("literal_set matches")
                && msg.contains("exceeds the output-buffer cap 3")
                && msg.contains("drop 4 match(es)")
                && matches.is_empty(),
            "literal_set decode must surface the dropped-match overflow and expose no partial set: {msg}"
        );
    }

    #[test]
    fn decode_outputs_decodes_exact_set_within_cap() {
        // Positive twin: a count within the cap decodes the real triples verbatim
        // (the buffer physically holds 4 slots; the counter reports 2).
        let mut triples = Vec::new();
        triples.extend_from_slice(&match_triple_bytes(2, 0, 2));
        triples.extend_from_slice(&match_triple_bytes(5, 3, 6));
        triples.extend_from_slice(&match_triple_bytes(0, 0, 0));
        triples.extend_from_slice(&match_triple_bytes(0, 0, 0));
        let outputs = vec![match_count_bytes(2), triples];
        let mut matches = Vec::new();
        decode_literal_set_outputs_into(&outputs, 4, &mut matches)
            .expect("count 2 within cap 4 must decode");
        assert_eq!(matches, vec![Match::new(2, 0, 2), Match::new(5, 3, 6)]);
    }

    #[test]
    fn try_compile_packs_offsets_lengths_and_bytes_without_truncation() {
        let compiled = GpuLiteralSet::try_compile(&[b"ab".as_slice(), b"cde".as_slice()])
            .expect("Fix: small literal set must compile");

        assert_eq!(compiled.pattern_offsets, vec![0, 2]);
        assert_eq!(compiled.pattern_lengths, vec![2, 3]);
        assert_eq!(
            compiled.pattern_bytes,
            vec![
                b'a' as u32,
                b'b' as u32,
                b'c' as u32,
                b'd' as u32,
                b'e' as u32
            ]
        );
    }

    #[test]
    fn compile_empty_patterns_matches_fallible_compile_contract() {
        let compat = GpuLiteralSet::compile(&[]);
        let fallible =
            GpuLiteralSet::try_compile(&[]).expect("Fix: empty literal set must compile");

        assert_eq!(compat.pattern_offsets, fallible.pattern_offsets);
        assert_eq!(compat.pattern_lengths, fallible.pattern_lengths);
        assert_eq!(compat.pattern_bytes, fallible.pattern_bytes);
    }

    #[test]
    fn literal_prefilter_masks_are_derived_from_literal_suffixes() {
        let patterns: [&[u8]; 3] = [b"a", b"bc", b"token"];
        let end_mask = literal_set_candidate_end_byte_mask_words(&patterns);
        let suffix2_mask = literal_set_candidate_suffix2_mask_words(&patterns);

        let end_contains = |byte: u8| {
            let byte = usize::from(byte);
            (end_mask[byte / 32] & (1_u32 << (byte % 32))) != 0
        };
        let suffix2_contains = |previous: u8, current: u8| {
            let suffix = (usize::from(previous) << 8) | usize::from(current);
            (suffix2_mask[suffix / 32] & (1_u32 << (suffix % 32))) != 0
        };

        assert!(end_contains(b'a'));
        assert!(end_contains(b'c'));
        assert!(end_contains(b'n'));
        assert!(!end_contains(b'z'));
        assert!(suffix2_contains(0, b'a'));
        assert!(suffix2_contains(u8::MAX, b'a'));
        assert!(suffix2_contains(b'b', b'c'));
        assert!(suffix2_contains(b'e', b'n'));
        assert!(!suffix2_contains(b'x', b'n'));
    }

    #[test]
    fn prepare_literal_scratch_populates_reusable_program_and_prefilter_tables() {
        let engine =
            GpuLiteralSet::try_compile(&[b"a".as_slice(), b"bc".as_slice(), b"token".as_slice()])
                .expect("Fix: small literal set must compile");
        let mut scratch = LiteralSetScanScratch::default();

        engine
            .prepare_literal_scratch(3, &mut scratch)
            .expect("Fix: literal hot-loop scratch preparation should build derived state");

        assert!(
            scratch.cached_program.is_some(),
            "Fix: non-default match cap should prepare a reusable rewritten Program."
        );
        let prefilter = scratch
            .cached_prefilter
            .as_ref()
            .expect("Fix: scratch preparation should cache suffix-prefilter tables.");
        assert_ne!(
            prefilter.candidate_end_mask, [0; 8],
            "Fix: suffix-prefilter preparation must materialize candidate-end bits."
        );
        assert!(
            prefilter
                .candidate_suffix2_mask
                .iter()
                .any(|&word| word != 0),
            "Fix: suffix-prefilter preparation must materialize suffix2 candidate bits."
        );
        assert!(
            prefilter
                .candidate_suffix3_bloom
                .iter()
                .any(|&word| word != 0),
            "Fix: suffix-prefilter preparation must materialize suffix3 candidate bits."
        );
        assert!(scratch.cached_count_program.is_none());
    }

    #[test]
    fn prepare_count_scratch_populates_count_program_and_prefilter_tables() {
        let engine =
            GpuLiteralSet::try_compile(&[b"a".as_slice(), b"bc".as_slice(), b"token".as_slice()])
                .expect("Fix: small literal set must compile");
        let mut scratch = LiteralSetScanScratch::default();

        engine
            .prepare_count_scratch(&mut scratch)
            .expect("Fix: literal count scratch preparation should build derived state");

        assert!(
            scratch.cached_count_program.is_some(),
            "Fix: count hot-loop scratch should prepare the count-only program."
        );
        assert!(
            scratch.cached_prefilter.is_some(),
            "Fix: count hot-loop scratch should prepare suffix-prefilter tables."
        );
        assert!(
            scratch.cached_program.is_none(),
            "Fix: count scratch preparation should not build match-list output programs."
        );
    }

    #[test]
    fn prepare_scan_dispatch_matches_borrowed_input_layout() {
        let engine =
            GpuLiteralSet::try_compile(&[b"a".as_slice(), b"bc".as_slice(), b"token".as_slice()])
                .expect("Fix: small literal set must compile");
        let plan = engine
            .prepare_scan_dispatch(b"xx token bc a", 3)
            .expect("Fix: prepared literal scan dispatch should own input buffers");
        let backend = RecordingLiteralBackend::new(vec![match_count_bytes(0), Vec::new()]);
        let mut matches = Vec::new();

        engine
            .scan_into(&backend, b"xx token bc a", 3, &mut matches)
            .expect("Fix: recording backend should accept literal scan");

        assert_eq!(plan.inputs.len(), LITERAL_SET_INPUT_COUNT);
        assert_eq!(
            backend.observed_input_lengths()[0],
            plan.inputs.iter().map(Vec::len).collect::<Vec<_>>(),
            "Fix: prepared dispatch buffers must stay in the same ABI order as direct scan dispatch."
        );
        assert_eq!(
            plan.dispatch_config.grid_override,
            Some([1, 1, 1]),
            "Fix: prepared dispatch must preserve byte-scan grid geometry."
        );
        assert_eq!(plan.match_count_readback_bytes(), U32_COUNTER_BYTES);
        assert_eq!(
            plan.match_triples_readback_bytes(u32::MAX)
                .expect("Fix: clamped readback sizing should not overflow"),
            plan.matches_output_bytes
        );
        assert_eq!(
            plan.encoded_input_bytes,
            plan.inputs
                .iter()
                .map(|input| input.len() as u64)
                .sum::<u64>()
        );
    }

    #[test]
    fn prepared_scan_decodes_resident_style_readback() {
        let engine = GpuLiteralSet::try_compile(&[b"a".as_slice(), b"bc".as_slice()])
            .expect("Fix: small literal set must compile");
        let plan = engine
            .prepare_scan_dispatch(b"abc", 2)
            .expect("Fix: prepared literal scan dispatch should build");
        let outputs = vec![
            match_count_bytes(2),
            [match_triple_bytes(0, 0, 1), match_triple_bytes(1, 1, 3)].concat(),
        ];
        let mut matches = Vec::new();

        plan.decode_outputs_into(&outputs, &mut matches)
            .expect("Fix: prepared scan decoder should read count plus match triples");

        assert_eq!(
            matches,
            vec![Match::new(0, 0, 1), Match::new(1, 1, 3)],
            "Fix: prepared dispatch decode must match public GpuLiteralSet scan semantics."
        );
    }

    #[test]
    fn literal_count_uses_count_only_program_and_readback() {
        let engine = GpuLiteralSet::try_compile(&[b"a".as_slice(), b"bc".as_slice()])
            .expect("Fix: small literal set must compile");
        let backend = RecordingCountBackend::new(vec![match_count_bytes(3)]);
        let mut scratch = LiteralSetScanScratch::default();

        let count = engine
            .count_with_literal_scratch(&backend, b"abcabc", &mut scratch)
            .expect("Fix: literal count dispatch should decode one count output");

        assert_eq!(count, 3);
        assert_eq!(
            backend.observed_input_lengths()[0].len(),
            LITERAL_SET_COUNT_INPUT_COUNT,
            "Fix: count-only dispatch must not upload output_records or pattern lengths."
        );
        assert_eq!(
            backend.observed_buffer_names()[0],
            vec![
                "haystack",
                "transitions",
                "output_offsets",
                "candidate_end_mask",
                "candidate_suffix2_mask",
                "candidate_suffix3_bloom",
                "haystack_len",
                "match_count"
            ],
            "Fix: literal count must dispatch the suffix3 count program ABI."
        );
        assert!(
            scratch.cached_count_program.is_some(),
            "Fix: count hot loops should reuse the count program."
        );
    }

    #[test]
    fn prepare_count_dispatch_matches_count_input_layout() {
        let engine = GpuLiteralSet::try_compile(&[b"a".as_slice(), b"bc".as_slice()])
            .expect("Fix: small literal set must compile");
        let plan = engine
            .prepare_count_dispatch(b"abcabc")
            .expect("Fix: prepared literal count dispatch should own input buffers");
        let backend = RecordingCountBackend::new(vec![match_count_bytes(3)]);

        let count = engine
            .count(&backend, b"abcabc")
            .expect("Fix: recording backend should accept literal count");

        assert_eq!(count, 3);
        assert_eq!(plan.inputs.len(), LITERAL_SET_COUNT_INPUT_COUNT);
        assert_eq!(
            backend.observed_input_lengths()[0],
            plan.inputs.iter().map(Vec::len).collect::<Vec<_>>(),
            "Fix: prepared count buffers must stay in the same ABI order as direct count dispatch."
        );
        assert_eq!(plan.dispatch_config.grid_override, Some([1, 1, 1]));
        assert_eq!(plan.count_readback_bytes(), U32_COUNTER_BYTES);
        assert_eq!(
            plan.decode_outputs(&[match_count_bytes(3)])
                .expect("Fix: prepared count decoder should read one u32"),
            3
        );
        assert_eq!(
            plan.encoded_input_bytes,
            plan.inputs
                .iter()
                .map(|input| input.len() as u64)
                .sum::<u64>()
        );
    }

    #[test]
    fn reserve_vec_reports_compile_storage_failure() {
        let mut scratch = Vec::<u8>::new();
        let error = reserve_vec(&mut scratch, usize::MAX, "adversarial scratch")
            .expect_err("Fix: usize::MAX reserve must fail instead of silently truncating");

        match error {
            LiteralSetCompileError::StorageReserveFailed {
                field, requested, ..
            } => {
                assert_eq!(field, "adversarial scratch");
                assert_eq!(requested, usize::MAX);
            }
            other => panic!("expected storage reserve failure, got {other:?}"),
        }
        assert!(scratch.is_empty());
    }

    #[test]
    fn literal_scan_rejects_short_match_count_readback() {
        let engine = GpuLiteralSet::compile(&[b"a".as_slice()]);
        let backend = LiteralReadbackBackend {
            outputs: vec![vec![1, 2, 3], Vec::new()],
        };
        let mut matches = vec![Match::new(99, 1, 2)];

        let err = engine
            .scan_into(&backend, b"a", 1, &mut matches)
            .expect_err("short literal match-count readback must fail");

        let msg = err.to_string();
        assert!(
            matches.is_empty(),
            "scan errors must not expose stale matches"
        );
        assert!(
            msg.contains("literal_set match count") && msg.contains("requires 4 bytes"),
            "literal scan counter error must name the malformed output: {msg}"
        );
    }

    #[test]
    fn literal_scan_rejects_missing_match_output_slot() {
        let engine = GpuLiteralSet::compile(&[b"a".as_slice()]);
        let backend = LiteralReadbackBackend {
            outputs: vec![match_count_bytes(1)],
        };
        let mut matches = Vec::new();

        let err = engine
            .scan_into(&backend, b"a", 1, &mut matches)
            .expect_err("missing literal match output must fail");

        let msg = err.to_string();
        assert!(
            msg.contains("literal_set matches") && msg.contains("output index 1"),
            "literal scan missing-output error must identify the omitted slot: {msg}"
        );
    }

    #[test]
    fn literal_scan_rejects_match_payload_shorter_than_reported_count() {
        let engine = GpuLiteralSet::compile(&[b"a".as_slice()]);
        let backend = LiteralReadbackBackend {
            outputs: vec![match_count_bytes(2), match_triple_bytes(0, 0, 1)],
        };
        let mut matches = vec![Match::new(99, 1, 2)];

        let err = engine
            .scan_into(&backend, b"a", 2, &mut matches)
            .expect_err("short literal match payload must fail");

        let msg = err.to_string();
        assert!(
            matches.is_empty(),
            "scan errors must not expose stale matches"
        );
        assert!(
            msg.contains("readback was 12 byte(s)")
                && msg.contains("count=2")
                && msg.contains("requires 24 byte(s)"),
            "literal scan match-payload error must identify observed and required bytes: {msg}"
        );
    }

    #[test]
    fn literal_scan_exposes_scratch_backed_dispatch_staging() {
        let production = include_str!("literal_set.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: literal_set.rs must contain production section");

        assert!(
            production.contains("pub fn scan_into_with_scratch")
                && production.contains("ScanDispatchScratch")
                && production.contains("LiteralSetScanScratch")
                && production.contains("pack_haystack_u32_into")
                && !production.contains(concat!("pack_haystack_u32", "(haystack)")),
            "Fix: literal scan hot path must expose reusable dispatch scratch and avoid fresh haystack packing allocations."
        );
        // No LAZY panics (no fix hint); explicit panic!() fail-loud is allowed.
        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: literal_set production wrappers must not use bare .unwrap()/.expect() — use an explicit panic!() with a fix hint."
        );
        // Law 10 regression guard: GpuLiteralSet::compile must not swallow a
        // compile error into an empty matcher (which silently matches nothing,
        // reporting every input as clean). The old arm used
        // `eprintln! + empty_after_compile_failure()`; assert it is gone and a
        // fail-loud panic!() is present.
        assert!(
            !production.contains("eprintln!(\"vyre-libs GpuLiteralSet::compile failed")
                && !production.contains("empty_after_compile_failure"),
            "Fix: GpuLiteralSet::compile must not log-and-return an empty matcher on error — fail loud via panic!() so callers use try_compile."
        );
        assert!(
            production.contains("panic!("),
            "Fix: GpuLiteralSet::compile must panic!() on an unrepresentable pattern set, never fabricate an empty matcher."
        );
        let program_debug = format!("{:#?}", GpuLiteralSet::compile(&[b"a".as_slice()]).program);
        assert!(
            !program_debug.contains("_vyre_match_leader"),
            "Fix: literal-set GPU program must use the CUDA-lowerable append primitive, not subgroup leader append."
        );
        let engine = GpuLiteralSet::compile(&[b"a".as_slice(), b"bc".as_slice()]);
        let buffer_names = engine
            .program
            .buffers()
            .iter()
            .map(|buffer| buffer.name())
            .collect::<Vec<_>>();
        assert_eq!(
            buffer_names,
            vec![
                "haystack",
                "transitions",
                "output_offsets",
                "output_records",
                "pattern_lengths",
                "haystack_len",
                "match_count",
                "candidate_end_mask",
                "candidate_suffix2_mask",
                "candidate_suffix3_bloom",
                "matches"
            ],
            "Fix: public literal-set dispatch must run on the suffix-prefiltered bounded DFA table layout, not the old literal-byte compare ABI."
        );
        assert!(
            !program_debug.contains("pattern_bytes")
                && !program_debug.contains("pattern_offsets")
                && !program_debug.contains("_pid")
                && !program_debug.contains("_literal_matched"),
            "Fix: literal-set GPU program must not retain the per-pattern literal compare loop."
        );
    }

    #[test]
    fn literal_scan_sizes_match_output_to_requested_cap() {
        let engine = GpuLiteralSet::compile(&[b"a".as_slice()]);
        let mut payload = match_triple_bytes(0, 0, 1);
        payload.extend_from_slice(&match_triple_bytes(0, 3, 4));
        let backend = RecordingLiteralBackend::new(vec![match_count_bytes(2), payload]);
        let mut matches = Vec::new();

        engine
            .scan_into(&backend, b"a--a", 2, &mut matches)
            .expect("Fix: literal scan with two-match cap should dispatch");

        assert_eq!(matches, vec![Match::new(0, 0, 1), Match::new(0, 3, 4)]);
        assert_eq!(backend.observed_matches_layouts(), vec![(6, Some(0..24))]);
    }

    #[test]
    fn literal_scan_uploads_dfa_tables_instead_of_literal_compare_tables() {
        let engine = GpuLiteralSet::compile(&[
            b"AKIA".as_slice(),
            b"ghp_".as_slice(),
            b"Authorization: Bearer ".as_slice(),
        ]);
        let backend = RecordingLiteralBackend::new(vec![match_count_bytes(0), Vec::new()]);
        let mut matches = Vec::new();

        engine
            .scan_into(
                &backend,
                b"prefix Authorization: Bearer token",
                4,
                &mut matches,
            )
            .expect("Fix: literal scan should dispatch with DFA table inputs");

        assert!(matches.is_empty());
        let packed_haystack_len =
            crate::scan::dispatch_io::pack_haystack_u32(b"prefix Authorization: Bearer token")
                .len();
        let prefilter = engine
            .build_prefilter_tables()
            .expect("Fix: small literal-set prefilter tables should build");
        assert_eq!(
            backend.observed_input_lengths(),
            vec![vec![
                packed_haystack_len,
                engine.dfa.transitions.len() * U32_BYTES,
                engine.dfa.output_offsets.len() * U32_BYTES,
                engine.dfa.output_records.len() * U32_BYTES,
                engine.pattern_lengths.len() * U32_BYTES,
                U32_BYTES,
                U32_BYTES,
                prefilter.candidate_end_mask.len() * U32_BYTES,
                prefilter.candidate_suffix2_mask.len() * U32_BYTES,
                prefilter.candidate_suffix3_bloom.len() * U32_BYTES,
            ]],
            "Fix: public literal-set scan must upload haystack, DFA tables, suffix-prefilter masks, haystack_len, and match_count."
        );
    }

    #[test]
    fn literal_set_dfa_program_reference_eval_matches_public_oracle() {
        let patterns: [&[u8]; 5] = [b"a", b"bc", b"abcd", b"BEGIN", b"token"];
        let haystack = b"zabcd BEGIN token abcdbc";
        let engine = GpuLiteralSet::compile(&patterns);
        let prefilter = engine
            .build_prefilter_tables()
            .expect("Fix: small literal-set prefilter tables should build");
        let inputs = vec![
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_haystack_u32(
                haystack,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &engine.dfa.transitions,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &engine.dfa.output_offsets,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &engine.dfa.output_records,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &engine.pattern_lengths,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(&[
                haystack.len() as u32,
            ])),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(&[0])),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &prefilter.candidate_end_mask,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &prefilter.candidate_suffix2_mask,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &prefilter.candidate_suffix3_bloom,
            )),
        ];
        let outputs = vyre_reference::reference_eval(&engine.program, &inputs).expect(
            "Fix: public literal-set suffix-prefiltered bounded-DFA program should evaluate in reference backend.",
        );
        let mut actual = decode_reference_matches(&outputs);
        let mut expected = engine.reference_scan(haystack);
        actual.sort_unstable();
        expected.sort_unstable();

        assert_eq!(actual, expected);
    }

    /// CPU reference oracle for the FUSED region-presence + match-positions program.
    /// One `reference_eval` of the fused suffix3 program must produce BOTH outputs
    /// recall-identically to running the two scans separately:
    ///   - the match triples must equal `reference_scan` (the linear AC oracle), and
    ///   - each region's presence bits must equal the set of pattern ids whose match
    ///     end falls in that region.
    /// This is the soundness contract a downstream GPU phase-1 fold depends on: collapsing
    /// `scan_presence_by_region` + a separate position scan into one walk must change
    /// neither output. CPU-only (no GPU) so it runs in the lib gate.
    #[test]
    fn fused_presence_and_positions_by_region_reference_eval_matches_both_oracles() {
        // patterns -> ids 0=abc, 1=xyz, 2=BEGIN (compile preserves input order).
        let patterns: [&[u8]; 3] = [b"abc", b"xyz", b"BEGIN"];
        // Two coalesced regions; no match spans the boundary.
        //   region 0 = [0, 7)  "ooabcoo"     -> holds "abc"
        //   region 1 = [7, 21) "ooxyzooBEGINoo" -> holds "xyz" and "BEGIN"
        let haystack = b"ooabcooooxyzooBEGINoo";
        let region_starts: [u32; 2] = [0, 7];
        let pattern_count: u32 = 3;
        let region_count: u32 = 2;
        let max_matches: u32 = 64;

        let engine = GpuLiteralSet::compile(&patterns);
        let prefilter = engine
            .build_prefilter_tables()
            .expect("Fix: small literal-set prefilter tables should build");
        let total_presence_words =
            presence_by_region_words(pattern_count, region_count) as usize;

        // Inputs in binding order (read-write buffers passed zeroed; the matches
        // output at binding 13 is backend-allocated and not passed).
        let inputs = vec![
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_haystack_u32(
                haystack,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &engine.dfa.transitions,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &engine.dfa.output_offsets,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &engine.dfa.output_records,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &engine.pattern_lengths,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(&[
                haystack.len() as u32,
            ])),
            // 6: per-region presence, zeroed.
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &vec![0u32; total_presence_words],
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &prefilter.candidate_end_mask,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &prefilter.candidate_suffix2_mask,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &prefilter.candidate_suffix3_bloom,
            )),
            // 10: region_starts, 11: region_base (0), 12: match_count (zeroed).
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(
                &region_starts,
            )),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(&[0u32])),
            vyre_reference::value::Value::from(crate::scan::dispatch_io::pack_u32_slice(&[0u32])),
        ];

        let program = try_build_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program(
            &engine.dfa,
            pattern_count,
            region_count,
            max_matches,
        )
        .expect("Fix: fused presence+positions program should build for a 3-pattern set");
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: fused presence+positions program should evaluate in the reference backend");

        // outputs[0] = presence (read_write binding 6), [1] = match_count (12),
        // [2] = matches (13).
        let presence = decode_u32_words(&outputs[0].to_bytes());
        let count = decode_u32_words(&outputs[1].to_bytes())[0] as usize;
        let mut actual_matches: Vec<Match> = decode_u32_words(&outputs[2].to_bytes())
            .into_iter()
            .take(count.saturating_mul(3))
            .collect::<Vec<_>>()
            .chunks_exact(3)
            .map(|chunk| Match::new(chunk[0], chunk[1], chunk[2]))
            .collect();

        // (1) Positions: the fused triples must equal the linear AC oracle exactly.
        let mut expected_matches = engine.reference_scan(haystack);
        actual_matches.sort_unstable();
        expected_matches.sort_unstable();
        assert_eq!(
            actual_matches, expected_matches,
            "fused match triples must equal reference_scan; got count={count}"
        );
        assert_eq!(count, 3, "exactly abc + xyz + BEGIN should match");

        // (2) Presence: derive the expected per-region bitmap from the SAME oracle
        // matches (region = largest r with region_starts[r] <= end-1; pid<32 so one
        // word per region) and require the fused presence to equal it bit-for-bit.
        assert_eq!(
            presence.len(),
            total_presence_words,
            "fused presence must be region_count x presence_words"
        );
        let presence_words = presence_bitmap_words(pattern_count) as usize;
        let mut expected_presence = vec![0u32; total_presence_words];
        for m in &expected_matches {
            let pos = m.end - 1;
            let region = region_starts
                .iter()
                .rposition(|&start| start <= pos)
                .expect("every match position lands in a region (region_starts[0]==0)");
            expected_presence[region * presence_words + (m.pattern_id >> 5) as usize] |=
                1u32 << (m.pattern_id & 31);
        }
        assert_eq!(
            presence, expected_presence,
            "fused per-region presence must equal the region-mapped firing set"
        );
        // Concrete cross-check of the derived expectation: region 0 = {abc(id0)},
        // region 1 = {xyz(id1), BEGIN(id2)}.
        assert_eq!(presence[0], 1 << 0, "region 0 presence must be exactly {{abc}}");
        assert_eq!(
            presence[1],
            (1 << 1) | (1 << 2),
            "region 1 presence must be exactly {{xyz, BEGIN}}"
        );
    }

    #[test]
    fn scan_presence_by_region_binds_dfa_and_prefilter_views_in_declared_order() {
        // Regression guard for the shared `DfaPrefilterByteViews` byte-prep: the four
        // borrowed-dispatch scan methods (`scan_presence`, this one,
        // `scan_presence_and_positions_by_region`, `scan_into_with_program`) now fill
        // their `borrowed_inputs` array by referencing the struct's fields BY NAME, so
        // a field/binding swap would silently miswire the GPU program with no compile
        // error. This is the only DEFAULT-GATE test that drives a `scan_presence*`
        // method's binding assembly end to end (the live-GPU integration tests cover
        // the megakernel, not `GpuLiteralSet`). The three patterns are chosen so the
        // four DFA tables have mutually DISTINCT byte lengths, which is what lets the
        // observed per-binding length vector detect a swap among bindings 1..=4.
        // `"bc"` is a suffix of `"abc"`, so the `abc` accepting state emits TWO pattern
        // ids — that pushes `output_records` (binding 3) past the pattern count, so it
        // differs in length from `pattern_lengths` (binding 4) and a 3<->4 swap is
        // observable (with no suffix sharing both would be `pattern_count` long).
        let patterns: [&[u8]; 3] = [b"abc", b"bc", b"xyz"];
        let engine = GpuLiteralSet::compile(&patterns);
        let prefilter = engine
            .build_prefilter_tables()
            .expect("Fix: small literal-set prefilter tables should build");

        let haystack = b"ooabcooxyzoo";
        let region_starts: [u32; 1] = [0];
        let pattern_count = patterns.len() as u32;
        let region_count = region_starts.len() as u32;
        let total_words = presence_by_region_words(pattern_count, region_count) as usize;

        // The DFA tables must be mutually distinct in length, or the length-vector
        // assertion below could not catch a swap among them.
        let dfa_lens = [
            engine.dfa.transitions.len(),
            engine.dfa.output_offsets.len(),
            engine.dfa.output_records.len(),
            engine.pattern_lengths.len(),
        ];
        for i in 0..dfa_lens.len() {
            for j in (i + 1)..dfa_lens.len() {
                assert_ne!(
                    dfa_lens[i], dfa_lens[j],
                    "Fix: test DFA tables (transitions/output_offsets/output_records/pattern_lengths) \
                     must have distinct lengths so a binding swap is observable"
                );
            }
        }

        // `RecordingCountBackend` echoes outputs[0] (the presence buffer) and records
        // every borrowed input's byte length in binding order, with no required output
        // buffer — `RecordingLiteralBackend` insists on a `matches` buffer the
        // presence-only program does not declare.
        let backend = RecordingCountBackend::new(vec![vec![0u8; total_words * 4]]);
        let presence = engine
            .scan_presence_by_region(&backend, haystack, &region_starts)
            .expect("Fix: recording backend should accept the region-presence dispatch");
        assert_eq!(
            presence.len(),
            total_words,
            "Fix: region-presence output must be region_count x presence_words"
        );

        // Expected per-binding byte layout in declared ABI order (see
        // `scan_presence_by_region_with_scratch`): 0 haystack, 1 transitions,
        // 2 output_offsets, 3 output_records, 4 pattern_lengths, 5 haystack_len,
        // 6 presence (zeroed), 7..=9 prefilter masks, 10 region_starts, 11 region_base.
        let expected = vec![
            crate::scan::dispatch_io::pack_haystack_u32(haystack).len(),
            engine.dfa.transitions.len() * 4,
            engine.dfa.output_offsets.len() * 4,
            engine.dfa.output_records.len() * 4,
            engine.pattern_lengths.len() * 4,
            4, // haystack_len: one u32 word
            total_words * 4, // per-region presence buffer (uploaded zeroed)
            prefilter.candidate_end_mask.len() * 4,
            prefilter.candidate_suffix2_mask.len() * 4,
            prefilter.candidate_suffix3_bloom.len() * 4,
            region_starts.len() * 4,
            4, // region_base: one u32 word
        ];
        assert_eq!(
            backend.observed_input_lengths()[0],
            expected,
            "Fix: scan_presence_by_region must bind DfaPrefilterByteViews fields in declared ABI order"
        );
    }

    #[test]
    fn literal_scan_default_cap_uses_compiled_output_layout() {
        let engine = GpuLiteralSet::compile(&[b"a".as_slice()]);
        let backend = RecordingLiteralBackend::new(vec![match_count_bytes(0), Vec::new()]);
        let mut matches = Vec::new();

        engine
            .scan_into(
                &backend,
                b"no hits",
                LITERAL_SET_DEFAULT_MAX_MATCHES,
                &mut matches,
            )
            .expect("Fix: default literal scan cap should use the compiled program layout");

        assert!(matches.is_empty());
        assert_eq!(backend.observed_matches_layouts(), vec![(30_000, None)]);
    }

    #[test]
    fn literal_scan_zero_cap_fails_closed_when_a_match_is_found() {
        // Law 10 boundary: a zero-capacity position buffer that the kernel still
        // counts a match into must FAIL CLOSED, not silently return empty.
        // `scan_into` exposes only `matches`, so returning empty here would hide a
        // real match with no signal — exactly the silent false negative the capped
        // decode forbids. A caller wanting presence/count without positions uses
        // `scan_presence_by_region`; a 0-cap positions scan that finds a match has
        // dropped it, so surface that.
        let engine = GpuLiteralSet::compile(&[b"a".as_slice()]);
        let backend = RecordingLiteralBackend::new(vec![match_count_bytes(1), Vec::new()]);
        let mut matches = vec![Match::new(99, 1, 2)];

        let err = engine
            .scan_into(&backend, b"a", 0, &mut matches)
            .expect_err("zero cap with a counted match must error, not silently drop it");
        let msg = err.to_string();
        assert!(
            msg.contains("exceeds the output-buffer cap 0")
                && msg.contains("drop 1 match(es)")
                && matches.is_empty(),
            "zero-cap overflow must name the drop and expose no partial matches: {msg}"
        );
        // The dispatch still happened: the readback observed the true count and a
        // zero-length match payload before the decode failed closed.
        assert_eq!(backend.observed_matches_layouts(), vec![(1, Some(0..0))]);
    }

    #[test]
    fn literal_scan_zero_cap_with_no_matches_is_empty_ok() {
        // The benign twin: zero cap AND zero matches is not an overflow (count 0
        // is not > cap 0), so it returns empty without error.
        let engine = GpuLiteralSet::compile(&[b"a".as_slice()]);
        let backend = RecordingLiteralBackend::new(vec![match_count_bytes(0), Vec::new()]);
        let mut matches = vec![Match::new(99, 1, 2)];

        engine
            .scan_into(&backend, b"zzz", 0, &mut matches)
            .expect("zero cap with zero matches must succeed with an empty result");
        assert!(matches.is_empty());
    }

    #[test]
    fn literal_scan_expands_match_output_above_legacy_fixed_cap() {
        let engine = GpuLiteralSet::compile(&[b"a".as_slice()]);
        let backend = RecordingLiteralBackend::new(vec![match_count_bytes(0), Vec::new()]);
        let mut matches = Vec::new();

        engine
            .scan_into(&backend, b"no hits", 20_001, &mut matches)
            .expect("Fix: literal scan should honor caps above the compiled default");

        assert!(matches.is_empty());
        assert_eq!(
            backend.observed_matches_layouts(),
            vec![(60_003, Some(0..240_012))]
        );
    }

    #[test]
    fn literal_scan_literal_scratch_reuses_rewritten_program_for_same_cap() {
        let engine = GpuLiteralSet::compile(&[b"a".as_slice()]);
        let backend = RecordingLiteralBackend::new(vec![match_count_bytes(0), Vec::new()]);
        let mut matches = Vec::new();
        let mut scratch = LiteralSetScanScratch::default();

        engine
            .scan_into_with_literal_scratch(&backend, b"first", 2, &mut matches, &mut scratch)
            .expect("Fix: first cap-specific literal scan should dispatch");
        engine
            .scan_into_with_literal_scratch(&backend, b"second", 2, &mut matches, &mut scratch)
            .expect("Fix: repeated cap-specific literal scan should dispatch");

        assert_eq!(
            backend.observed_matches_layouts(),
            vec![(6, Some(0..24)), (6, Some(0..24))]
        );
        let ptrs = backend.observed_program_buffer_ptrs();
        assert_eq!(ptrs.len(), 2);
        assert_eq!(
            ptrs[0], ptrs[1],
            "Fix: literal-set scan scratch must reuse the rewritten Program for stable caps"
        );
    }

    #[test]
    fn literal_scan_literal_scratch_rebuilds_rewritten_program_when_cap_changes() {
        let engine = GpuLiteralSet::compile(&[b"a".as_slice()]);
        let backend = RecordingLiteralBackend::new(vec![match_count_bytes(0), Vec::new()]);
        let mut matches = Vec::new();
        let mut scratch = LiteralSetScanScratch::default();

        engine
            .scan_into_with_literal_scratch(&backend, b"first", 2, &mut matches, &mut scratch)
            .expect("Fix: first cap-specific literal scan should dispatch");
        engine
            .scan_into_with_literal_scratch(&backend, b"second", 3, &mut matches, &mut scratch)
            .expect("Fix: changed cap-specific literal scan should dispatch");

        assert_eq!(
            backend.observed_matches_layouts(),
            vec![(6, Some(0..24)), (9, Some(0..36))]
        );
        let ptrs = backend.observed_program_buffer_ptrs();
        assert_eq!(ptrs.len(), 2);
        assert_ne!(
            ptrs[0], ptrs[1],
            "Fix: literal-set scan scratch must rebuild cached Program when cap changes"
        );
    }

    #[test]
    fn literal_scan_rejects_match_cap_that_overflows_output_words() {
        let engine = GpuLiteralSet::compile(&[b"a".as_slice()]);
        let backend = RecordingLiteralBackend::new(vec![match_count_bytes(0), Vec::new()]);
        let mut matches = Vec::new();

        let err = engine
            .scan_into(&backend, b"a", u32::MAX, &mut matches)
            .expect_err("Fix: overflowing literal max_matches must fail before dispatch");
        let msg = err.to_string();

        assert!(msg.contains("literal_set max_matches"));
        assert!(msg.contains("overflows the GPU match-output word count"));
        assert!(backend.observed_matches_layouts().is_empty());
    }
}

const LITERAL_SET_WIRE_MAGIC: &[u8; 4] = b"VLIT";
const LITERAL_SET_WIRE_VERSION: u32 = 3;
const LITERAL_SET_LEGACY_BOUNDED_DFA_WIRE_VERSION: u32 = 2;
const LITERAL_SET_LEGACY_LITERAL_COMPARE_WIRE_VERSION: u32 = 1;

/// Errors returned by [`GpuLiteralSet::from_bytes`]. Outer-framing
/// failures (truncation, bad magic, version drift) are forwarded
/// straight from the shared `WireFraming` envelope. Inner-section
/// failures (program decode, DFA decode) keep their own typed variants
/// so consumers can act on them. Variants are non-exhaustive so future
/// inner sections can be added without a breaking change.
#[derive(Debug)]
#[non_exhaustive]
pub enum LiteralSetWireError {
    /// Outer envelope (magic / version / section length) was rejected.
    /// Forwarded from `vyre_foundation::serial::envelope::EnvelopeError`.
    WireFraming(vyre_foundation::serial::envelope::EnvelopeError),
    /// The nested vyre IR `Program` blob was rejected. Inner message is
    /// stringified to keep this error type independent of vyre's own
    /// error enum.
    InvalidProgram(String),
    /// The nested `CompiledDfa` blob was rejected.
    InvalidDfa(DfaWireError),
}

impl std::fmt::Display for LiteralSetWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WireFraming(e) => write!(f, "GpuLiteralSet wire envelope: {e}"),
            Self::InvalidProgram(msg) => {
                write!(f, "GpuLiteralSet wire blob has invalid Program: {msg}")
            }
            Self::InvalidDfa(e) => {
                write!(f, "GpuLiteralSet wire blob has invalid DFA: {e}")
            }
        }
    }
}

impl std::error::Error for LiteralSetWireError {}

fn try_build_literal_set_program(dfa: &CompiledDfa, pattern_count: u32) -> Result<Program, String> {
    try_build_ac_bounded_ranges_suffix3_prefilter_program_ext(
        dfa,
        pattern_count,
        LITERAL_SET_DEFAULT_MAX_MATCHES,
        false,
    )
}

/// Innovation I.18: JIT DFA Lowering.
///
/// Converts a static transition table into a nested \`If\` cascade.
/// For small pattern sets, this eliminates the VRAM bandwidth bottleneck
/// by keeping the state machine in the GPU instruction cache.
pub fn dfa_to_jit_ir(dfa: &CompiledDfa, state_var: &str, byte_expr: Expr) -> Node {
    build_state_cascade(dfa, 0, state_var, byte_expr)
}

fn build_state_cascade(dfa: &CompiledDfa, state: u32, state_var: &str, byte_expr: Expr) -> Node {
    // Basic implementation: if state == S { if byte == B1 { state = T1 } ... }
    // V7-PERF-024: Binary-search tree emission for instructions.
    // Naive linear if/else is O(N); a binary tree is O(log N).

    let mut arms = Vec::new();
    for byte in 0..=255 {
        let next_state = dfa.transitions[(state as usize) * 256 + byte];
        if next_state != 0 {
            arms.push((byte as u32, next_state));
        }
    }

    if arms.is_empty() {
        return Node::Assign {
            name: state_var.into(),
            value: Expr::u32(0),
        };
    }

    // Build a nested If cascade for the transitions from this state
    let mut node = Node::Assign {
        name: state_var.into(),
        value: Expr::u32(0),
    };
    for (byte, next) in arms.into_iter().rev() {
        node = Node::If {
            cond: Expr::eq(byte_expr.clone(), Expr::u32(byte)),
            then: vec![Node::Assign {
                name: state_var.into(),
                value: Expr::u32(next),
            }],
            otherwise: vec![node],
        };
    }
    node
}
