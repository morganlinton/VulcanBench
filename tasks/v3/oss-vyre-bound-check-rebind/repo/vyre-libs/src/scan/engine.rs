//! Common abstractions over the matching engines in `vyre-libs`.
//!
//! Every concrete engine in this crate (`GpuLiteralSet`, `DirectGpuScanner`,
//! `RulePipeline`, future ones for parsers / taint flow / anomaly scoring)
//! ships the same shape of public API:
//!
//!   1. A `compile(...)` constructor that takes some pattern set.
//!   2. A `scan(&backend, &haystack, max_matches)` GPU dispatch.
//!   3. A `reference_scan(&haystack)` parity reference.
//!   4. A `to_bytes()` / `from_bytes(...)` cache pair.
//!
//! Until now each engine duplicated the trait shape ad-hoc. This module
//! is the lego-block fix: one set of traits, one generic
//! `cached_load_or_compile` helper, every engine plugs in.
//!
//! # Why two traits, not one
//!
//! - [`MatchScan`] is dyn-safe (no associated types, no `Sized`). Consumers
//!   can store `Box<dyn MatchScan>` to swap engines at runtime  -  scanner
//!   backend selection becomes a runtime
//!   trait-object swap instead of a hardcoded match arm.
//! - [`MatchEngineCache`] keeps typed errors (each engine's own
//!   `WireError` enum with its specific variants), so the cache layer's
//!   error messages stay actionable. Object-safety isn't needed here:
//!   cache wiring always knows the concrete type at compile time.
//!
//! Engines implement BOTH; consumers pick whichever fits their call site.
//!
//! # Cache wiring rule (Torvalds-style: do it once)
//!
//! [`cached_load_or_compile`] is the only blessed way to wire a cache.
//! Consumers should never re-implement the load/compile
//! /save dance. If a new engine needs special cache invalidation logic
//! (e.g. dropping the cache on certain ABI bumps), extend this helper  -
//! don't fork it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use vyre::VyreBackend;
use vyre_foundation::match_result::Match;
use vyre_primitives::hash::fnv1a::{fnv1a64_initial_state, fnv1a64_update_byte};

static CACHE_TMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const MAX_MATCH_ENGINE_CACHE_BYTES: u64 = 64 * 1024 * 1024;

/// Diagnostic-bearing wrapper around a scan result.
///
/// Every consumer pipeline ends up reconstructing these flags ad-hoc
/// (was the scan truncated? how long did it take? did we hit the
/// disk cache?). Centralising them gives downstream tooling
/// (telemetry pipelines, watch-mode dashboards, perf benches) a
/// single struct to read instead of parsing engine-specific output.
///
/// `ScanResult::matches` is the primary payload  -  consumers that
/// don't care about diagnostics can `result.matches` and ignore the
/// rest. The struct is `Clone` so it can be passed across thread
/// boundaries and `Default` so tests can fabricate empties.
#[derive(Debug, Clone, Default)]
pub struct ScanResult {
    /// Sorted matches produced by the engine.
    pub matches: Vec<Match>,
    /// True when the engine hit the per-dispatch `max_matches` cap
    /// AND the underlying scan reported overflow. Consumers should
    /// treat truncated results as incomplete and re-scan with a
    /// larger cap if every match matters (security audits).
    pub truncated: bool,
    /// Total wall-clock time the scan call spent, including dispatch
    /// + readback. `Duration::ZERO` when the engine doesn't measure.
    pub elapsed: Duration,
    /// True when the engine was loaded from disk cache instead of
    /// being recompiled. Used by perf tooling to attribute cold-
    /// start cost.
    pub cache_hit: bool,
}

impl ScanResult {
    /// Build a result from a bare match vector. Diagnostic flags
    /// default to safe values (not truncated, zero elapsed, no
    /// cache hit). For engines that produce richer diagnostics,
    /// construct the struct directly.
    #[must_use]
    pub fn from_matches(matches: Vec<Match>) -> Self {
        Self {
            matches,
            ..Self::default()
        }
    }

    /// Number of matches produced.
    #[must_use]
    pub fn len(&self) -> usize {
        self.matches.len()
    }

