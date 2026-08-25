//! Adversarial GPU preprocessor coverage for release-plan item 29.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

mod support;

use support::gpu_preprocess::{IncludeEventResidencyAssert, NullLoader, ReferenceDispatcher};

#[allow(unused_imports)]
use vyre_driver_cuda as _;
#[allow(unused_imports)]
use vyre_driver_wgpu as _;

use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    gpu_filter_source_bytes, gpu_preprocess_translation_unit, BackendDispatcher,
    IncludeAccelerationKind, IncludeLoader,
};

#[test]
fn adversarial_recursive_object_macro_does_not_loop_or_cpu_fallback() {
    let dispatcher = ReferenceDispatcher;
    let loader = NullLoader;
    let path = PathBuf::from("recursive_macro.c");
    let raw = b"#define R R\nint x = R;\n";

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("recursive self-reference must terminate without fallback");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert!(out.contains("R"));
    assert!(res.macro_expansion_events.iter().any(|event| {
        event.name == b"R" && event.gpu_resident && event.include_stack.is_empty()
    }));
    assert!(res
        .token_provenance_events
        .iter()
        .any(|event| event.macro_name == b"R" && event.gpu_resident));
}

#[test]
fn adversarial_deep_include_chain_preserves_stack_and_depth() {
    struct ChainLoader;

    impl IncludeLoader for ChainLoader {
        fn load(
            &self,
            path: &[u8],
            _is_system: bool,
            _is_next: bool,
            _from: &Path,
        ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
            let name = std::str::from_utf8(path).map_err(|error| error.to_string())?;
            let idx_text = name
                .strip_prefix('h')
                .and_then(|rest| rest.strip_suffix(".h"))
                .ok_or_else(|| format!("unexpected include name {name}"))?;
            let idx: usize = idx_text.parse().map_err(|error| format!("{error}"))?;
            let body = if idx < 16 {
                format!("#include \"h{}.h\"\nint v{};\n", idx + 1, idx)
            } else {
                "int vend;\n".to_string()
            };
            Ok(Some((
                PathBuf::from(format!("/tmp/vyrec-deep/h{idx}.h")),
                body.into_bytes().into(),
            )))
        }
    }

    let dispatcher = ReferenceDispatcher;
    let loader = ChainLoader;
    let path = PathBuf::from("/tmp/vyrec-deep/main.c");
    let raw = b"#include \"h0.h\"\n";

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("deep include chain must preprocess");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert!(out.contains("int vend;"));
    assert!(out.contains("int v0;"));
    assert_eq!(res.include_events.len(), 17);
    assert!(res
        .include_events
        .iter()
        .all(|event| event.request_residency.is_gpu_resident_request()));
}

#[test]
fn adversarial_disabled_branches_do_not_load_include_or_fire_error() {
    struct PanicLoader;

    impl IncludeLoader for PanicLoader {
        fn load(
            &self,
            path: &[u8],
            _is_system: bool,
            _is_next: bool,
            _from: &Path,
        ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
            panic!(
                "disabled include must not reach loader: {}",
                String::from_utf8_lossy(path)
            );
        }
    }

    let dispatcher = ReferenceDispatcher;
    let loader = PanicLoader;
    let path = PathBuf::from("disabled_branch.c");
    let raw = concat!(
        "#if 0\n",
        "#include \"must_not_load.h\"\n",
        "#error must_not_fire\n",
        "#define HIDDEN 1\n",
        "#endif\n",
        "int visible;\n",
    )
    .as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("disabled branch must not execute side effects");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert!(out.contains("int visible;"));
    assert!(!out.contains("HIDDEN"));
    assert!(res.include_events.is_empty());
    assert!(res.macro_events.iter().all(|event| event.name != b"HIDDEN"));
}

