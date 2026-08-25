use super::*;

#[test]
fn cli_define_visible_to_ifdef() {
    let loader = MemLoader::new();
    let out = run(
        b"#ifdef FROM_CLI\nint visible;\n#endif\n",
        &[MacroDef {
            name: b"FROM_CLI".to_vec().into(),
            args: Vec::new(),
            body: Vec::new(),
            is_function_like: false,
        }],
        &loader,
    );
    assert!(String::from_utf8_lossy(&out).contains("visible"));
}

#[test]
fn define_above_ifdef_in_same_file_takes_active_branch() {
    // The kernel evaluates conditionals against the macro snapshot at
    // extract time. Without a host-side re-evaluation, an in-file
    // `#define` that appears above an `#ifdef` does not influence
    // that `#ifdef`'s value. This fixture verifies the host-side
    // re-eval correctly observes the live macro table.
    let loader = MemLoader::new();
    let src = b"#define IN_FILE\n#ifdef IN_FILE\nint visible;\n#endif\nint trailing;\n";
    let out = run(src, &[], &loader);
    let out_str = String::from_utf8_lossy(&out);
    assert!(
        out_str.contains("visible"),
        "in-file #define must enable subsequent #ifdef; got {out_str:?}"
    );
    assert!(out_str.contains("trailing"));
}

#[test]
fn undef_above_ifdef_drops_active_branch() {
    // CLI macro defines FOO. Source #undefs it and then `#ifdef FOO`
    // must evaluate to FALSE. Verifies the dedicated gpu_undef_parse
    // kernel actually removes the macro from the live table.
    let loader = MemLoader::new();
    let out = run(
        b"#undef FOO\n#ifdef FOO\nint should_drop;\n#endif\nint after;\n",
        &[MacroDef {
            name: b"FOO".to_vec().into(),
            args: Vec::new(),
            body: Vec::new(),
            is_function_like: false,
        }],
        &loader,
    );
    let out_str = String::from_utf8_lossy(&out);
    assert!(
        !out_str.contains("should_drop"),
        "after #undef FOO, #ifdef FOO must drop body; got {out_str:?}"
    );
    assert!(out_str.contains("after"));
}

#[test]
fn if_expr_uses_live_macro_table() {
    // `#if defined(FOO)` evaluated row-by-row should see `FOO` defined
    // by the in-file `#define` above it.
    let loader = MemLoader::new();
    let src = b"#define FOO\n#if defined(FOO)\nint visible;\n#endif\n";
    let out = run(src, &[], &loader);
    let out_str = String::from_utf8_lossy(&out);
    assert!(
        out_str.contains("visible"),
        "live macro table must be visible to subsequent #if; got {out_str:?}"
    );
}

#[test]
fn macro_prefilter_keeps_object_and_function_invocations_distinct() {
    let loader = MemLoader::new();
    let out = run(
        b"#define OBJ 7\n#define FN(x) x\nint a = OBJ;\nint b = FN(3);\nint c = FN;\n",
        &[],
        &loader,
    );
    let out_str = String::from_utf8_lossy(&out);
    assert!(
        out_str.contains("7"),
        "object-like macro use must expand after live-use prefilter; got {out_str:?}"
    );
    assert!(
        out_str.contains("3"),
        "function-like call must expand after live-use prefilter; got {out_str:?}"
    );
    assert!(
        out_str.contains("FN"),
        "bare function-like macro identifier must not be treated as an invocation; got {out_str:?}"
    );
}

#[test]
fn macro_expansion_dispatch_consumes_raw_unpadded_byte_arenas() {
    let loader = MemLoader::new();
    let dispatcher = CountingDispatcher::new();
    let active = b"int a = OBJ + FN(alpha);\n";
    let mut src = b"#define OBJ 123\n#define FN(x) x\n".to_vec();
    src.extend_from_slice(active);
    let out =
        gpu_preprocess_translation_unit(&dispatcher, &loader, Path::new("<macro-tu>"), &src, &[])
            .expect("object and function-like macros must expand through GPU materialization");
    let out_str = String::from_utf8_lossy(&out.bytes);
    assert!(
        out_str.contains("123") && out_str.contains("alpha"),
        "macro expansion must preserve object and function-like replacements; got {out_str:?}"
    );
    assert!(
        dispatcher
            .macro_byte_arena_elements
            .borrow()
            .iter()
            .all(|(_, element)| *element == DataType::U8),
        "materialized macro expansion must declare raw U8 input byte arenas"
    );
    assert!(
        dispatcher
            .macro_byte_arena_input_lens("source_words")
            .contains(&active.len()),
        "materialized macro source input must be the raw active segment length {}, got {:?}",
        active.len(),
        dispatcher.macro_byte_arena_input_lens("source_words")
    );
    assert_eq!(
        dispatcher.macro_byte_arena_input_lens("macro_name_words"),
        vec![5],
        "macro names OBJ+FN must dispatch as five raw bytes, not five padded U32 words"
    );
    assert_eq!(
        dispatcher.macro_byte_arena_input_lens("macro_replacement_words"),
        vec![4],
        "replacement bodies 123+x must dispatch as four raw bytes, not four padded U32 words"
    );
}

#[test]
fn variadic_macro_substitutes_va_args_on_gpu_expansion_path() {
    let loader = MemLoader::new();
    let out = run(
        b"#define LOG(fmt, ...) print(fmt, __VA_ARGS__)\nLOG(x, y)\n",
        &[],
        &loader,
    );
    let out_str = String::from_utf8_lossy(&out);
    assert!(
        out_str.contains("print"),
        "variadic macro body must be emitted through named GPU expansion; got {out_str:?}"
    );
    assert!(
        out_str.contains("x") && out_str.contains("y"),
        "fixed and variadic arguments must be substituted; got {out_str:?}"
    );
    assert!(
        !out_str.contains("LOG") && !out_str.contains("__VA_ARGS__"),
        "macro invocation and variadic parameter marker must not leak downstream; got {out_str:?}"
    );
}

#[test]
fn gnu_named_variadic_macro_substitutes_named_rest_parameter() {
    let loader = MemLoader::new();
    let out = run(
        b"#define LOG(fmt, rest...) print(fmt, rest)\nLOG(x, y)\n",
        &[],
        &loader,
    );
    let out_str = String::from_utf8_lossy(&out);
    assert!(
        out_str.contains("print"),
        "GNU named variadic macro body must be emitted through named GPU expansion; got {out_str:?}"
    );
    assert!(
        out_str.contains("x") && out_str.contains("y"),
        "fixed and named variadic arguments must be substituted; got {out_str:?}"
    );
    assert!(
        !out_str.contains("LOG") && !out_str.contains("rest"),
        "macro invocation and named variadic parameter marker must not leak downstream; got {out_str:?}"
    );
}
