//! E2E test of `prepare_resident_translation_unit_source_gpu`:
//! drives the entire 18a→18b→18c→18d chain with real on-disk
//! `#include` resolution. Validates the production tu_host wiring.

// Link concrete GPU drivers so inventory registration is present in this
// integration test binary.
#[allow(unused_imports)]
use vyre_driver_cuda as _;
#[allow(unused_imports)]
use vyre_driver_wgpu as _;

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use vyre_frontend_c::api::VyreCompileOptions;
use vyre_frontend_c::tu_host::prepare_resident_translation_unit_source_gpu;

fn gpu_prepare_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("GPU prepare test mutex poisoned")
}

fn write_tmp(name: &str, contents: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("vyre_gpu_e2e_{name}"));
    fs::write(&path, contents).expect("write temp");
    path
}

fn options_with(tmpdir_includes: Vec<PathBuf>) -> VyreCompileOptions {
    let mut opts = VyreCompileOptions::default();
    opts.disable_system_include_dirs = true;
    opts.include_dirs = tmpdir_includes;
    opts
}

fn default_options() -> VyreCompileOptions {
    options_with(Vec::new())
}

#[test]
fn passes_through_simple_source_with_no_directives() {
    let _guard = gpu_prepare_guard();
    let tu = write_tmp("plain.c", b"int main(void) { return 0; }\n");
    let raw = fs::read_to_string(&tu).unwrap();
    let out = prepare_resident_translation_unit_source_gpu(&tu, &raw, &default_options())
        .expect("prepare");
    assert!(out.contains("int"));
    assert!(out.contains("main"));
    assert!(out.contains("return"));
    assert!(out.contains("0"));
}

#[test]
fn drops_line_comment() {
    let _guard = gpu_prepare_guard();
    let tu = write_tmp("linecomment.c", b"int x; // trailing comment\nint y;\n");
    let raw = fs::read_to_string(&tu).unwrap();
    let out = prepare_resident_translation_unit_source_gpu(&tu, &raw, &default_options())
        .expect("prepare");
    assert!(!out.contains("trailing comment"));
    assert!(out.contains("int"));
}

#[test]
fn drops_block_comment() {
    let _guard = gpu_prepare_guard();
    let tu = write_tmp("block.c", b"int /* comment */ x;\n");
    let raw = fs::read_to_string(&tu).unwrap();
    let out = prepare_resident_translation_unit_source_gpu(&tu, &raw, &default_options())
        .expect("prepare");
    assert!(!out.contains("comment"));
    assert!(out.contains("int"));
    assert!(out.contains("x"));
}

#[test]
fn ifdef_active_block_passes_through() {
    let _guard = gpu_prepare_guard();
    let mut opts = VyreCompileOptions::default();
    opts.disable_system_include_dirs = true;
    opts.macros = vec![("FOO".into(), Some("1".into()))];
    let tu = write_tmp("ifdef.c", b"#ifdef FOO\nint a;\n#endif\nint b;\n");
    let raw = fs::read_to_string(&tu).unwrap();
    let out = prepare_resident_translation_unit_source_gpu(&tu, &raw, &opts).expect("prepare");
    assert!(out.contains("a"));
    assert!(out.contains("b"));
}

#[test]
fn ifdef_inactive_block_dropped() {
    let _guard = gpu_prepare_guard();
    let tu = write_tmp("ifdef_drop.c", b"#ifdef MISSING\nint a;\n#endif\nint b;\n");
    let raw = fs::read_to_string(&tu).unwrap();
    let out = prepare_resident_translation_unit_source_gpu(&tu, &raw, &default_options())
        .expect("prepare");
    assert!(
        !out.contains("a"),
        "inactive #ifdef must drop 'a'; got {out:?}"
    );
    assert!(out.contains("b"));
}

#[test]
fn local_include_inlines_file_from_disk() {
    let _guard = gpu_prepare_guard();
    let header = write_tmp("local_inc_h.h", b"int from_header;\n");
    // Place TU in same dir so relative `#include "header.h"` resolves.
    let header_dir = header.parent().unwrap().to_path_buf();
    let header_name = header.file_name().unwrap().to_string_lossy().into_owned();
    let tu_contents = format!("#include \"{header_name}\"\nint main_tu;\n");
    let tu = write_tmp("local_inc.c", tu_contents.as_bytes());
    let raw = fs::read_to_string(&tu).unwrap();
    let opts = options_with(vec![header_dir]);
    let out = prepare_resident_translation_unit_source_gpu(&tu, &raw, &opts).expect("prepare");
    assert!(out.contains("from_header"));
    assert!(out.contains("main_tu"));
}