    /// True when the engine produced no matches.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }
}

/// GPU + Reference scan operations exposed by every matcher in this crate.
/// Object-safe (`dyn MatchScan` is valid) so consumers can hold a heap-
/// allocated trait object and swap engines at runtime.
pub trait MatchScan {
    /// GPU dispatch through a concrete backend, returning up to
    /// `max_matches` matches. Engines pre-allocate the hit buffer at
    /// `max_matches * 3 + 1` u32 slots; setting this too low silently
    /// truncates results.
    fn scan(
        &self,
        backend: &dyn VyreBackend,
        haystack: &[u8],
        max_matches: u32,
    ) -> Result<Vec<Match>, vyre::BackendError>;

    /// Reference oracle scan. Used by the cross-layer parity tests in
    /// `vyre-conform`; engines that lack a meaningful CPU stepper
    /// (none today) can return an empty vec but should never fabricate
    /// results.
    ///
    /// Engines whose CPU stepper can fail (e.g. a haystack exceeding the
    /// `u32` match ABI) must abort loudly here rather than returning an empty
    /// vec — an empty result is a silent recall lie (Law 10). Such engines
    /// override [`Self::try_reference_scan`] so `dyn MatchScan` consumers can
    /// recover the error instead of unwinding.
    fn reference_scan(&self, haystack: &[u8]) -> Vec<Match>;

    /// Fallible reference oracle scan. Surfaces a CPU-stepper failure (a
    /// haystack longer than the `u32` match ABI the GPU path uses) as an error
    /// instead of aborting, so consumers holding `dyn MatchScan` can recover.
    /// The default delegates to the infallible [`Self::reference_scan`] for
    /// engines whose stepper genuinely cannot fail; engines with a fallible
    /// stepper override this to forward to their real fallible scan (and never
    /// route through the panicking infallible wrapper).
    ///
    /// # Errors
    /// Engine-specific [`vyre::BackendError`] when the CPU oracle cannot honor
    /// the same `u32` match ABI the GPU path uses for this haystack.
    fn try_reference_scan(&self, haystack: &[u8]) -> Result<Vec<Match>, vyre::BackendError> {
        Ok(self.reference_scan(haystack))
    }

    /// Stable identity for cache filenames + telemetry. Engines hash
    /// their pattern set + version constant. Consumers pass this
    /// straight to [`cached_load_or_compile`] without further hashing.
    fn cache_key(&self) -> String;
}

/// Wire serialization for caching a compiled engine. Kept separate
/// from [`MatchScan`] because typed errors aren't dyn-safe.
pub trait MatchEngineCache: Sized {
    /// The engine's wire-error enum. Forwarded to the cache helper so
    /// load failures discriminate "stale cache, recompile" from "real
    /// bug, refuse to start".
    type WireError: std::fmt::Display + std::fmt::Debug;

    /// Wire-format magic the engine stamps on every encoded blob. The
    /// contracts test asserts that `to_bytes()[0..4] == WIRE_MAGIC`
    /// so consumers cannot accidentally forge a cache file with a
    /// different magic and have it silently load.
    const WIRE_MAGIC: [u8; 4];

    /// Wire-format version stamped after the magic. Bumped on any
    /// breaking layout change. The cache helper uses this to discard
    /// blobs from older builds; a `VersionMismatch` decode error is
    /// the canonical "stale cache, recompile" signal.
    const WIRE_VERSION: u32;

    /// Largest on-disk cache blob this engine will read back. A blob exceeding
    /// it is treated as oversized (dropped, then recompiled). Defaults to
    /// [`MAX_MATCH_ENGINE_CACHE_BYTES`]; engines whose compiled form is
    /// genuinely large (e.g. a batched-megakernel DFA catalog, ~GB) override it.
    const MAX_CACHE_BYTES: u64 = MAX_MATCH_ENGINE_CACHE_BYTES;

    /// Encode the compiled engine for on-disk caching.
    ///
    /// # Errors
    /// Engine-specific framing error.
    fn to_bytes(&self) -> Result<Vec<u8>, Self::WireError>;

