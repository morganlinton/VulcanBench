use super::*;
/// Options for compiling C translation units through the CUDA-first frontend.
pub struct VyreCompileOptions {
    /// `-c` was supplied: emit a `.o` artifact and skip linking.
    pub is_compile_only: bool,
    /// C source files to compile, in CLI order.
    pub input_files: Vec<PathBuf>,
    /// Override for the output path; valid for one compile-only input. Link mode is rejected by
    /// the CUDA-first frontend, and multiple compile-only inputs emit per-input `.o` files.
    pub output_file: Option<PathBuf>,
    /// `-I` directories to add to the include search path.
    pub include_dirs: Vec<PathBuf>,
    /// `-iquote` directories searched after the including file directory for
    /// quoted includes only.
    pub quote_include_dirs: Vec<PathBuf>,
    /// `-isystem` directories searched as explicit system roots before
    /// compiler-discovered system defaults.
    pub system_include_dirs: Vec<PathBuf>,
    /// `-idirafter` directories searched after compiler-discovered system
    /// defaults.
    pub after_include_dirs: Vec<PathBuf>,
    /// Disable compiler-discovered default system include roots (`-nostdinc`).
    ///
    /// CLI-provided include roots are still honored. This flag exists because
    /// silently ignoring `-nostdinc` binds corpus translation units to host ABI
    /// headers that clang/gcc would not search for that invocation.
    pub disable_system_include_dirs: bool,
    /// Sysroot used to relocate compiler-discovered system include roots.
    ///
    /// This models `--sysroot=<dir>` / `-isysroot <dir>` at the frontend
    /// boundary. It affects only compiler-discovered system include roots;
    /// explicit `-I` / `-isystem` roots are already canonicalized by the CLI
    /// and remain exact.
    pub system_include_sysroot: Option<PathBuf>,
    /// Target, dialect, and predefined-macro semantic options.
    pub target: CTargetOptions,
    /// `-include` operands prepended before the translation unit body, in CLI order.
    ///
    /// Values may be absolute paths or include-search names such as
    /// `generated/autoconf.h`; the preprocessor resolves them through the
    /// normal include search path after CLI include roots are applied.
    pub forced_include_files: Vec<PathBuf>,
    /// `-imacros` operands processed before forced includes and the main TU.
    ///
    /// Each file is preprocessed through the same GPU-resident preprocessor as
    /// normal source, but its emitted tokens are discarded and only the live
    /// macro table is carried forward. This matches compiler driver semantics:
    /// `-imacros config.h` imports macro definitions without injecting
    /// declarations or statements into the translation unit stream.
    pub imacro_files: Vec<PathBuf>,
    /// Compatibility-only `-D NAME[=VALUE]` macro definitions.
    ///
    /// Empty value is `Some("")` and `-D NAME` is `None`. New callers must use
    /// [`Self::macro_actions`] so command-line order is preserved. Production
    /// resident preprocessing rejects invocations that populate both this
    /// legacy unordered field and `macro_actions`.
    pub macros: Vec<(String, Option<String>)>,
    /// Compatibility-only `-U NAME` macro undefinitions.
    ///
    /// Used after `macros` only when `macro_actions` is empty to preserve the
    /// legacy API contract. Production resident preprocessing rejects mixed
    /// use with `macro_actions`.
    pub undefs: Vec<String>,
    /// Ordered command-line macro actions from `-D` and `-U`.
    ///
    /// Use this for compiler-accurate CLI semantics: `-D FOO -U FOO` and
    /// `-U FOO -D FOO` intentionally produce different macro environments.
    /// When empty, the frontend falls back to the legacy `macros` then
    /// `undefs` ordering for existing library callers.
    pub macro_actions: Vec<CliMacroAction>,
}

impl Default for VyreCompileOptions {
    fn default() -> Self {
        Self {
            is_compile_only: false,
            input_files: Vec::new(),
            output_file: None,
            include_dirs: Vec::new(),
            quote_include_dirs: Vec::new(),
            system_include_dirs: Vec::new(),
            after_include_dirs: Vec::new(),
            disable_system_include_dirs: false,
            system_include_sysroot: None,
            target: CTargetOptions::default(),
            forced_include_files: Vec::new(),
            imacro_files: Vec::new(),
            macros: Vec::new(),
            undefs: Vec::new(),
            macro_actions: Vec::new(),
        }
    }
}

/// Ordered command-line macro action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CliMacroAction {
    /// `-D NAME[=VALUE]`.
    Define {
        /// Macro name.
        name: String,
        /// Macro body; `None` means compiler-style `1`.
        value: Option<String>,
    },
    /// `-D NAME(PARAMS)[=VALUE]`.
    DefineFunction {
        /// Macro name.
        name: String,
        /// Raw parameter spellings, in order.
        params: Vec<String>,
        /// Macro body; `None` means compiler-style `1`.
        value: Option<String>,
    },
    /// `-U NAME`.
    Undef {
        /// Macro name.
        name: String,
    },
}
