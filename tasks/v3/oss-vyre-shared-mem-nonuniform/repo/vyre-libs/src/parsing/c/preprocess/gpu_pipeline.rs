//! GPU-resident preprocessor pipeline orchestration.
//!
//! Replaces the CPU helpers in `vyre-frontend-c::tu_host` with a chain
//! of GPU dispatches. Host-side responsibilities are limited to:
//!
//! - File I/O initiation (`fs::read`)  -  the kernel-mode VFS work that
//!   has no GPU equivalent.
//! - Recursive include scheduling  -  graph-traversal bookkeeping over
//!   file paths after GPU directive classification emits the include
//!   requests.
//! - Macro / conditional-frame bookkeeping between dispatches. The
//!   directive parsing, conditional evaluation, and replacement-token
//!   work stay in GPU kernels; host state only carries compact metadata
//!   from one dispatch frontier to the next.
//!
//! All actual byte-level / token-level / expression-level computation
//! runs on GPU via the kernels in
//! `vyre_libs::parsing::c::preprocess::*`.
//!
//! ## Phase split (this module ships in chunks)
//!
//! - **18a (this commit):** `gpu_filter_source_bytes`  -  runs
//!   `line_splice_classify` + `comment_strip_mask` + element-wise AND
//!   + prefix-scan + scatter-compact to produce the post-phase-2,
//!   comment-free byte stream that the lexer consumes. Foundational
//!   brick that every later stage builds on.
//! - **18b:** Lex + directive-classify + ifdef/if value evaluation
//!   batch.
//! - **18c:** `#define` / `#include` row parsing + macro-table
//!   maintenance.
//! - **18d:** Recursive include graph driver + macro expansion.
//! - **18e:** production callers route through this GPU pipeline; host
//!   preprocessor code remains only as explicit reference/test infrastructure.

#[path = "gpu_pipeline_filter.rs"]
mod gpu_pipeline_filter;
pub use gpu_pipeline_filter::{gpu_filter_source_bytes, FilteredBytes};
#[path = "gpu_pipeline/buffers.rs"]
mod buffers;
#[path = "gpu_pipeline/byte_lru_cache.rs"]
mod byte_lru_cache;
#[path = "gpu_pipeline/cache.rs"]
mod cache;
#[cfg(test)]
#[path = "gpu_pipeline/cache_tests.rs"]
mod cache_tests;
#[path = "gpu_pipeline/classified_size.rs"]
mod classified_size;
#[cfg(test)]
#[path = "gpu_pipeline/conditional_eval.rs"]
mod conditional_eval;
#[path = "gpu_pipeline/conditional_events.rs"]
mod conditional_events;
#[path = "gpu_pipeline/conditional_stack.rs"]
mod conditional_stack;
#[path = "gpu_pipeline/directives.rs"]
mod directives;
#[path = "gpu_pipeline/dispatch.rs"]
mod dispatch;
#[path = "gpu_pipeline/driver.rs"]
mod driver;
#[path = "gpu_pipeline/expansion_events.rs"]
mod expansion_events;
#[path = "gpu_pipeline/header_reuse.rs"]
mod header_reuse;
#[path = "gpu_pipeline/include_acceleration.rs"]
mod include_acceleration;
#[path = "gpu_pipeline/include_events.rs"]
mod include_events;
#[path = "gpu_pipeline/live_conditional_cache.rs"]
mod live_conditional_cache;
#[path = "gpu_pipeline/live_state.rs"]
mod live_state;
#[path = "gpu_pipeline/lru_index.rs"]
mod lru_index;
#[path = "gpu_pipeline/macro_events.rs"]
mod macro_events;
#[path = "gpu_pipeline/macro_expansion.rs"]
mod macro_expansion;
#[path = "gpu_pipeline/macro_table.rs"]
mod macro_table;
#[path = "gpu_pipeline/macro_values.rs"]
mod macro_values;
#[path = "gpu_pipeline/payload_size.rs"]
mod payload_size;
#[path = "gpu_pipeline/scan.rs"]
mod scan;
#[path = "gpu_pipeline/segments.rs"]
mod segments;
#[path = "gpu_pipeline/source_spans.rs"]
mod source_spans;
#[path = "gpu_pipeline/token_provenance.rs"]
mod token_provenance;
#[path = "gpu_pipeline/tokenization.rs"]
mod tokenization;
#[path = "gpu_pipeline/types.rs"]
mod types;
pub use buffers::bucket_pow2;
pub use conditional_events::{ConditionalEvent, ConditionalEventKind, ConditionalEventResidency};
pub use directives::{gpu_extract_directive_payloads, DirectivePayload};
pub use dispatch::{BackendDispatcher, GpuDispatcher};
pub use expansion_events::MacroExpansionEvent;
pub use header_reuse::{HeaderReuseEvent, HeaderReuseKey};
pub use include_acceleration::{IncludeAccelerationEvent, IncludeAccelerationKind};
pub use include_events::{IncludeByteCacheStats, IncludeEvent, IncludeEventResidency};
pub use macro_events::{MacroEvent, MacroEventKind};
pub use token_provenance::TokenProvenanceEvent;
pub use tokenization::{gpu_tokenize_and_classify, ClassifiedTokens};
pub use types::{IncludeLoader, MacroDef, PreprocessedSource, MAX_INCLUDE_DEPTH};

/// Drive the GPU preprocessor over a translation unit and recursively expand
/// active includes through `loader`.
pub fn gpu_preprocess_translation_unit(
    dispatcher: &dyn GpuDispatcher,
    loader: &dyn IncludeLoader,
    tu_path: &std::path::Path,
    source: &[u8],
    cli_macros: &[MacroDef],
) -> Result<PreprocessedSource, String> {
    driver::preprocess_translation_unit(dispatcher, loader, tu_path, source, cli_macros)
}