#[test]
fn adversarial_linux_config_compound_conditionals_prune_include_side_effects() {
    struct LinuxConfigLoader {
        live_loads: AtomicUsize,
    }

    impl IncludeLoader for LinuxConfigLoader {
        fn load(
            &self,
            path: &[u8],
            _is_system: bool,
            _is_next: bool,
            _from: &Path,
        ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
            match path {
                b"live.h" => {
                    self.live_loads.fetch_add(1, Ordering::SeqCst);
                    Ok(Some((
                        PathBuf::from("/tmp/vyrec-linux-config/live.h"),
                        b"int live_from_linux_config;\n".to_vec().into(),
                    )))
                }
                other => panic!(
                    "inactive Linux config include must not reach loader: {}",
                    String::from_utf8_lossy(other)
                ),
            }
        }
    }

    let dispatcher = ReferenceDispatcher;
    let loader = LinuxConfigLoader {
        live_loads: AtomicUsize::new(0),
    };
    let path = PathBuf::from("linux_config_compound.c");
    let raw = concat!(
        "#define CONFIG_ON 1\n",
        "#if IS_ENABLED(CONFIG_ON) && !defined(CONFIG_OFF)\n",
        "#include \"live.h\"\n",
        "#endif\n",
        "#if IS_ENABLED(CONFIG_OFF) || IS_MODULE(CONFIG_OFF)\n",
        "#include \"must_not_load.h\"\n",
        "#error must_not_fire\n",
        "#endif\n",
        "int tail;\n",
    )
    .as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("compound Linux config conditionals must preprocess");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert_eq!(loader.live_loads.load(Ordering::SeqCst), 1);
    assert!(out.contains("int live_from_linux_config;"), "{out}");
    assert!(out.contains("int tail;"), "{out}");
    assert!(!out.contains("must_not_fire"), "{out}");
    assert_eq!(res.include_events.len(), 1);
    assert_eq!(res.include_events[0].requested_path, b"live.h");
}

#[test]
fn adversarial_include_guard_with_leading_directive_still_skips_repeat() {
    struct GuardLoader;

    impl IncludeLoader for GuardLoader {
        fn load(
            &self,
            path: &[u8],
            _is_system: bool,
            _is_next: bool,
            _from: &Path,
        ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
            assert_eq!(path, b"guarded.h");
            Ok(Some((
                PathBuf::from("/tmp/vyrec-guarded/guarded.h"),
                concat!(
                    "#line 1 \"guarded.h\"\n",
                    "#ifndef VYREC_GUARDED_H\n",
                    "#define VYREC_GUARDED_H\n",
                    "int guarded_once;\n",
                    "#endif\n",
                )
                .as_bytes()
                .to_vec()
                .into(),
            )))
        }
    }

    let dispatcher = ReferenceDispatcher;
    let loader = GuardLoader;
    let path = PathBuf::from("/tmp/vyrec-guarded/main.c");
    let raw = concat!("#include \"guarded.h\"\n", "#include \"guarded.h\"\n",).as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("guarded repeat include must preprocess");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert_eq!(out.matches("guarded_once").count(), 1, "{out}");
    assert!(res.include_acceleration_events.iter().any(|event| {
        event.kind == IncludeAccelerationKind::IncludeGuard
            && event.guard_macro == b"VYREC_GUARDED_H"
            && event.skipped_include
            && event.gpu_directive_derived
    }));
}

#[test]
fn adversarial_live_conditional_resolves_integer_macro_alias_chain() {
    let dispatcher = ReferenceDispatcher;
    let loader = NullLoader;
    let path = PathBuf::from("linux_hz_alias.c");
    let raw = concat!(
        "#define CONFIG_HZ 1000\n",
        "#define HZ 100\n",
        "# undef HZ\n",
        "# define HZ CONFIG_HZ\t/* Internal kernel timer frequency */\n",
        "#if HZ != 1000\n",
        "#error wrong HZ\n",
        "#endif\n",
        "int ok;\n",
    )
    .as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("live #if must resolve object-like integer alias chains");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert!(out.contains("int ok;"), "{out}");
    assert!(!out.contains("wrong HZ"), "{out}");
}