#[test]
fn include_next_resumes_after_current_include_directory() {
    let _guard = gpu_prepare_guard();
    let root = std::env::temp_dir().join(format!("vyre_gpu_include_next_{}", std::process::id()));
    let first = root.join("first");
    let second = root.join("second");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    fs::write(
        first.join("chain.h"),
        b"#include_next \"chain.h\"\nint first_header;\n",
    )
    .unwrap();
    fs::write(second.join("chain.h"), b"int second_header;\n").unwrap();
    let tu = root.join("main.c");
    fs::write(&tu, b"#include \"chain.h\"\nint main_tu;\n").unwrap();
    let raw = fs::read_to_string(&tu).unwrap();
    let opts = options_with(vec![first, second]);

    let out = prepare_resident_translation_unit_source_gpu(&tu, &raw, &opts).expect("prepare");

    assert!(out.contains("second_header"), "{out}");
    assert!(out.contains("first_header"), "{out}");
    assert!(out.contains("main_tu"), "{out}");
}

#[test]
fn missing_local_include_returns_error() {
    let _guard = gpu_prepare_guard();
    let tu = write_tmp(
        "missing_inc.c",
        b"#include \"this_does_not_exist_xyz.h\"\nint after;\n",
    );
    let raw = fs::read_to_string(&tu).unwrap();
    let result = prepare_resident_translation_unit_source_gpu(&tu, &raw, &default_options());
    let err = result.expect_err("missing include must fail loudly");
    assert!(
        err.contains("not found") && err.contains("-I"),
        "missing include error should explain the search-path fix: {err}"
    );
}

#[test]
fn cli_define_visible_to_ifdef() {
    let _guard = gpu_prepare_guard();
    let mut opts = VyreCompileOptions::default();
    opts.disable_system_include_dirs = true;
    opts.macros = vec![("FROM_CLI".into(), None)];
    let tu = write_tmp("cli_visible.c", b"#ifdef FROM_CLI\nint visible;\n#endif\n");
    let raw = fs::read_to_string(&tu).unwrap();
    let out = prepare_resident_translation_unit_source_gpu(&tu, &raw, &opts).expect("prepare");
    assert!(out.contains("visible"));
}

#[test]
fn cli_undef_overrides_prior_define_on_gpu_prep_path() {
    let _guard = gpu_prepare_guard();
    let mut opts = VyreCompileOptions::default();
    opts.disable_system_include_dirs = true;
    opts.macros = vec![("FROM_CLI".into(), Some("1".into()))];
    opts.undefs = vec!["FROM_CLI".into()];
    let tu = write_tmp(
        "cli_undef_visible.c",
        b"#ifdef FROM_CLI\nint hidden;\n#else\nint visible;\n#endif\n",
    );
    let raw = fs::read_to_string(&tu).unwrap();
    let out = prepare_resident_translation_unit_source_gpu(&tu, &raw, &opts).expect("prepare");
    assert!(!out.contains("hidden"), "{out}");
    assert!(out.contains("visible"), "{out}");
}

#[test]
fn forced_include_runs_before_translation_unit_on_gpu_prep_path() {
    let _guard = gpu_prepare_guard();
    let header = write_tmp("forced_inc_h.h", b"#define FORCED_GPU 1\n");
    let tu = write_tmp(
        "forced_inc_main.c",
        b"#ifdef FORCED_GPU\nint visible;\n#else\nint hidden;\n#endif\n",
    );
    let raw = fs::read_to_string(&tu).unwrap();
    let mut opts = VyreCompileOptions::default();
    opts.disable_system_include_dirs = true;
    opts.forced_include_files = vec![header];

    let out = prepare_resident_translation_unit_source_gpu(&tu, &raw, &opts).expect("prepare");

    assert!(out.contains("visible"), "{out}");
    assert!(!out.contains("hidden"), "{out}");
}