    /// Decode a previously-cached engine.
    ///
    /// # Errors
    /// Engine-specific framing error. The cache helper treats every
    /// `WireError` as "stale, drop and recompile"  -  that's the
    /// designed-in semantics.
    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::WireError>;
}

/// Resolve the cache file path for `cache_key` under `cache_dir`.
/// Creates `cache_dir` (and any missing parents) on first use. Returns
/// `None` when the directory could not be created  -  consumers should
/// fall through to a non-cached compile in that case.
pub fn cache_path(cache_dir: &Path, cache_key: &str) -> Option<PathBuf> {
    if !cache_dir.exists() && std::fs::create_dir_all(cache_dir).is_err() {
        return None;
    }
    Some(cache_dir.join(format!("{cache_key}.bin")))
}

fn cache_tmp_path(path: &Path) -> PathBuf {
    let sequence = CACHE_TMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    path.with_extension(format!("tmp.{}.{}", std::process::id(), sequence))
}

fn read_match_engine_cache_bounded(
    path: &Path,
    max_bytes: u64,
) -> std::io::Result<Option<Vec<u8>>> {
    let mut reader = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    let mut total = 0u64;
    let mut chunk = [0u8; 8192];
    loop {
        let read = std::io::Read::read(&mut reader, &mut chunk)?;
        if read == 0 {
            return Ok(Some(bytes));
        }
        let read = read as u64;
        total = total.saturating_add(read);
        if total > max_bytes {
            return Ok(None);
        }
        bytes.extend_from_slice(&chunk[..read as usize]);
    }
}

fn remove_match_engine_cache(path: &Path, message: &'static str) {
    if let Err(error) = std::fs::remove_file(path) {
        tracing::debug!(
            path = %path.display(),
            error = %error,
            "{}",
            message
        );
    }
}

/// Generic load-or-compile-and-save for any [`MatchEngineCache`].
///
/// Replaces the per-engine cache wiring every downstream scanner
/// would otherwise duplicate. The contract:
///
///   - Cache hit: read the file, attempt `from_bytes`. On success
///     return the loaded engine. On framing error, delete the stale
///     blob and fall through.
///   - Cache miss / stale: call `compile`, `to_bytes`, atomically
///     write to a `.tmp.<pid>.<sequence>` sibling, rename onto the final path.
///   - Any save-side error is logged at `tracing::debug` and ignored  -
///     a failed cache write must never break the scan path.
///
/// `compile` is `FnOnce` so consumers can move expensive captures
/// (pattern sources, file readers) into it without cloning.
pub fn cached_load_or_compile<E, F>(cache_dir: &Path, cache_key: &str, compile: F) -> E
where
    E: MatchEngineCache,
    F: FnOnce() -> E,
{
    let Some(path) = cache_path(cache_dir, cache_key) else {
        return compile();
    };

    match read_match_engine_cache_bounded(&path, E::MAX_CACHE_BYTES) {
        Ok(Some(bytes)) => match E::from_bytes(&bytes) {
            Ok(engine) => return engine,
            Err(_) => remove_match_engine_cache(&path, "failed to remove corrupt matching cache"),
        },
        Ok(None) => {
            remove_match_engine_cache(&path, "failed to remove oversized matching cache");
        }
        Err(error) => {
            tracing::debug!(
                path = %path.display(),
                error = %error,
                "failed to read matching cache"
            );
        }
    }

    let engine = compile();
    if let Ok(bytes) = engine.to_bytes() {
        let tmp = cache_tmp_path(&path);
        match std::fs::write(&tmp, &bytes) {
            Ok(()) => {
                if let Err(error) = std::fs::rename(&tmp, &path) {
                    tracing::debug!(
                        path = %path.display(),
                        tmp = %tmp.display(),
                        error = %error,
                        "failed to publish matching cache"
                    );
                    if let Err(cleanup_error) = std::fs::remove_file(&tmp) {
                        tracing::debug!(
                            tmp = %tmp.display(),
                            error = %cleanup_error,
                            "failed to remove matching cache temp file"
                        );
                    }
                }
            }
            Err(error) => {
                tracing::debug!(
                    tmp = %tmp.display(),
                    error = %error,
                    "failed to write matching cache temp file"
                );
            }
        }
    }
    engine
}

