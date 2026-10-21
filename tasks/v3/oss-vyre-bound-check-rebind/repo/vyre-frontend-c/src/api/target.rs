//! Target, dialect, and predefined-macro option model for the C frontend.
//!
//! These types are the API boundary for flags that affect preprocessing or
//! semantic layout before lowering. If a new compiler flag changes builtin
//! macros, ABI sizes, default type signedness, or C standard mode, model it
//! here instead of treating it as parser-neutral noise.

/// Complete target/preprocessor semantic option bundle.
///
/// Keep these knobs grouped so every compile path, cache key, predefine
/// builder, and CLI bridge has one obvious object to thread. Splitting these
/// across unrelated `VyreCompileOptions` fields makes partial migrations easy:
/// e.g. wiring `-m32` into ABI layout but not predefined macros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CTargetOptions {
    /// Target architecture used for compiler predefined architecture macros.
    pub arch: CTargetArch,
    /// Target C data-model ABI used by semantic layout stages.
    pub abi: CTargetAbi,
    /// Target default signedness for plain C `char`.
    pub char_signedness: CCharSignedness,
    /// C hosted/freestanding environment model.
    pub environment: CEnvironment,
    /// C language dialect selected by `-std=...`.
    pub dialect: CDialect,
    /// Compiler compatibility predefine profile.
    pub compiler_predefine_profile: CCompilerPredefineProfile,
    /// Scope of compiler predefined macros.
    pub predefine_scope: CPredefineScope,
}

impl CTargetOptions {
    /// Stable cache discriminator for every target/predefine semantic option.
    ///
    /// Cache sites should use this single tag instead of hashing fields one by
    /// one. That keeps future semantic flags from accidentally missing one
    /// cache layer.
    pub const fn cache_tag(self) -> u64 {
        self.arch.cache_tag().wrapping_mul(0x1000_0000_01b3)
            ^ self.abi.cache_tag().wrapping_mul(0x1000_0000_01b3)
            ^ self.char_signedness.cache_tag()
            ^ self.environment.cache_tag().rotate_left(7)
            ^ self.dialect.cache_tag().rotate_left(13)
            ^ self.compiler_predefine_profile.cache_tag().rotate_left(29)
            ^ self.predefine_scope.cache_tag().rotate_left(41)
    }
}

/// Target CPU architecture for compiler predefined macros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CTargetArch {
    /// AMD64/x86-64 architecture in the GNU/Clang predefined macro model.
    X86_64,
}

impl Default for CTargetArch {
    fn default() -> Self {
        Self::X86_64
    }
}

impl CTargetArch {
    /// Stable cache discriminator for preprocessor and semantic keys.
    pub const fn cache_tag(self) -> u64 {
        match self {
            Self::X86_64 => 0x5838_365f_3634,
        }
    }

    /// Whether the target compiler model exposes 128-bit integer support.
    pub const fn supports_int128(self) -> bool {
        match self {
            Self::X86_64 => true,
        }
    }
}

/// C target data-model ABI for pre-lowering semantic layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CTargetAbi {
    /// LP64 data model: `int` is 32-bit, `long` and pointers are 64-bit.
    Lp64,
    /// ILP32 data model: `int`, `long`, and pointers are 32-bit.
    Ilp32,
}

impl Default for CTargetAbi {
    fn default() -> Self {
        Self::Lp64
    }
}

impl CTargetAbi {
    /// Pointer width in bytes for GPU ABI layout kernels.
    pub const fn pointer_size_bytes(self) -> u32 {
        match self {
            Self::Lp64 => 8,
            Self::Ilp32 => 4,
        }
    }

    /// C `long` width in bytes for GPU ABI layout kernels.
    pub const fn long_size_bytes(self) -> u32 {
        match self {
            Self::Lp64 => 8,
            Self::Ilp32 => 4,
        }
    }

    /// C `double` alignment in bytes for GPU ABI layout kernels.
    pub const fn double_alignment_bytes(self) -> u32 {
        match self {
            Self::Lp64 => 8,
            Self::Ilp32 => 4,
        }
    }

