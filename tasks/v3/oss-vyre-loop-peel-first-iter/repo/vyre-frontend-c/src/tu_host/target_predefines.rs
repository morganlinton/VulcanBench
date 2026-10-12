use crate::api::{
    CCharSignedness, CCompilerPredefineProfile, CDialect, CPredefineScope, CTargetAbi, CTargetArch,
    CTargetOptions, CliMacroAction,
};

/// Compiler predefined macros derived from explicit frontend target options.
///
/// This is the single source of truth for preprocessor-visible target facts.
/// Do not add ad hoc builtin defines in `tu_host.rs`; model the option on
/// `VyreCompileOptions`, include it in cache keys, and emit the macro here.
pub(super) fn predefined_macro_actions(target: CTargetOptions) -> Vec<CliMacroAction> {
    let pointer_size = target.abi.pointer_size_bytes().to_string();
    let pointer_width = target.abi.pointer_width_bits().to_string();
    let long_size = target.abi.long_size_bytes().to_string();
    let pointer_width_bits = target.abi.pointer_width_bits().to_string();
    let long_width_bits = target.abi.long_width_bits().to_string();
    let uintptr_max = target.abi.uintptr_max_macro().to_string();
    let intptr_max = target.abi.intptr_max_macro().to_string();
    let uintmax_max = target.abi.uintmax_max_macro().to_string();
    let intmax_max = target.abi.intmax_max_macro().to_string();
    let long_max = target.abi.long_max_macro().to_string();
    let mut actions = Vec::with_capacity(84);
    if let Some(stdc_version) = target.dialect.stdc_version_macro() {
        define(&mut actions, "__STDC_VERSION__", stdc_version);
    }
    if target.dialect.is_strict_ansi() {
        define(&mut actions, "__STRICT_ANSI__", "1");
    }
    define(&mut actions, "__STDC__", "1");
    define(
        &mut actions,
        "__STDC_HOSTED__",
        target.environment.stdc_hosted_macro(),
    );
    if matches!(target.predefine_scope, CPredefineScope::StandardOnly) {
        return actions;
    }
    append_compiler_profile(
        &mut actions,
        target.compiler_predefine_profile,
        target.dialect,
    );
    append_arch_profile(&mut actions, target.arch);
    define(&mut actions, "__CHAR_BIT__", "8");
    define(
        &mut actions,
        "__CHAR_MAX__",
        target.char_signedness.char_max_macro(),
    );
    define(&mut actions, "__SCHAR_MAX__", "127");
    if matches!(target.char_signedness, CCharSignedness::Unsigned) {
        define(&mut actions, "__CHAR_UNSIGNED__", "1");
    }
    define(&mut actions, "__SHRT_MAX__", "32767");
    define(&mut actions, "__INT_MAX__", "2147483647");
    define(&mut actions, "__LONG_MAX__", &long_max);
    define(&mut actions, "__LONG_LONG_MAX__", "9223372036854775807LL");
    define(&mut actions, "__WCHAR_MAX__", "2147483647");
    define(&mut actions, "__WINT_MAX__", "4294967295U");
    define(&mut actions, "__SIG_ATOMIC_MAX__", "2147483647");
    define(&mut actions, "__INT_WIDTH__", "32");
    define(&mut actions, "__LONG_WIDTH__", &long_width_bits);
    define(&mut actions, "__LONG_LONG_WIDTH__", "64");
    define(&mut actions, "__WCHAR_WIDTH__", "32");
    define(&mut actions, "__WINT_WIDTH__", "32");
    define(&mut actions, "__ORDER_LITTLE_ENDIAN__", "1234");
    define(&mut actions, "__ORDER_BIG_ENDIAN__", "4321");
    define(&mut actions, "__BYTE_ORDER__", "1234");
    define(&mut actions, "__SIZEOF_POINTER__", &pointer_size);
    define(&mut actions, "__POINTER_WIDTH__", &pointer_width);
    if target.arch.supports_int128() {
        define(&mut actions, "__SIZEOF_INT128__", "16");
    }
    define(&mut actions, "__SIZEOF_LONG__", &long_size);
    define(&mut actions, "__SIZEOF_SIZE_T__", &pointer_size);
    define(&mut actions, "__SIZEOF_PTRDIFF_T__", &pointer_size);
    define(&mut actions, "__SIZEOF_SHORT__", "2");
    define(&mut actions, "__SIZEOF_INT__", "4");
    define(&mut actions, "__SIZEOF_FLOAT__", "4");
    define(&mut actions, "__SIZEOF_DOUBLE__", "8");
    define(&mut actions, "__SIZEOF_LONG_LONG__", "8");
    define(&mut actions, "__SIZEOF_WCHAR_T__", "4");
    define(&mut actions, "__SIZEOF_WINT_T__", "4");
    define(&mut actions, "__SIZE_WIDTH__", &pointer_width_bits);
    define(&mut actions, "__PTRDIFF_WIDTH__", &pointer_width_bits);
    define(&mut actions, "__INTPTR_WIDTH__", &pointer_width_bits);
    define(&mut actions, "__UINTPTR_WIDTH__", &pointer_width_bits);
    define(&mut actions, "__INTMAX_WIDTH__", "64");
    define(&mut actions, "__UINTMAX_WIDTH__", "64");
    define(&mut actions, "__SIZE_TYPE__", target.abi.size_type_macro());
    define(
        &mut actions,
        "__PTRDIFF_TYPE__",
        target.abi.ptrdiff_type_macro(),
    );
    define(
        &mut actions,
        "__INTPTR_TYPE__",
        target.abi.intptr_type_macro(),
    );
    define(
        &mut actions,
        "__UINTPTR_TYPE__",
        target.abi.uintptr_type_macro(),
    );
    define(
        &mut actions,
        "__INTMAX_TYPE__",
        target.abi.intmax_type_macro(),
    );
    define(
        &mut actions,
        "__UINTMAX_TYPE__",
        target.abi.uintmax_type_macro(),
    );
    define(&mut actions, "__WCHAR_TYPE__", "int");
    define(&mut actions, "__WINT_TYPE__", "unsigned int");
    define(&mut actions, "__SIG_ATOMIC_TYPE__", "int");
    define(&mut actions, "__SIZE_MAX__", &uintptr_max);
    define(&mut actions, "__UINTPTR_MAX__", &uintptr_max);
    define(&mut actions, "__PTRDIFF_MAX__", &intptr_max);
    define(&mut actions, "__INTPTR_MAX__", &intptr_max);
    define(&mut actions, "__INTMAX_MAX__", &intmax_max);
    define(&mut actions, "__UINTMAX_MAX__", &uintmax_max);
    match target.abi {
        CTargetAbi::Lp64 => {
            define(&mut actions, "__LP64__", "1");
            define(&mut actions, "_LP64", "1");
        }
        CTargetAbi::Ilp32 => {
            define(&mut actions, "__ILP32__", "1");
            define(&mut actions, "_ILP32", "1");
        }
    }
    actions
}

