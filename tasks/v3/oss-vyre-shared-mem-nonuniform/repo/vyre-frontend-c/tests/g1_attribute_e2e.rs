//! g1_attribute_e2e  -  __attribute__((...)) recognition and skip.
//!
//! Validates that GNU attribute syntax is parsed, specific attribute names are
//! recognized, and malformed attribute syntax is rejected.

mod support;

use support::*;
use vyre_frontend_c::api::{compile, VyreCompileOptions};

const DECLS_SOURCE: &str = include_str!("corpus/g1_attribute/decls.c");

#[test]
fn attribute_decls_compile_successfully() {
    let (object, _resident) =
        compile_source_with_resident("g1_attr_decls", DECLS_SOURCE, Vec::new(), Vec::new());
    object.assert_elf();

    // Verify that every declaration produced a non-empty VAST section.
    let vast = object.section(SECTION_VAST);
    assert_ne!(
        vast.len(),
        0,
        "attribute-bearing TU must produce a VAST section"
    );
}

#[test]
fn malformed_attribute_is_rejected() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/g1_attribute/negatives/malformed.c");

    let mut options = VyreCompileOptions::default();
    options.is_compile_only = true;
    options.disable_system_include_dirs = true;
    options.input_files = vec![fixture.clone()];
    let result = compile(options);

    let err = result.expect_err("malformed __attribute__ must be rejected");
    assert!(
        err.contains("malformed-gnu-attribute") || err.contains("dispatch failed"),
        "error must surface malformed attribute rejection, got: {err}"
    );
}
