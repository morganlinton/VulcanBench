//! Malformed translation unit diagnostics surfaced by the object pipeline API.

use std::fs;
use vyre_frontend_c::api::{compile, VyreCompileOptions};

fn compile_options(
    input_files: Vec<std::path::PathBuf>,
    output_file: Option<std::path::PathBuf>,
) -> VyreCompileOptions {
    let mut options = VyreCompileOptions::default();
    options.is_compile_only = true;
    options.input_files = input_files;
    options.output_file = output_file;
    options
}

#[test]
fn compile_rejects_non_c_extension_with_diagnostic() {
    let tmp = std::env::temp_dir().join(format!(
        "vyre_frontend_c_bad_ext_{}.cpp",
        std::process::id()
    ));
    fs::write(&tmp, "int x;").unwrap();

    let result = compile(compile_options(vec![tmp.clone()], None));

    let _ = fs::remove_file(&tmp);
    let err = result.expect_err("non-.c/.h extension must fail");
    assert!(
        err.contains(".c") || err.contains(".h"),
        "error must mention expected extensions: {err}"
    );
    assert!(
        err.contains("vyre-frontend-c"),
        "error must identify compiler: {err}"
    );
}

#[test]
fn compile_rejects_missing_input_file_with_diagnostic() {
    let missing =
        std::env::temp_dir().join(format!("vyre_frontend_c_missing_{}.c", std::process::id()));
    // Do not create the file.

    let result = compile(compile_options(vec![missing.clone()], None));

    let err = result.expect_err("missing file must fail");
    assert!(
        err.contains("read"),
        "error must mention file read failure: {err}"
    );
    assert!(
        err.contains(&missing.to_string_lossy().to_string()),
        "error must name the missing path: {err}"
    );
}

#[test]
fn compile_rejects_lexer_diagnostic_before_object_emission() {
    let tmp = std::env::temp_dir().join(format!(
        "vyre_frontend_c_lexer_diag_{}.c",
        std::process::id()
    ));
    fs::write(&tmp, "int x = \"unterminated;\nint y;").unwrap();

    let result = compile(compile_options(vec![tmp.clone()], None));

    let _ = fs::remove_file(&tmp);
    let err = result.expect_err("unterminated string must fail");
    assert!(
        err.contains("C lexer rejected"),
        "error surfaces lexer rejection: {err}"
    );
    assert!(
        err.contains("before parser, VAST, or ProgramGraph lowering"),
        "error marks pre-parser boundary: {err}"
    );
    assert!(
        err.contains("UnterminatedString"),
        "error includes diagnostic kind: {err}"
    );
}

#[test]
fn compile_rejects_empty_input_files_list() {
    let result = compile(compile_options(Vec::new(), None));
    let err = result.expect_err("empty input must fail");
    assert!(
        err.contains("no input files") && err.contains("VyreCompileOptions::input_files"),
        "error mentions no inputs: {err}"
    );
}

#[test]
fn compile_rejects_single_output_for_multiple_compile_only_inputs() {
    let dir = std::env::temp_dir().join(format!("vyre_frontend_c_multi_o_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.c");
    let b = dir.join("b.c");
    let out = dir.join("one.o");
    fs::write(&a, "int a;").unwrap();
    fs::write(&b, "int b;").unwrap();

    let result = compile(compile_options(vec![a, b], Some(out)));

    let _ = fs::remove_dir_all(&dir);
    let err = result.expect_err("multiple compile-only inputs with one -o must fail");
    assert!(
        err.contains("multiple compile inputs")
            && err.contains("single-output contract")
            && err.contains("omit output_file"),
        "error must explain the single-output contract: {err}"
    );
    assert!(
        !err.contains("not supported yet"),
        "driver contract must not present this as unfinished work: {err}"
    );
}