    /// Predefined C type spelling for `size_t`.
    pub const fn size_type_macro(self) -> &'static str {
        match self {
            Self::Lp64 => "long unsigned int",
            Self::Ilp32 => "unsigned int",
        }
    }

    /// Predefined C type spelling for `ptrdiff_t`.
    pub const fn ptrdiff_type_macro(self) -> &'static str {
        match self {
            Self::Lp64 => "long int",
            Self::Ilp32 => "int",
        }
    }

    /// Predefined C type spelling for `intptr_t`.
    pub const fn intptr_type_macro(self) -> &'static str {
        match self {
            Self::Lp64 => "long int",
            Self::Ilp32 => "int",
        }
    }

    /// Predefined C type spelling for `uintptr_t`.
    pub const fn uintptr_type_macro(self) -> &'static str {
        match self {
            Self::Lp64 => "long unsigned int",
            Self::Ilp32 => "unsigned int",
        }
    }

    /// Predefined C type spelling for `intmax_t`.
    pub const fn intmax_type_macro(self) -> &'static str {
        match self {
            Self::Lp64 => "long int",
            Self::Ilp32 => "long long int",
        }
    }

    /// Predefined C type spelling for `uintmax_t`.
    pub const fn uintmax_type_macro(self) -> &'static str {
        match self {
            Self::Lp64 => "long unsigned int",
            Self::Ilp32 => "long long unsigned int",
        }
    }

    /// Width in bits for pointer-sized unsigned integer types.
    pub const fn pointer_width_bits(self) -> u32 {
        self.pointer_size_bytes() * 8
    }

    /// C `long` width in bits.
    pub const fn long_width_bits(self) -> u32 {
        self.long_size_bytes() * 8
    }

    /// Predefined max-value spelling for `size_t` and `uintptr_t`.
    pub const fn uintptr_max_macro(self) -> &'static str {
        match self {
            Self::Lp64 => "18446744073709551615UL",
            Self::Ilp32 => "4294967295U",
        }
    }

    /// Predefined max-value spelling for `ptrdiff_t` and `intptr_t`.
    pub const fn intptr_max_macro(self) -> &'static str {
        match self {
            Self::Lp64 => "9223372036854775807L",
            Self::Ilp32 => "2147483647",
        }
    }

    /// Predefined max-value spelling for C `long`.
    pub const fn long_max_macro(self) -> &'static str {
        match self {
            Self::Lp64 => "9223372036854775807L",
            Self::Ilp32 => "2147483647L",
        }
    }

    /// Predefined max-value spelling for `intmax_t`.
    pub const fn intmax_max_macro(self) -> &'static str {
        match self {
            Self::Lp64 => "9223372036854775807L",
            Self::Ilp32 => "9223372036854775807LL",
        }
    }

    /// Predefined max-value spelling for `uintmax_t`.
    pub const fn uintmax_max_macro(self) -> &'static str {
        match self {
            Self::Lp64 => "18446744073709551615UL",
            Self::Ilp32 => "18446744073709551615ULL",
        }
    }

    /// Stable cache discriminator for semantic-summary cache keys.
    pub const fn cache_tag(self) -> u64 {
        match self {
            Self::Lp64 => 0x4c50_3634,
            Self::Ilp32 => 0x494c_5033_32,
        }
    }
}

/// Target default signedness for plain C `char`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CCharSignedness {
    /// Plain `char` behaves as signed char.
    Signed,
    /// Plain `char` behaves as unsigned char.
    Unsigned,
}

impl Default for CCharSignedness {
    fn default() -> Self {
        Self::Signed
    }
}

impl CCharSignedness {
    /// Stable cache discriminator for preprocessor and semantic keys.
    pub const fn cache_tag(self) -> u64 {
        match self {
            Self::Signed => 0x5343_4841_52,
            Self::Unsigned => 0x5543_4841_52,
        }
    }

    /// Predefined max-value spelling for plain `char`.
    pub const fn char_max_macro(self) -> &'static str {
        match self {
            Self::Signed => "127",
            Self::Unsigned => "255",
        }
    }
}

