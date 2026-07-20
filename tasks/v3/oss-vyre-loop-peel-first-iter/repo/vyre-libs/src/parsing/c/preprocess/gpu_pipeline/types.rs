//! Public contracts for the GPU preprocessor pipeline.

use std::sync::Arc;

use super::{
    ConditionalEvent, HeaderReuseEvent, IncludeAccelerationEvent, IncludeByteCacheStats,
    IncludeEvent, MacroEvent, MacroExpansionEvent, TokenProvenanceEvent,
};

/// Include resolver used by the orchestration layer after GPU directive
/// extraction emits an include request.
pub trait IncludeLoader {
    /// Resolve and load `#include <path>` (system) or `#include "path"`
    /// (local). `is_next` is true for GNU `#include_next`, where search
    /// resumes after the include directory that supplied `from`. `from`
    /// is the canonical path of the file currently being preprocessed;
    /// the impl uses it as the search base for local includes.
    ///
    /// Returns `(canonical_path, file_bytes)`. Returns `Err` for missing
    /// includes and fatal I/O errors; production callers must not silently
    /// skip a requested C header.
    fn load(
        &self,
        path: &[u8],
        is_system: bool,
        is_next: bool,
        from: &std::path::Path,
    ) -> Result<Option<(std::path::PathBuf, Arc<[u8]>)>, String>;
}

/// Output of the driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessedSource {
    /// Concatenated active bytes  -  line-spliced, comment-stripped,
    /// conditional-masked, include-expanded. Macro expansion is
    /// deliberately NOT performed here (mirrors the v0.4
    /// `prepare_resident_translation_unit_source` contract).
    pub bytes: Vec<u8>,
    /// Macros accumulated during the walk (CLI macros + every
    /// `#define` in active branches). Downstream macro-expansion
    /// kernels consume this.
    pub macros: Vec<MacroDef>,
    /// Include graph events whose requests were extracted by GPU directive
    /// payload kernels. Resolution/read remains host filesystem metadata.
    pub include_events: Vec<IncludeEvent>,
    /// Per-run include byte-cache counters for loader avoidance evidence.
    pub include_byte_cache_stats: IncludeByteCacheStats,
    /// Conditional stack events whose directive payloads are GPU-derived.
    pub conditional_events: Vec<ConditionalEvent>,
    /// Macro definition-table events whose directive payloads are GPU-derived.
    pub macro_events: Vec<MacroEvent>,
    /// Macro expansion origin events.
    pub macro_expansion_events: Vec<MacroExpansionEvent>,
    /// Token-level spelling and expansion provenance for the preprocessed output.
    pub token_provenance_events: Vec<TokenProvenanceEvent>,
    /// Include guard / pragma-once acceleration evidence.
    pub include_acceleration_events: Vec<IncludeAccelerationEvent>,
    /// Header-analysis cache reuse evidence.
    pub header_reuse_events: Vec<HeaderReuseEvent>,
}

/// A `#define`'d macro encountered during preprocessing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDef {
    /// Macro identifier bytes.
    pub name: Vec<u8>,
    /// Comma-separated argument-list bytes for function-like macros;
    /// empty for object-like.
    pub args: Vec<u8>,
    /// Replacement body bytes.
    pub body: Vec<u8>,
    /// `true` for function-like (`#define M(a) …`).
    pub is_function_like: bool,
}

/// Maximum recursive `#include` depth before the driver bails out.
/// Matches the resident frontend include-depth contract.
pub const MAX_INCLUDE_DEPTH: u32 = 64;