// ---- Concrete impls for the engines this crate ships ----

use crate::scan::literal_set::{GpuLiteralSet, LiteralSetWireError};

impl MatchScan for GpuLiteralSet {
    fn scan(
        &self,
        backend: &dyn VyreBackend,
        haystack: &[u8],
        max_matches: u32,
    ) -> Result<Vec<Match>, vyre::BackendError> {
        GpuLiteralSet::scan(self, backend, haystack, max_matches)
    }

    fn reference_scan(&self, haystack: &[u8]) -> Vec<Match> {
        GpuLiteralSet::reference_scan(self, haystack)
    }

    fn cache_key(&self) -> String {
        // Use vyre's FNV-1a primitive instead of std::DefaultHasher.
        // DefaultHasher's SipHash seed is randomized per process, so
        // cache files written by one run would never match keys
        // generated by the next  -  silently breaking the cache. FNV-1a
        // is deterministic, fast, and we don't need cryptographic
        // collision resistance for an identity hash.
        let h = fnv1a64_word_slices([
            self.pattern_offsets.as_slice(),
            self.pattern_lengths.as_slice(),
            self.pattern_bytes.as_slice(),
        ]);
        format!("lit-{h:016x}")
    }
}

impl MatchEngineCache for GpuLiteralSet {
    type WireError = LiteralSetWireError;
    const WIRE_MAGIC: [u8; 4] = *b"VLIT";
    const WIRE_VERSION: u32 = 3;

    fn to_bytes(&self) -> Result<Vec<u8>, Self::WireError> {
        GpuLiteralSet::to_bytes(self)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::WireError> {
        GpuLiteralSet::from_bytes(bytes)
    }
}

#[cfg(feature = "matching-dfa")]
mod direct_gpu_impls {
    use super::*;
    use crate::scan::direct_gpu::DirectGpuScanner;

    impl MatchScan for DirectGpuScanner {
        fn scan(
            &self,
            backend: &dyn VyreBackend,
            haystack: &[u8],
            max_matches: u32,
        ) -> Result<Vec<Match>, vyre::BackendError> {
            DirectGpuScanner::scan(self, backend, haystack, max_matches)
        }

        fn reference_scan(&self, haystack: &[u8]) -> Vec<Match> {
            DirectGpuScanner::reference_scan(self, haystack)
        }

        fn cache_key(&self) -> String {
            // Direct scanner is a thin wrapper over a literal-set  -
            // delegate so caches don't fork.
            format!("direct-gpu-{}", self.literal_set_cache_key())
        }
    }
}

#[cfg(feature = "matching-nfa")]
mod rule_pipeline_impls {
    use super::*;
    use crate::scan::mega_scan::{PipelineWireError, RulePipeline};

    impl MatchScan for RulePipeline {
        fn scan(
            &self,
            backend: &dyn VyreBackend,
            haystack: &[u8],
            max_matches: u32,
        ) -> Result<Vec<Match>, vyre::BackendError> {
            RulePipeline::scan(self, backend, haystack, max_matches)
        }

        fn reference_scan(&self, haystack: &[u8]) -> Vec<Match> {
            RulePipeline::reference_scan(self, haystack)
        }

        fn try_reference_scan(
            &self,
            haystack: &[u8],
        ) -> Result<Vec<Match>, vyre::BackendError> {
            // Forward to the inherent fallible scan, NOT the default — the
            // default would call the panicking infallible `reference_scan`,
            // turning a recoverable >u32 haystack into an abort.
            RulePipeline::try_reference_scan(self, haystack)
        }

        fn cache_key(&self) -> String {
            // Deterministic hash via vyre's FNV-1a primitive  -  see the
            // `GpuLiteralSet::cache_key` implementation for why
            // `DefaultHasher` is the wrong choice here (per-process
            // SipHash seed defeats persistent caching).
            let header = [self.plan.num_states, self.plan.input_len];
            let h = fnv1a64_word_slices([
                header.as_slice(),
                self.transition_table.as_slice(),
                self.epsilon_table.as_slice(),
            ]);
            format!("pipe-{h:016x}")
        }
    }