/// C implementation environment for `__STDC_HOSTED__`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CEnvironment {
    /// Hosted C implementation (`__STDC_HOSTED__ == 1`).
    Hosted,
    /// Freestanding C implementation (`__STDC_HOSTED__ == 0`).
    Freestanding,
}

impl Default for CEnvironment {
    fn default() -> Self {
        Self::Hosted
    }
}

impl CEnvironment {
    /// Stable cache discriminator for preprocessor and semantic keys.
    pub const fn cache_tag(self) -> u64 {
        match self {
            Self::Hosted => 0x484f_5354_4544,
            Self::Freestanding => 0x4652_4545,
        }
    }

    /// Predefined value for `__STDC_HOSTED__`.
    pub const fn stdc_hosted_macro(self) -> &'static str {
        match self {
            Self::Hosted => "1",
            Self::Freestanding => "0",
        }
    }
}

/// C language dialect for preprocessor-visible standard macros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CDialect {
    /// ISO C89/C90.
    C89,
    /// GNU C89/C90.
    Gnu89,
    /// ISO C99.
    C99,
    /// GNU C99.
    Gnu99,
    /// ISO C11.
    C11,
    /// GNU C11.
    Gnu11,
    /// ISO C17/C18.
    C17,
    /// GNU C17/C18.
    Gnu17,
    /// ISO C23.
    C23,
    /// GNU C23.
    Gnu23,
}

impl Default for CDialect {
    fn default() -> Self {
        Self::Gnu11
    }
}

impl CDialect {
    /// Stable cache discriminator for preprocessor and semantic keys.
    pub const fn cache_tag(self) -> u64 {
        match self {
            Self::C89 => 0x4338_39,
            Self::Gnu89 => 0x474e_5538_39,
            Self::C99 => 0x4339_39,
            Self::Gnu99 => 0x474e_5539_39,
            Self::C11 => 0x4331_31,
            Self::Gnu11 => 0x474e_5531_31,
            Self::C17 => 0x4331_37,
            Self::Gnu17 => 0x474e_5531_37,
            Self::C23 => 0x4332_33,
            Self::Gnu23 => 0x474e_5532_33,
        }
    }

    /// Whether this dialect is an ISO mode rather than a GNU extension mode.
    pub const fn is_strict_ansi(self) -> bool {
        matches!(
            self,
            Self::C89 | Self::C99 | Self::C11 | Self::C17 | Self::C23
        )
    }

    /// C standard version macro value, if the standard defines one.
    pub const fn stdc_version_macro(self) -> Option<&'static str> {
        match self {
            Self::C89 | Self::Gnu89 => None,
            Self::C99 | Self::Gnu99 => Some("199901L"),
            Self::C11 | Self::Gnu11 => Some("201112L"),
            Self::C17 | Self::Gnu17 => Some("201710L"),
            Self::C23 | Self::Gnu23 => Some("202311L"),
        }
    }
}

/// Compiler compatibility profile for builtin predefined macros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CCompilerPredefineProfile {
    /// GNU-compatible predefined macro profile.
    GnuCompat,
}

impl Default for CCompilerPredefineProfile {
    fn default() -> Self {
        Self::GnuCompat
    }
}

impl CCompilerPredefineProfile {
    /// Stable cache discriminator for preprocessor and semantic keys.
    pub const fn cache_tag(self) -> u64 {
        match self {
            Self::GnuCompat => 0x474e_5543_4f4d_5041,
        }
    }
}

/// Which predefined macros are emitted before user `-D`/`-U` actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CPredefineScope {
    /// Emit standard C macros plus compiler/target compatibility macros.
    Full,
    /// Emit only standard C macros.
    StandardOnly,
}

impl Default for CPredefineScope {
    fn default() -> Self {
        Self::Full
    }
}

impl CPredefineScope {
    /// Stable cache discriminator for preprocessor and semantic keys.
    pub const fn cache_tag(self) -> u64 {
        match self {
            Self::Full => 0x5052_4544_4655_4c4c,
            Self::StandardOnly => 0x5052_4544_5354_4443,
        }
    }
}
