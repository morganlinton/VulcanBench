//! Generic engine plus post-process pipeline.
//!
//! Pairs any [`MatchScan`] engine with the canonical reference
//! [`post_process::try_reference_post_process`] output.
//!
//! The pipeline is intentionally `Pipeline<E>` rather than a trait  -
//! every consumer wants the *same* post-processing semantics, so
//! parameterising over the engine while pinning the post-processor
//! preserves byte-for-byte cross-consumer equivalence.

use crate::scan::engine::MatchScan;
#[cfg(any(test, feature = "cpu-parity"))]
use crate::scan::post_process::try_reference_post_process;
use crate::scan::post_process::{PostProcessError, PostProcessedMatch};
use vyre::{BackendError, VyreBackend};
use vyre_foundation::match_result::Match;

/// Function pointer for the post-processing stage. Stored as an `fn`
/// (not `Box<dyn Fn>`) to keep `Pipeline<E>` `Copy + Sync` and avoid
/// indirection in the hot path.
pub type PostProcessFn = fn(&[Match], &[u8]) -> Result<Vec<PostProcessedMatch>, PostProcessError>;

/// Engine plus post-processor pair. Construct via [`Pipeline::new`] for the
/// default Reference oracle post-process, or [`Pipeline::with_post_process`] to
/// inject a custom one.
pub struct Pipeline<E> {
    /// Underlying scan engine. Anything that implements `MatchScan`
    /// composes  -  `GpuLiteralSet`, `RulePipeline`, future custom
    /// scanners.
    pub engine: E,
    /// Post-processing function. Defaults to `try_reference_post_process`.
    pub post_process: PostProcessFn,
}

impl<E: MatchScan> Pipeline<E> {
    /// Wrap an engine with the default reference post-processor.
    #[must_use]
    #[cfg(any(test, feature = "cpu-parity"))]
    pub const fn new(engine: E) -> Self {
        Self {
            engine,
            post_process: try_reference_post_process,
        }
    }

    /// Wrap an engine with a caller-supplied post-processor. Use when
    /// downstream consumers need different scoring, such as taint-flow
    /// reduction or benchmark passthrough.
    #[must_use]
    pub const fn with_post_process(engine: E, post_process: PostProcessFn) -> Self {
        Self {
            engine,
            post_process,
        }
    }

    /// Reference oracle one-shot: scan + post-process.
    #[must_use]
    #[cfg(any(test, feature = "cpu-parity"))]
    pub fn reference_scan_processed(&self, haystack: &[u8]) -> Vec<PostProcessedMatch> {
        let raw = self.engine.reference_scan(haystack);
        (self.post_process)(&raw, haystack).unwrap_or_else(|error| {
            panic!("vyre-libs scan Reference oracle post-process contract failed: {error}")
        })
    }

    /// Reference oracle one-shot that surfaces BOTH scan and post-processing
    /// errors.
    ///
    /// Unlike [`Self::reference_scan_processed`], a CPU-stepper failure (a
    /// haystack exceeding the `u32` match ABI) propagates as a [`BackendError`]
    /// instead of aborting — and instead of being swallowed into an empty
    /// match set, which would be a silent recall lie (Law 10). This mirrors the
    /// [`Self::scan_processed`] GPU contract: every full-pipeline entry point
    /// reports failure through the same error type so a consumer recovers
    /// uniformly.
    ///
    /// # Errors
    ///
    /// Returns a [`BackendError`] when the reference scan cannot honor the
    /// `u32` match ABI, or when a scan engine reports a match range outside the
    /// scanned haystack (a post-processing contract violation).
    #[cfg(any(test, feature = "cpu-parity"))]
    pub fn try_reference_scan_processed(
        &self,
        haystack: &[u8],
    ) -> Result<Vec<PostProcessedMatch>, BackendError> {
        let raw = self.engine.try_reference_scan(haystack)?;
        (self.post_process)(&raw, haystack).map_err(|error| BackendError::new(error.to_string()))
    }

    /// Full GPU dispatch + post-process. Mirrors `MatchScan::scan` and
    /// then runs the post-processor on the host before returning.
    ///
    /// # Errors
    /// Returns the `MatchScan::scan` error verbatim, or wraps a
    /// post-processing contract violation in [`BackendError`].
    pub fn scan_processed(
        &self,
        backend: &dyn VyreBackend,
        haystack: &[u8],
        max_matches: u32,
    ) -> Result<Vec<PostProcessedMatch>, BackendError> {
        let raw = self.engine.scan(backend, haystack, max_matches)?;
        (self.post_process)(&raw, haystack).map_err(|error| BackendError::new(error.to_string()))
    }
}

impl<E: Clone> Clone for Pipeline<E> {
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
            post_process: self.post_process,
        }
    }
}

impl<E: std::fmt::Debug> std::fmt::Debug for Pipeline<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pipeline")
            .field("engine", &self.engine)
            .field("post_process", &"fn(_, _) -> Vec<_>")
            .finish()
    }
}
