//! Token cap lift regression: 65536 → 524288.

mod support;

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;
use vyre_frontend_c::api::parse_syntax_bytes;
use vyre_frontend_c::tu_host::prepare_resident_translation_unit_source;

#[test]
fn large_source_host_prep_does_not_panic() {
    let mut source = String::new();
    for i in 0..22_000 {
        source.push_str(&format!("int x{i};\n"));
    }
    let tmp =
        std::env::temp_dir().join(format!("vyre_large_token_stream_{}.c", std::process::id()));
    std::fs::write(&tmp, &source).unwrap();
    let mut opts = vyre_frontend_c::api::VyreCompileOptions::default();
    opts.is_compile_only = true;
    opts.disable_system_include_dirs = true;
    opts.input_files = vec![tmp.clone()];
    let result = prepare_resident_translation_unit_source(&tmp, &source, &opts);
    let _ = std::fs::remove_file(&tmp);
    result.expect("host prep must not panic on large source");
}

#[test]
fn large_token_stream_exceeds_65536_and_compiles_without_panic() {
    let mut source = String::new();
    // Each `int xN;` produces 3 tokens (TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON).
    // 22_000 declarations -> 66_000 tokens, above the old 65_536 cap.
    for i in 0..22_000 {
        source.push_str(&format!("int x{i};\n"));
    }

    let summary = parse_syntax_bytes(source.as_bytes())
        .expect("large CUDA syntax parse succeeds above the old token cap");
    assert!(
        summary.token_count > 65_536,
        "expected token count > 65536, got {}",
        summary.token_count
    );
}
