//! Preprocessor include failure and guard contracts.

use std::fs;
use vyre_frontend_c::api::VyreCompileOptions;
use vyre_frontend_c::tu_host::reference_prepare_translation_unit_source;

fn quote_only_options() -> VyreCompileOptions {
    let mut options = VyreCompileOptions::default();
    options.disable_system_include_dirs = true;
    options
}

#[test]
fn include_cycle_a_to_b_to_a_returns_error() {
    let tmp = std::env::temp_dir().join(format!("vyre_frontend_c_cycle_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("a.h"), r#"#include "b.h""#).unwrap();
    fs::write(tmp.join("b.h"), r#"#include "a.h""#).unwrap();
    let tu = tmp.join("main.c");
    fs::write(&tu, r#"#include "a.h""#).unwrap();

    let raw = fs::read_to_string(&tu).unwrap();
    let result = reference_prepare_translation_unit_source(&tu, &raw, &quote_only_options());
    assert!(
        result.is_err(),
        "expected error for cyclic include, got:\n{result:?}"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("cycle"),
        "error message should mention cycle: {err}"
    );
}

#[test]
fn include_self_cycle_returns_error() {
    let tmp =
        std::env::temp_dir().join(format!("vyre_frontend_c_self_cycle_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("self.h"), r#"#include "self.h""#).unwrap();
    let tu = tmp.join("main.c");
    fs::write(&tu, r#"#include "self.h""#).unwrap();

    let raw = fs::read_to_string(&tu).unwrap();
    let result = reference_prepare_translation_unit_source(&tu, &raw, &quote_only_options());
    assert!(
        result.is_err(),
        "expected error for self-cycle, got:\n{result:?}"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("cycle"),
        "error message should mention cycle: {err}"
    );
}

#[test]
fn include_depth_exceeded_returns_error() {
    let tmp = std::env::temp_dir().join(format!("vyre_frontend_c_depth_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    for i in 0..65 {
        let next = if i == 64 {
            "end.h"
        } else {
            &format!("{}.h", i + 1)
        };
        fs::write(
            tmp.join(format!("{}.h", i)),
            format!(r#"#include "{}""#, next),
        )
        .unwrap();
    }
    fs::write(tmp.join("end.h"), "int end = 1;\n").unwrap();

    let tu = tmp.join("main.c");
    fs::write(&tu, r#"#include "0.h""#).unwrap();

    let raw = fs::read_to_string(&tu).unwrap();
    let result = reference_prepare_translation_unit_source(&tu, &raw, &quote_only_options());
    assert!(
        result.is_err(),
        "expected error for excessive include depth, got:\n{result:?}"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("depth"),
        "error message should mention depth: {err}"
    );
}

#[test]
fn include_depth_at_limit_succeeds() {
    let tmp = std::env::temp_dir().join(format!("vyre_frontend_c_depth_ok_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    for i in 0..63 {
        let next = if i == 62 {
            "end.h"
        } else {
            &format!("{}.h", i + 1)
        };
        fs::write(
            tmp.join(format!("{}.h", i)),
            format!(r#"#include "{}""#, next),
        )
        .unwrap();
    }
    fs::write(tmp.join("end.h"), "int end = 1;\n").unwrap();

    let tu = tmp.join("main.c");
    fs::write(&tu, r#"#include "0.h""#).unwrap();

    let raw = fs::read_to_string(&tu).unwrap();
    let result = reference_prepare_translation_unit_source(&tu, &raw, &quote_only_options());
    assert!(
        result.is_ok(),
        "depth 63 should be within limit: {result:?}"
    );
    assert!(result.unwrap().contains("int end = 1;"));
}

#[test]
fn include_size_guard_exceeded_returns_error() {
    let tmp = std::env::temp_dir().join(format!("vyre_frontend_c_size_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let big_content = "x\n".repeat(9_000_000);
    fs::write(tmp.join("big.h"), big_content).unwrap();

    let tu = tmp.join("main.c");
    fs::write(&tu, r#"#include "big.h""#).unwrap();

    let raw = fs::read_to_string(&tu).unwrap();
    let result = reference_prepare_translation_unit_source(&tu, &raw, &quote_only_options());
    assert!(
        result.is_err(),
        "expected error for oversized include expansion, got:\n{result:?}"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("exceeds") || err.contains("bytes"),
        "error message should mention size: {err}"
    );
}