fn append_arch_profile(actions: &mut Vec<CliMacroAction>, arch: CTargetArch) {
    match arch {
        CTargetArch::X86_64 => {
            define(actions, "__x86_64__", "1");
            define(actions, "__x86_64", "1");
            define(actions, "__amd64__", "1");
            define(actions, "__amd64", "1");
        }
    }
}

fn append_compiler_profile(
    actions: &mut Vec<CliMacroAction>,
    profile: CCompilerPredefineProfile,
    dialect: CDialect,
) {
    match profile {
        CCompilerPredefineProfile::GnuCompat => {
            define(actions, "__GNUC__", "4");
            define(actions, "__GNUC_MINOR__", "2");
            define(actions, "__GNUC_PATCHLEVEL__", "1");
            define(actions, "__GNUC_STDC_INLINE__", "1");
            define_feature_query(actions, "__has_attribute");
            define_feature_query(actions, "__has_builtin");
            define_feature_query(actions, "__has_feature");
            define_feature_query(actions, "__has_extension");
            define(actions, "__ELF__", "1");
            define(actions, "__unix", "1");
            define(actions, "__unix__", "1");
            define(actions, "__linux", "1");
            define(actions, "__linux__", "1");
            if !dialect.is_strict_ansi() {
                define(actions, "unix", "1");
                define(actions, "linux", "1");
            }
        }
    }
}