    impl MatchEngineCache for RulePipeline {
        type WireError = PipelineWireError;
        const WIRE_MAGIC: [u8; 4] = *b"VRPL";
        // Tracks `mega_scan::PIPELINE_WIRE_VERSION` (V4 adds the
        // per-workgroup max-scan-bytes uniform buffer to the encoded
        // Program; V3 added the runtime-haystack-len buffer).
        const WIRE_VERSION: u32 = 4;

        fn to_bytes(&self) -> Result<Vec<u8>, Self::WireError> {
            RulePipeline::to_bytes(self)
        }

        fn from_bytes(bytes: &[u8]) -> Result<Self, Self::WireError> {
            RulePipeline::from_bytes(bytes)
        }
    }
}

fn fnv1a64_word_slices<const N: usize>(slices: [&[u32]; N]) -> u64 {
    let mut h = fnv1a64_initial_state();
    for words in slices {
        for &word in words {
            for byte in word.to_le_bytes() {
                h = fnv1a64_update_byte(h, byte);
            }
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::literal_set::GpuLiteralSet;

    #[test]
    fn cache_key_changes_when_patterns_change() {
        let a = GpuLiteralSet::compile(&[b"AKIA".as_slice(), b"ghp_".as_slice()]);
        let b = GpuLiteralSet::compile(&[b"AKIA".as_slice(), b"ghp__".as_slice()]);
        assert_ne!(MatchScan::cache_key(&a), MatchScan::cache_key(&b));
    }

    #[test]
    fn cache_key_stable_for_same_patterns() {
        let a = GpuLiteralSet::compile(&[b"AKIA".as_slice(), b"ghp_".as_slice()]);
        let b = GpuLiteralSet::compile(&[b"AKIA".as_slice(), b"ghp_".as_slice()]);
        assert_eq!(MatchScan::cache_key(&a), MatchScan::cache_key(&b));
    }

    #[test]
    fn streaming_word_hash_matches_allocated_little_endian_bytes() {
        let words_a = [0x0102_0304_u32, 0xAABB_CCDD];
        let words_b = [0x1122_3344_u32];
        let mut bytes = Vec::new();
        for &word in words_a.iter().chain(words_b.iter()) {
            bytes.extend_from_slice(&word.to_le_bytes());
        }

        assert_eq!(
            fnv1a64_word_slices([words_a.as_slice(), words_b.as_slice()]),
            vyre_primitives::hash::fnv1a::fnv1a64(&bytes)
        );
    }

    #[test]
    fn cached_helper_round_trips_via_disk() {
        let dir = tempfile::tempdir().unwrap();
        let key = "test-engine";
        let mut compiles = 0;
        let _engine: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
            compiles += 1;
            GpuLiteralSet::compile(&[b"AKIA".as_slice()])
        });
        assert_eq!(compiles, 1);

        // Second call hits the disk cache; the closure must NOT run.
        let mut second_compiles = 0;
        let _engine2: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
            second_compiles += 1;
            GpuLiteralSet::compile(&[b"AKIA".as_slice()])
        });
        assert_eq!(second_compiles, 0);
    }

    #[test]
    fn cache_tmp_paths_do_not_collide_within_process() {
        let dir = tempfile::tempdir().unwrap();
        let path = cache_path(dir.path(), "same-key").unwrap();
        let first = cache_tmp_path(&path);
        let second = cache_tmp_path(&path);

        assert_ne!(
            first, second,
            "Fix: concurrent cache writers in one process need distinct temp files."
        );
        assert_eq!(first.parent(), path.parent());
        assert_eq!(second.parent(), path.parent());
    }

    #[test]
    fn cached_helper_recompiles_on_corrupt_blob() {
        let dir = tempfile::tempdir().unwrap();
        let key = "test-corrupt";
        // Plant a corrupt blob.
        std::fs::write(dir.path().join(format!("{key}.bin")), b"not a real blob").unwrap();

        let mut compiles = 0;
        let _engine: GpuLiteralSet = cached_load_or_compile(dir.path(), key, || {
            compiles += 1;
            GpuLiteralSet::compile(&[b"AKIA".as_slice()])
        });
        assert_eq!(compiles, 1);
    }
}
