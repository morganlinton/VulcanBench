//! Host-side preprocessor preparation for GPU translation units (`-D`, includes).
use std::fs;

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;
use vyre_frontend_c::api::VyreCompileOptions;
use vyre_frontend_c::tu_host::{
    prepare_resident_translation_unit_source, reference_expand_preprocessor_macros,
    reference_prepare_translation_unit_source,
};

fn quote_only_options() -> VyreCompileOptions {
    let mut options = VyreCompileOptions::default();
    options.disable_system_include_dirs = true;
    options
}

#[test]
fn object_and_function_macros_expand_before_gpu_lexing() {
    let source = "\
#define PAGE_SHIFT 12
#define PAGE_SIZE (1 << PAGE_SHIFT)
#define ADD(a, b) ((a) + (b))
int x = PAGE_SIZE;
int y = ADD(2, 3);
";

    let out = reference_expand_preprocessor_macros(source);
    assert!(!out.contains("#define"));
    assert!(out.contains("int x = (1 << 12);"));
    assert!(out.contains("int y = ((2) + (3));"));
}

#[test]
fn stringify_and_token_paste_expand_without_touching_string_literals() {
    let source = "\
#define STR(x) #x
#define FIELD(n) field_##n
const char *s = STR(hello world);
int FIELD(7) = 1;
const char *literal = \"FIELD(7)\";
";

    let out = reference_expand_preprocessor_macros(source);
    assert!(out.contains("const char *s = \"hello world\";"));
    assert!(
        out.contains("int field_7 = 1;"),
        "token-paste output was:\n{out}"
    );
    assert!(out.contains("const char *literal = \"FIELD(7)\";"));
}

#[test]
fn conditional_branches_are_removed_before_token_pipeline() {
    let source = "\
#define ENABLED 1
#if defined(ENABLED) && ENABLED == 1
int active = 1;
#else
int inactive = 1;
#endif
#ifndef MISSING
int also_active = 2;
#endif
";

    let out = reference_expand_preprocessor_macros(source);
    assert!(out.contains("int active = 1;"));
    assert!(out.contains("int also_active = 2;"));
    assert!(!out.contains("inactive"));
    assert!(!out.contains("#if"));
}

#[test]
fn variadic_macros_expand_named_and_standard_arguments() {
    let source = "\
#define TRACE(fmt, ...) log(fmt, __VA_ARGS__)
#define WRAP(prefix, rest...) prefix(rest)
TRACE(\"%d %d\", 1, 2);
WRAP(call, x, y);
";

    let out = reference_expand_preprocessor_macros(source);
    assert!(out.contains("log(\"%d %d\", 1, 2);"));
    assert!(out.contains("call(x, y);"));
}

#[test]
fn directive_comments_do_not_leak_into_macro_values_or_conditions() {
    let source = "\
#define VALUE 7 /* trailing block comment */
#define ENABLED 1 // trailing line comment
#if defined(ENABLED) && VALUE == 7 /* condition comment */
int value = VALUE;
#else
int wrong = VALUE;
#endif
";

    let out = reference_expand_preprocessor_macros(source);
    assert!(out.contains("int value = 7;"));
    assert!(!out.contains("wrong"));
    assert!(!out.contains("comment"));
}

#[test]
fn undef_removes_later_macro_expansion() {
    let source = "\
#define VALUE 7
int before = VALUE;
#undef VALUE
int after = VALUE;
";

    let out = reference_expand_preprocessor_macros(source);
    assert!(out.contains("int before = 7;"));
    assert!(out.contains("int after = VALUE;"));
}

#[test]
fn include_defined_macros_are_visible_to_later_translation_unit_lines() {
    let tmp = std::env::temp_dir().join(format!(
        "vyre_frontend_c_tu_preprocessor_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("defs.h"), "#define VALUE 42\n").unwrap();
    let tu = tmp.join("main.c");
    fs::write(&tu, "#include \"defs.h\"\nint v = VALUE;\n").unwrap();

    let raw = fs::read_to_string(&tu).unwrap();
    let out = reference_prepare_translation_unit_source(&tu, &raw, &quote_only_options()).unwrap();

    assert!(out.contains("int v = 42;"));
    assert!(!out.contains("#include"));
    assert!(!out.contains("#define"));
}

#[test]
fn resident_compile_prep_expands_include_macros_on_gpu_frontend() {
    let tmp = std::env::temp_dir().join(format!(
        "vyre_frontend_c_resident_tu_preprocessor_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("defs.h"), "#define VALUE 42\n").unwrap();
    let tu = tmp.join("main.c");
    fs::write(&tu, "#include \"defs.h\"\nint v = VALUE;\n").unwrap();

    let raw = fs::read_to_string(&tu).unwrap();
    let out = prepare_resident_translation_unit_source(&tu, &raw, &quote_only_options()).unwrap();

    assert!(!out.contains("#include"));
    assert!(!out.contains("#define VALUE 42"));
    assert!(out.contains("int v = 42"));
    assert!(
        !out.contains("int v = VALUE;"),
        "resident path must expand included macros through the GPU frontend"
    );
}
