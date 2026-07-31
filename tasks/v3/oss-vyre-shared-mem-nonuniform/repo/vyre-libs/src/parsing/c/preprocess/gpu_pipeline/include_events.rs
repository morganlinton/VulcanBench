//! Include-graph evidence emitted by the GPU preprocessor driver.

/// Residency class for include preprocessing evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncludeEventResidency {
    /// Include request was extracted by GPU directive payload kernels.
    GpuResidentRequest,
    /// Include resolution/read is unavoidable host filesystem metadata work.
    HostFilesystemMetadata,
    /// Include bytes were reused from this translation-unit run's header cache.
    HostMemoryCache,
}

/// Per-translation-unit include byte-cache counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IncludeByteCacheStats {
    /// Include byte lookups satisfied from this run's in-memory cache.
    pub hits: u64,
    /// Include byte lookups that reached the loader.
    pub misses: u64,
    /// Current include byte cache entries retained for this translation unit.
    pub entries: usize,
    /// Header byte cache entries evicted to enforce translation-unit budgets.
    pub evictions: u64,
    /// Bytes currently retained by the include byte cache.
    pub retained_bytes: u64,
    /// Header bytes loaded through the include loader.
    pub loaded_bytes: u64,
    /// Header bytes reused from the in-memory cache.
    pub reused_bytes: u64,
}

/// Include event emitted by the GPU-resident preprocessor driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeEvent {
    /// File that contained the include directive.
    pub includer: std::path::PathBuf,
    /// Raw include spelling extracted by the GPU include parser.
    pub requested_path: Vec<u8>,
    /// Canonical path returned by the include loader.
    pub resolved_path: std::path::PathBuf,
    /// Directive row in the classified token stream.
    pub directive_row: u32,
    /// Byte offset of the include directive token in the filtered source.
    pub directive_byte_offset: u32,
    /// Whether the request used `<...>` system include spelling.
    pub is_system: bool,
    /// Whether the request came from GNU `#include_next`.
    pub is_next: bool,
    /// Residency of the request extractor.
    pub request_residency: IncludeEventResidency,
    /// Residency of the path resolution/read side.
    pub resolution_residency: IncludeEventResidency,
}
