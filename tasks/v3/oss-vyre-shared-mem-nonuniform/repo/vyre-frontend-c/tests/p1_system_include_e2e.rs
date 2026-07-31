//! P1  -  explicit system-include paths.
//!
//! Proves vyrec's host preprocessor resolves `#include <...>` through explicit
//! system roots supplied by the invocation. Compiler-default discovery is
//! intentionally disabled in the GPU-first production path because spawning
//! gcc/clang/cc would add a host compiler dependency and a slow CPU probe.

use std::fs;
use std::path::PathBuf;

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;
use vyre_frontend_c::api::VyreCompileOptions;
use vyre_frontend_c::tu_host::prepare_resident_translation_unit_source;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/p1_system_include")
        .join(rel)
}

fn read_fixture(rel: &str) -> (PathBuf, String) {
    let path = fixture(rel);
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read fixture {rel}: {e}"));
    (path, source)
}

fn explicit_system_include_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vyre_frontend_c_p1_system_include_{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap_or_else(|error| {
        panic!(
            "create explicit system include fixture {}: {error}",
            dir.display()
        )
    });
    fs::write(dir.join("stdint.h"), b"typedef int int32_t;\n")
        .unwrap_or_else(|error| panic!("write stdint.h fixture in {}: {error}", dir.display()));
    dir
}

fn opts_with_system_includes(
    input: PathBuf,
    system_include_dirs: Vec<PathBuf>,
) -> VyreCompileOptions {
    let mut options = VyreCompileOptions::default();
    options.is_compile_only = true;
    options.input_files = vec![input];
    options.system_include_dirs = system_include_dirs;
    options.disable_system_include_dirs = true;
    options
}

#[test]
fn resolves_system_header_from_explicit_system_include_dir() {
    let (path, source) = read_fixture("empty_main.c");
    let opts = opts_with_system_includes(path.clone(), vec![explicit_system_include_dir()]);
    let resident = prepare_resident_translation_unit_source(&path, &source, &opts).expect(
        "P1 contract: <stdint.h> resolves through explicit -isystem/sysroot include roots. Fix: pass the target system include directory explicitly.",
    );

    // The resident TU must contain stdint.h's defining content. Two markers
    // because some C libraries split definitions across multiple files included
    // transitively  -  at least one of these must survive the inline expansion.
    let merged = resident.to_ascii_lowercase();
    assert!(
        merged.contains("int32_t") || merged.contains("__int32_t"),
        "expanded TU must carry stdint.h symbols; got {} bytes",
        resident.len(),
    );
}

#[test]
fn missing_system_header_is_rejected() {
    let (path, source) = read_fixture("negatives/missing_header.c");
    let opts = opts_with_system_includes(path.clone(), vec![explicit_system_include_dir()]);
    let result = prepare_resident_translation_unit_source(&path, &source, &opts);
    let err = match result {
        Err(message) => message,
        Ok(_) => panic!(
            "P1 negative twin: <this_header_does_not_exist_88e5f1c2.h> must be rejected, not silently accepted"
        ),
    };
    assert!(
        err.contains("this_header_does_not_exist_88e5f1c2.h"),
        "rejection message must name the missing header for diagnostic clarity, got: {err}"
    );
}