#[test]
fn adversarial_real_gpu_live_conditional_resolves_integer_macro_alias_chain() {
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("GPU backend must be available");
    let dispatcher = BackendDispatcher(backend.as_ref());
    let loader = NullLoader;
    let path = PathBuf::from("linux_hz_alias_real_gpu.c");
    let raw = concat!(
        "/* SPDX-License-Identifier: GPL-2.0 */\n",
        "#ifndef __ASM_GENERIC_PARAM_H\n",
        "#define __ASM_GENERIC_PARAM_H\n",
        "#define CONFIG_HZ 1000\n",
        "#define HZ 100\n",
        "# undef HZ\n",
        "# define HZ CONFIG_HZ\t/* Internal kernel timer frequency */\n",
        "# define USER_HZ 100\t\t/* some user interfaces are */\n",
        "#endif /* __ASM_GENERIC_PARAM_H */\n",
        "#if HZ != 1000\n",
        "#error wrong HZ\n",
        "#endif\n",
        "int ok;\n",
    )
    .as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("real GPU live #if must resolve object-like integer alias chains");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert!(out.contains("int ok;"), "{out}");
    assert!(!out.contains("wrong HZ"), "{out}");
}

#[test]
fn adversarial_real_gpu_filter_strips_multiline_block_comment_before_declaration() {
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("GPU backend must be available");
    let dispatcher = BackendDispatcher(backend.as_ref());
    let raw = concat!(
        "#define FORCE_FULL_FILTER \\\n",
        "    1\n",
        "/*\n",
        " * Some architectures need to provide custom definitions.\n",
        " * Generated wrappers may not exist yet.\n",
        " */\n",
        "struct ftrace_branch_data { int x; };\n",
    )
    .as_bytes();

    let filtered = gpu_filter_source_bytes(&dispatcher, raw)
        .expect("real GPU must filter multiline block comments");
    let out = std::str::from_utf8(&filtered.bytes).expect("filtered bytes are UTF-8");

    assert!(!out.trim_start().starts_with('/'), "{out:?}");
    assert!(!out.contains("/*"), "{out:?}");
    assert!(!out.contains("*/"), "{out:?}");
    assert!(out.contains("struct ftrace_branch_data"), "{out:?}");
}

#[test]
fn adversarial_real_gpu_filter_strips_asm_generic_param_comments() {
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("GPU backend must be available");
    let dispatcher = BackendDispatcher(backend.as_ref());
    let raw = concat!(
        "/* SPDX-License-Identifier: GPL-2.0 */\n",
        "#ifndef __ASM_GENERIC_PARAM_H\n",
        "#define __ASM_GENERIC_PARAM_H\n",
        "\n",
        "#include <uapi/asm-generic/param.h>\n",
        "\n",
        "# undef HZ\n",
        "# define HZ\t\tCONFIG_HZ\t/* Internal kernel timer frequency */\n",
        "# define USER_HZ\t100\t\t/* some user interfaces are */\n",
        "# define CLOCKS_PER_SEC\t(USER_HZ)       /* in \"ticks\" like times() */\n",
        "#endif /* __ASM_GENERIC_PARAM_H */\n",
    )
    .as_bytes();

    let filtered = gpu_filter_source_bytes(&dispatcher, raw)
        .expect("real GPU must filter asm-generic param comments");
    let out = std::str::from_utf8(&filtered.bytes).expect("filtered bytes are UTF-8");

    assert!(!out.trim_start().starts_with('/'), "{out:?}");
    assert!(!out.contains("CONFIG_HZ\t/"), "{out:?}");
    assert!(!out.contains("*/"), "{out:?}");
}

#[test]
fn adversarial_real_gpu_rescans_object_macro_replacements_without_self_recursing() {
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("GPU backend must be available");
    let dispatcher = BackendDispatcher(backend.as_ref());
    let loader = NullLoader;
    let path = PathBuf::from("recursive_replacement_rescan.c");
    let raw = concat!(
        "#define __gnu_inline __attribute__((__gnu_inline__))\n",
        "#define __maybe_unused __attribute__((__unused__))\n",
        "#define __inline_maybe_unused __maybe_unused\n",
        "#define notrace __attribute__((__no_instrument_function__))\n",
        "#define inline inline __gnu_inline __inline_maybe_unused notrace\n",
        "static inline int f(void) { return 1; }\n",
    )
    .as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("real GPU macro expansion must rescan replacement lists");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert!(out.contains("static inline __attribute__"), "{out}");
    assert!(out.contains("__gnu_inline__"), "{out}");
    assert!(out.contains("__unused__"), "{out}");
    assert!(out.contains("__no_instrument_function__"), "{out}");
    assert!(
        !out.contains("__gnu_inline __inline_maybe_unused notrace"),
        "{out}"
    );
}

