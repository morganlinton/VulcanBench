//! Lexer diagnostic contracts: malformed tokens fail before VAST / ProgramGraph.
use std::fs;
use std::path::PathBuf;

use vyre_frontend_c::api::{compile, VyreCompileOptions};

fn compile_source_expect_error(name: &str, source: &str) -> String {
    let mut src = std::env::temp_dir();
    src.push(format!("vyre_frontend_c_{name}_{}.c", std::process::id()));
    let mut out = PathBuf::from(&src);
    out.set_extension("o");
    fs::write(&src, source).expect("write malformed C source fixture");

    let mut options = VyreCompileOptions::default();
    options.is_compile_only = true;
    options.input_files = vec![src.clone()];
    options.output_file = Some(out.clone());
    let result = compile(options);

    let _ = fs::remove_file(&src);
    let _ = fs::remove_file(&out);
    result.expect_err("malformed lexer token must fail before parser entry")
}

#[test]
fn compile_rejects_lexer_diagnostics_before_vast_and_pg() {
    for (name, source, detail, kind) in [
        (
            "unterminated_string",
            "int x = \"unterminated;\nint y;",
            "unterminated string literal",
            "UnterminatedString",
        ),
        (
            "unterminated_char",
            "char c = 'x;\nint y;",
            "unterminated character literal",
            "UnterminatedChar",
        ),
        (
            "unterminated_comment",
            "int x; /* never closed",
            "unterminated block comment",
            "UnterminatedBlockComment",
        ),
        (
            "invalid_escape",
            "char *s = \"bad\\q\";",
            "invalid string or character escape",
            "InvalidEscape",
        ),
    ] {
        let error = compile_source_expect_error(name, source);
        assert!(
            error.contains("C lexer rejected"),
            "{name}: compiler must surface lexer rejection, got: {error}"
        );
        assert!(
            error.contains(detail),
            "{name}: compiler error must include diagnostic detail, got: {error}"
        );
        assert!(
            error.contains(kind),
            "{name}: compiler error must include diagnostic kind, got: {error}"
        );
        assert!(
            error.contains("token kind") && error.contains("token index"),
            "{name}: compiler error must include token index and kind, got: {error}"
        );
        assert!(
            error.contains("byte span") && error.contains("length"),
            "{name}: compiler error must include source span, got: {error}"
        );
        assert!(
            error.contains("before parser, VAST, or ProgramGraph lowering"),
            "{name}: compiler error must make the pre-parser boundary explicit, got: {error}"
        );
    }
}
