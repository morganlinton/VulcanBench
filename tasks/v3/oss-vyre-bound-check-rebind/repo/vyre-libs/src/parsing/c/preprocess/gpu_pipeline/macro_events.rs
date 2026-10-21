//! Macro definition-table evidence emitted by the GPU preprocessor driver.

/// Macro table event kind emitted by the GPU preprocessor driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroEventKind {
    /// `#define`.
    Define,
    /// `#undef`.
    Undef,
}

/// Macro definition-table event emitted by the GPU-resident preprocessor driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroEvent {
    /// File that contained the macro directive.
    pub file: std::path::PathBuf,
    /// Event kind.
    pub kind: MacroEventKind,
    /// Directive row in the classified token stream.
    pub directive_row: u32,
    /// Byte offset of the directive token in the filtered source.
    pub directive_byte_offset: u32,
    /// Stable macro symbol ID derived from the macro name bytes.
    pub symbol_id: [u8; 16],
    /// Macro name bytes.
    pub name: Vec<u8>,
    /// Macro name byte range in the filtered source.
    pub name_range: Option<(u32, u32)>,
    /// Parameter metadata bytes for function-like macros.
    pub args: Vec<u8>,
    /// Parameter metadata byte range in the filtered source.
    pub args_range: Option<(u32, u32)>,
    /// Replacement body bytes.
    pub replacement: Vec<u8>,
    /// Replacement byte range in the filtered source.
    pub replacement_range: Option<(u32, u32)>,
    /// Whether this macro uses function-like syntax.
    pub is_function_like: bool,
    /// Whether this macro is variadic.
    pub is_variadic: bool,
    /// Whether directive payload extraction was GPU-resident.
    pub gpu_resident: bool,
}

/// Stable macro symbol ID.
pub(super) fn stable_macro_symbol_id(name: &[u8]) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(name.len() as u64).to_le_bytes());
    hasher.update(name);
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest.as_bytes()[..16]);
    out
}

/// Returns whether function-like macro parameter metadata is variadic.
pub(super) fn macro_args_are_variadic(args: &[u8]) -> bool {
    args.windows(3).any(|window| window == b"...")
        || args.windows(11).any(|window| window == b"__VA_ARGS__")
}