#[test]
fn adversarial_real_gpu_expands_gnu_empty_variadic_comma_paste_chain() {
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("GPU backend must be available");
    let dispatcher = BackendDispatcher(backend.as_ref());
    let loader = NullLoader;
    let path = PathBuf::from("gnu_empty_variadic_comma_paste.c");
    let raw = concat!(
        "#define __stringify_1(x...) #x\n",
        "#define __stringify(x...) __stringify_1(x)\n",
        "#define __ASM_FORM_RAW(x, ...) __stringify(x,##__VA_ARGS__)\n",
        "#define __ASM_SEL_RAW(a,b) __ASM_FORM_RAW(b)\n",
        "#define __ASM_REG(reg) __ASM_SEL_RAW(e##reg, r##reg)\n",
        "#define _ASM_SP __ASM_REG(sp)\n",
        "register unsigned long current_stack_pointer asm(_ASM_SP);\n",
    )
    .as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("real GPU must expand GNU empty variadic comma paste chains");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert!(
        out.contains("register unsigned long current_stack_pointer"),
        "{out}"
    );
    assert!(out.contains("\"rsp\""), "{out}");
    assert!(!out.contains(", )"), "{out}");
}

#[test]
fn adversarial_real_gpu_prescans_function_macro_arguments_before_later_paste() {
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("GPU backend must be available");
    let dispatcher = BackendDispatcher(backend.as_ref());
    let loader = NullLoader;
    let path = PathBuf::from("function_argument_prescan_before_paste.c");
    let raw = concat!(
        "#define CONFIG_ILLEGAL_POINTER_VALUE 0xdead000000000000\n",
        "#define __AC(X,Y) (X##Y)\n",
        "#define _AC(X,Y) __AC(X,Y)\n",
        "unsigned long poison = _AC(CONFIG_ILLEGAL_POINTER_VALUE, UL);\n",
    )
    .as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("real GPU must prescan arguments before later token paste");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert!(out.contains("0xdead000000000000UL"), "{out}");
    assert!(!out.contains("CONFIG_ILLEGAL_POINTER_VALUEUL"), "{out}");
}

#[test]

fn adversarial_token_paste_edge_expands_and_records_parameter_provenance() {
    let dispatcher = ReferenceDispatcher;
    let loader = NullLoader;
    let path = PathBuf::from("paste_edge.c");
    let raw = concat!(
        "#define CAT3(a, b, c) a ## b ## c\n",
        "#define MAKE_FIELD(n) field_ ## n\n",
        "int CAT3(f, oo, bar);\n",
        "int MAKE_FIELD(42);\n",
    )
    .as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("token paste edge case must preprocess");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert!(
        out.contains("foobar"),
        "CAT3 must paste into foobar: {out:?}"
    );
    assert!(
        out.contains("field_42"),
        "MAKE_FIELD must paste identifier and number: {out:?}"
    );
    assert!(res
        .token_provenance_events
        .iter()
        .any(|event| event.macro_name == b"CAT3" && event.spelling_len == 2));
    assert!(res
        .token_provenance_events
        .iter()
        .any(|event| event.macro_name == b"MAKE_FIELD" && event.spelling_len == 2));
}

#[test]
fn adversarial_active_macro_diagnostic_is_actionable_error() {
    let dispatcher = ReferenceDispatcher;
    let loader = NullLoader;
    let path = PathBuf::from("active_error.c");
    let raw = b"#error hostile diagnostic\nint unreachable;\n";

    let err = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect_err("active #error must fail preprocessing");

    assert!(err.contains("active #error directive"), "{err}");
    assert!(err.contains("hostile diagnostic"), "{err}");
}
