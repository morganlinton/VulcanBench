use std::path::PathBuf;

use super::preprocess_reference::ReferenceDispatcher;
use super::NullLoader;
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    gpu_preprocess_translation_unit, IncludeLoader,
};

#[test]
fn gpu_preprocess_records_object_like_macro_expansion_origins() {
    let dispatcher = ReferenceDispatcher;
    let loader = NullLoader;
    let path = PathBuf::from("object_macro.c");
    let raw = concat!("#define X 42\n", "int x = X;\n").as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("gpu preprocess succeeds");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes must be UTF-8");

    assert!(
        out.contains("42"),
        "object-like macro must expand on GPU: {out:?}"
    );
    assert_eq!(res.macro_expansion_events.len(), 1);
    let event = &res.macro_expansion_events[0];
    assert_eq!(event.file, path);
    assert_eq!(event.name, b"X");
    assert_eq!(event.replacement, b"42");
    assert!(event.invocation_args.is_empty());
    assert!(event.include_stack.is_empty());
    assert_eq!(event.use_len, 1);
    assert!(!event.is_function_like);
    assert!(!event.is_variadic);
    assert!(event.gpu_resident);
    assert!(
        event.symbol_id.iter().any(|byte| *byte != 0),
        "stable macro expansion symbol ID must not be all zeros"
    );
    let provenance = res
        .token_provenance_events
        .iter()
        .find(|provenance| provenance.macro_name == b"X")
        .expect("expanded macro token provenance must be recorded");
    assert_eq!(provenance.output_len, 2);
    assert_eq!(provenance.spelling_file, path);
    assert_eq!(provenance.spelling_start, 10);
    assert_eq!(provenance.spelling_len, 2);
    assert_eq!(provenance.expansion_file, path);
    assert_eq!(provenance.expansion_len, 1);
    assert_eq!(provenance.macro_symbol_id, Some(event.symbol_id));
    assert!(provenance.include_stack.is_empty());
    assert!(provenance.gpu_resident);
}

#[test]
fn gpu_preprocess_records_function_like_macro_expansion_origins() {
    let dispatcher = ReferenceDispatcher;
    let loader = NullLoader;
    let path = PathBuf::from("function_macro.c");
    let raw = concat!(
        "#define ADD(a, b) ((a)+(b))\n",
        "#define STR(x) #x\n",
        "#define CAT(a, b) a ## b\n",
        "#define LOG(fmt, ...) fmt\n",
        "int a = ADD(1, 2);\n",
        "char *s = STR(abc);\n",
        "int CAT(foo, bar);\n",
        "LOG(\"x\", 1);\n",
    )
    .as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("gpu preprocess succeeds");

    for name in [
        b"ADD".as_slice(),
        b"STR".as_slice(),
        b"CAT".as_slice(),
        b"LOG".as_slice(),
    ] {
        assert!(
            res.macro_expansion_events
                .iter()
                .any(|event| event.name == name
                    && event.is_function_like
                    && event.gpu_resident
                    && !event.invocation_args.is_empty()),
            "function-like macro expansion event missing for {:?}: {:#?}",
            std::str::from_utf8(name).unwrap_or("<non-utf8>"),
            res.macro_expansion_events
        );
    }
    let log = res
        .macro_expansion_events
        .iter()
        .find(|event| event.name == b"LOG")
        .expect("LOG expansion must be recorded");
    assert!(log.is_variadic);
    assert_eq!(log.invocation_args, b"\"x\", 1");
    let add_provenance: Vec<_> = res
        .token_provenance_events
        .iter()
        .filter(|provenance| provenance.macro_name == b"ADD")
        .collect();
    assert!(
        !add_provenance.is_empty(),
        "function-like macro expansion must emit token-level provenance"
    );
    assert!(add_provenance
        .iter()
        .all(|provenance| provenance.expansion_len == 9
            && provenance.expansion_file == path
            && provenance.include_stack.is_empty()
            && provenance.macro_symbol_id.is_some()
            && provenance.gpu_resident));
    let expansion_start = add_provenance[0].expansion_start;
    let arg_one = expansion_start + 4;
    let arg_two = expansion_start + 7;
    assert!(
        add_provenance
            .iter()
            .any(|provenance| provenance.spelling_start == arg_one && provenance.spelling_len == 1),
        "substituted parameter `a` must spell from invocation argument `1`: {add_provenance:#?}"
    );
    assert!(
        add_provenance
            .iter()
            .any(|provenance| provenance.spelling_start == arg_two && provenance.spelling_len == 1),
        "substituted parameter `b` must spell from invocation argument `2`: {add_provenance:#?}"
    );
    let str_provenance: Vec<_> = res
        .token_provenance_events
        .iter()
        .filter(|provenance| provenance.macro_name == b"STR")
        .collect();
    let str_expansion_start = str_provenance[0].expansion_start;
    assert!(
        str_provenance.iter().any(|provenance| {
            provenance.spelling_start == str_expansion_start + 4 && provenance.spelling_len == 3
        }),
        "stringification parameter must spell from invocation argument `abc`: {str_provenance:#?}"
    );
    let cat_provenance: Vec<_> = res
        .token_provenance_events
        .iter()
        .filter(|provenance| provenance.macro_name == b"CAT")
        .collect();
    let cat_expansion_start = cat_provenance[0].expansion_start;
    assert!(
        cat_provenance.iter().any(|provenance| {
            provenance.spelling_start == cat_expansion_start + 4 && provenance.spelling_len == 3
        }),
        "token-paste left parameter must spell from invocation argument `foo`: {cat_provenance:#?}"
    );
    assert!(
        cat_provenance.iter().any(|provenance| {
            provenance.spelling_start == cat_expansion_start + 9 && provenance.spelling_len == 3
        }),
        "token-paste right parameter must spell from invocation argument `bar`: {cat_provenance:#?}"
    );
}