fn define(actions: &mut Vec<CliMacroAction>, name: &str, value: &str) {
    actions.push(CliMacroAction::Define {
        name: name.to_string(),
        value: Some(value.to_string()),
    });
}

fn define_feature_query(actions: &mut Vec<CliMacroAction>, name: &str) {
    actions.push(CliMacroAction::DefineFunction {
        name: name.to_string(),
        params: vec!["x".to_string()],
        value: Some("0".to_string()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{CEnvironment, CPredefineScope};

    fn defined_value(actions: &[CliMacroAction], expected: &str) -> Option<String> {
        actions.iter().find_map(|action| match action {
            CliMacroAction::Define { name, value } if name == expected => value.clone(),
            _ => None,
        })
    }

    #[test]
    fn lp64_predefines_include_libc_scalar_types() {
        let target = CTargetOptions {
            abi: CTargetAbi::Lp64,
            ..CTargetOptions::default()
        };
        let actions = predefined_macro_actions(target);
        assert_eq!(
            defined_value(&actions, "__SIZE_TYPE__").as_deref(),
            Some("long unsigned int")
        );
        assert_eq!(defined_value(&actions, "__x86_64__").as_deref(), Some("1"));
        assert_eq!(
            defined_value(&actions, "__SIZEOF_INT128__").as_deref(),
            Some("16")
        );
        assert_eq!(
            defined_value(&actions, "__INTMAX_TYPE__").as_deref(),
            Some("long int")
        );
        assert_eq!(
            defined_value(&actions, "__UINTMAX_MAX__").as_deref(),
            Some("18446744073709551615UL")
        );
        assert_eq!(
            defined_value(&actions, "__WCHAR_TYPE__").as_deref(),
            Some("int")
        );
        assert_eq!(
            defined_value(&actions, "__WINT_TYPE__").as_deref(),
            Some("unsigned int")
        );
        assert_eq!(
            defined_value(&actions, "__SIG_ATOMIC_TYPE__").as_deref(),
            Some("int")
        );
    }

    #[test]
    fn ilp32_predefines_use_long_long_for_intmax() {
        let target = CTargetOptions {
            abi: CTargetAbi::Ilp32,
            ..CTargetOptions::default()
        };
        let actions = predefined_macro_actions(target);
        assert_eq!(
            defined_value(&actions, "__SIZE_TYPE__").as_deref(),
            Some("unsigned int")
        );
        assert_eq!(
            defined_value(&actions, "__INTMAX_TYPE__").as_deref(),
            Some("long long int")
        );
        assert_eq!(
            defined_value(&actions, "__INTMAX_MAX__").as_deref(),
            Some("9223372036854775807LL")
        );
        assert_eq!(
            defined_value(&actions, "__UINTMAX_MAX__").as_deref(),
            Some("18446744073709551615ULL")
        );
    }

    #[test]
    fn standard_only_scope_keeps_standard_macros_without_target_extensions() {
        let target = CTargetOptions {
            environment: CEnvironment::Freestanding,
            predefine_scope: CPredefineScope::StandardOnly,
            ..CTargetOptions::default()
        };
        let actions = predefined_macro_actions(target);
        assert_eq!(defined_value(&actions, "__STDC__").as_deref(), Some("1"));
        assert_eq!(
            defined_value(&actions, "__STDC_HOSTED__").as_deref(),
            Some("0")
        );
        assert_eq!(defined_value(&actions, "__GNUC__"), None);
        assert_eq!(defined_value(&actions, "__SIZE_TYPE__"), None);
        assert_eq!(defined_value(&actions, "__WCHAR_TYPE__"), None);
    }
}
