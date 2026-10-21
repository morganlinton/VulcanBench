pub(crate) const REPLACEMENT_TOKEN_CACHE_MAX_ENTRIES: usize = 16_384;
pub(crate) const REPLACEMENT_TOKEN_CACHE_MAX_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(crate) struct ReplacementTokenCacheKey {
    pub(crate) symbol_id: [u8; 16],
    pub(crate) body_hash: [u8; 16],
    pub(crate) args_hash: [u8; 16],
    pub(crate) is_function_like: bool,
}

/// Source provenance for one preprocessed output token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenProvenanceEvent {
    /// File that contributed the preprocessed output token.
    pub file: std::path::PathBuf,
    /// Byte offset of the token in the final preprocessed output stream.
    pub output_start: u32,
    /// Byte length of the token in the final preprocessed output stream.
    pub output_len: u32,
    /// File containing the spelling bytes for this token.
    pub spelling_file: std::path::PathBuf,
    /// Byte offset of the token spelling in `spelling_file`'s filtered source.
    pub spelling_start: u32,
    /// Byte length of the token spelling in `spelling_file`'s filtered source.
    pub spelling_len: u32,
    /// File containing the expansion use site.
    pub expansion_file: std::path::PathBuf,
    /// Byte offset of the expansion use site in `expansion_file`'s filtered source.
    pub expansion_start: u32,
    /// Byte length of the expansion use site in `expansion_file`'s filtered source.
    pub expansion_len: u32,
    /// Include stack active when this output token was produced.
    pub include_stack: Vec<std::path::PathBuf>,
    /// Stable macro symbol ID for macro-produced tokens; `None` for direct tokens.
    pub macro_symbol_id: Option<[u8; 16]>,
    /// Macro name for macro-produced tokens; empty for direct tokens.
    pub macro_name: Vec<u8>,
    /// Whether the token provenance describes output from GPU-resident preprocessing.
    pub gpu_resident: bool,
}