#[test]
fn gpu_preprocess_records_macro_expansion_include_stack() {
    struct HeaderLoader;
    impl IncludeLoader for HeaderLoader {
        fn load(
            &self,
            path: &[u8],
            _is_system: bool,
            _is_next: bool,
            _from: &std::path::Path,
        ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
            assert_eq!(path, b"h.h");
            Ok(Some((
                PathBuf::from("/tmp/vyrec-macro-stack/h.h"),
                b"#define H 7\nint y = H;\n".to_vec().into(),
            )))
        }
    }

    let dispatcher = ReferenceDispatcher;
    let loader = HeaderLoader;
    let path = PathBuf::from("/tmp/vyrec-macro-stack/main.c");
    let raw = b"#include \"h.h\"\n";

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("gpu preprocess succeeds");

    let event = res
        .macro_expansion_events
        .iter()
        .find(|event| event.name == b"H")
        .expect("header macro expansion must be recorded");
    assert_eq!(
        event.include_stack,
        vec![PathBuf::from("/tmp/vyrec-macro-stack/h.h")]
    );
    let provenance = res
        .token_provenance_events
        .iter()
        .find(|provenance| provenance.macro_name == b"H")
        .expect("header macro token provenance must be recorded");
    assert_eq!(
        provenance.include_stack,
        vec![PathBuf::from("/tmp/vyrec-macro-stack/h.h")]
    );
    assert_eq!(
        provenance.spelling_file,
        PathBuf::from("/tmp/vyrec-macro-stack/h.h")
    );
    assert_eq!(
        provenance.expansion_file,
        PathBuf::from("/tmp/vyrec-macro-stack/h.h")
    );
}

#[test]
fn gpu_preprocess_records_each_replacement_token_provenance() {
    let dispatcher = ReferenceDispatcher;
    let loader = NullLoader;
    let path = PathBuf::from("replacement_tokens.c");
    let raw = concat!("#define PAIR 1 + 2\n", "int x = PAIR;\n").as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("gpu preprocess succeeds");
    let replacement_tokens: Vec<_> = res
        .token_provenance_events
        .iter()
        .filter(|provenance| provenance.macro_name == b"PAIR")
        .collect();

    assert_eq!(replacement_tokens.len(), 3);
    assert_eq!(
        replacement_tokens
            .iter()
            .map(|provenance| (provenance.spelling_start, provenance.spelling_len))
            .collect::<Vec<_>>(),
        vec![(13, 1), (15, 1), (17, 1)]
    );
    assert!(replacement_tokens
        .iter()
        .all(|provenance| provenance.expansion_len == 4
            && provenance.expansion_file == path
            && provenance.gpu_resident));
}

#[test]
fn gpu_preprocess_records_identity_path_token_provenance() {
    let dispatcher = ReferenceDispatcher;
    let loader = NullLoader;
    let path = PathBuf::from("identity_tokens.c");
    let raw = b"int x = 1;\n";

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("gpu preprocess succeeds");
    let first = res
        .token_provenance_events
        .first()
        .expect("identity path must still emit token provenance");

    assert_eq!(res.bytes, raw);
    assert_eq!(first.file, path);
    assert_eq!(first.output_start, 0);
    assert_eq!(first.spelling_file, path);
    assert_eq!(first.spelling_start, 0);
    assert_eq!(first.expansion_file, path);
    assert!(first.include_stack.is_empty());
    assert!(first.macro_symbol_id.is_none());
    assert!(first.gpu_resident);
}
